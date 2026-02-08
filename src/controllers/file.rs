use axum::{
    Json,
    body::Body,
    extract::{Multipart, Path, State},
    http::{HeaderMap, StatusCode, header},
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::fs::{self, File};
use tokio_util::io::ReaderStream;

use crate::{
    controllers::user::LoggedUser,
    controllers::{AppError, Context},
    utils::{jwt::Claims, response::Response},
};

// ==================== 请求/响应结构体 ====================

/// 目录及其文件的映射响应
pub type DirectoryMapResponse = HashMap<String, Vec<String>>;

/// 文件列表响应
pub type FileListResponse = Vec<String>;

/// 分片上传响应
#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct PartUploadResponse {
    pub chunk_id: String,
}

/// 合并分片请求
#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct MergeRequest {
    pub chunk_ids: Vec<String>,
    pub directory: String,
    pub filename: String,
}

/// 重命名请求
#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct RenameRequest {
    pub new_name: String,
}

fn make_file_response(file: File, filename: &str) -> impl IntoResponse + use<> {
    // 使用 ReaderStream 将文件转为流，避免一次性读入内存
    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);

    // 构建响应头
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        "application/octet-stream".parse().unwrap(),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        format!("attachment; filename=\"{}\"", filename.replace("\"", "_"))
            .parse()
            .unwrap(),
    );
    return (StatusCode::OK, headers, body);
}

// ==================== 用户目录操作 ====================

/// 列出所有用户目录及其文件
#[utoipa::path(
    get,
    path = "/user/",
    responses(
        (status = OK, body = Response<DirectoryMapResponse>)
    )
)]
pub async fn list_directories(
    State(ctx): State<Context>,
    Claims(_user): Claims<LoggedUser>,
) -> Result<Response<DirectoryMapResponse>, AppError> {
    let directories = ctx.storage.user().list_directory().await?;
    Ok(Response::ok().payload(directories))
}

/// 新建用户目录
#[utoipa::path(
    post,
    path = "/user/{directory}/",
    responses(
        (status = OK, body = Response<bool>)
    )
)]
pub async fn create_directory(
    State(ctx): State<Context>,
    Claims(_user): Claims<LoggedUser>,
    Path(directory): Path<String>,
) -> Result<Response<bool>, AppError> {
    ctx.storage.user().new_directory(&directory).await?;
    Ok(Response::ok()
        .msg("directory created successfully")
        .payload(true))
}

/// 删除用户目录
#[utoipa::path(
    delete,
    path = "/user/{directory}/",
    responses(
        (status = OK, body = Response<bool>)
    ),
)]
pub async fn delete_directory(
    State(ctx): State<Context>,
    Claims(_user): Claims<LoggedUser>,
    Path(directory): Path<String>,
) -> Result<Response<bool>, AppError> {
    ctx.storage.user().delete_directory(&directory).await?;
    Ok(Response::ok()
        .msg("directory deleted successfully")
        .payload(true))
}

/// 重命名用户目录
#[utoipa::path(
    patch,
    path = "/user/{directory}/",
    responses(
        (status = OK, body = Response<bool>)
    ),
)]
pub async fn rename_directory(
    State(ctx): State<Context>,
    Claims(_user): Claims<LoggedUser>,
    Path(directory): Path<String>,
    Json(request): Json<RenameRequest>,
) -> Result<Response<bool>, AppError> {
    ctx.storage
        .user()
        .rename_directory(&directory, &request.new_name)
        .await?;

    Ok(Response::ok()
        .msg("directory renamed successfully")
        .payload(true))
}

// ==================== 目录文件操作 ====================

/// 列出指定目录下的所有文件
#[utoipa::path(
    get,
    path = "/user/{directory}/",
    responses(
        (status = OK, body = Response<FileListResponse>)
    ),
)]
pub async fn list_files_in_directory(
    State(ctx): State<Context>,
    Claims(_user): Claims<LoggedUser>,
    Path(directory): Path<String>,
) -> Result<Response<FileListResponse>, AppError> {
    let files = ctx.storage.user().list_directory_file(&directory).await?;
    Ok(Response::ok().payload(files))
}

/// 直接上传文件
#[utoipa::path(
    post,
    path = "/user/{directory}/{file}",
    responses(
        (status = OK, body = Response<bool>)
    ),
)]
pub async fn upload_file(
    State(ctx): State<Context>,
    Claims(_user): Claims<LoggedUser>,
    Path((directory, file)): Path<(String, String)>,
    mut multipart: Multipart,
) -> Result<Response<bool>, AppError> {
    // 从 multipart 中提取文件内容
    let mut file_content: Option<Vec<u8>> = None;

    while let Some(field) = multipart.next_field().await? {
        let data = field.bytes().await?;
        file_content = Some(data.to_vec());
        break; // 只处理第一个字段
    }

    let content = file_content.ok_or_else(|| anyhow::anyhow!("no file content provided"))?;

    ctx.storage
        .user()
        .write_file(&directory, &file, &content)
        .await?;

    Ok(Response::ok()
        .msg("file uploaded successfully")
        .payload(true))
}

/// 下载文件
#[utoipa::path(
    get,
    path = "/user/{directory}/{file}",
    responses(
        (status = OK, body = Vec<u8>)
    ),
)]
pub async fn download_file(
    State(ctx): State<Context>,
    Claims(_user): Claims<LoggedUser>,
    Path((directory, file)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    let file_handle = ctx
        .storage
        .user()
        .open_file(&directory, &file, fs::OpenOptions::new().read(true))
        .await?;

    Ok(make_file_response(file_handle, &file))
}

/// 删除文件
#[utoipa::path(
    delete,
    path = "/user/{directory}/{file}",
    responses(
        (status = OK, body = Response<bool>)
    ),
)]
pub async fn delete_file(
    State(ctx): State<Context>,
    Claims(_user): Claims<LoggedUser>,
    Path((directory, file)): Path<(String, String)>,
) -> Result<Response<bool>, AppError> {
    ctx.storage.user().delete_file(&directory, &file).await?;

    Ok(Response::ok()
        .msg("file deleted successfully")
        .payload(true))
}

/// 重命名文件
#[utoipa::path(
    patch,
    path = "/user/{directory}/{file}",
    responses(
        (status = OK, body = Response<bool>)
    ),
)]
pub async fn rename_file(
    State(ctx): State<Context>,
    Claims(_user): Claims<LoggedUser>,
    Path((directory, file)): Path<(String, String)>,
    Json(request): Json<RenameRequest>,
) -> Result<Response<bool>, AppError> {
    ctx.storage
        .user()
        .rename_file(&directory, &file, &request.new_name)
        .await?;

    Ok(Response::ok()
        .msg("file renamed successfully")
        .payload(true))
}

// ==================== 分片上传操作 ====================

/// 分片上传文件
#[utoipa::path(
    post,
    path = "/temp/part",
    responses(
        (status = OK, body = Response<PartUploadResponse>)
    ),
)]
pub async fn upload_part(
    State(ctx): State<Context>,
    Claims(_user): Claims<LoggedUser>,
    mut multipart: Multipart,
) -> Result<Response<PartUploadResponse>, AppError> {
    // 从 multipart 中提取分片内容
    let mut chunk_content: Option<Vec<u8>> = None;

    while let Some(field) = multipart.next_field().await? {
        let data = field.bytes().await?;
        chunk_content = Some(data.to_vec());
        break; // 只处理第一个字段
    }

    let content = chunk_content.ok_or_else(|| anyhow::anyhow!("no chunk content provided"))?;

    let chunk_id = ctx.storage.temp().save(&content).await?;

    Ok(Response::ok()
        .msg("chunk uploaded successfully")
        .payload(PartUploadResponse { chunk_id }))
}

/// 合并分片上传的文件到用户目录
#[utoipa::path(
    post,
    path = "/temp/merge",
    responses(
        (status = OK, body = Response<bool>)
    ),
)]
pub async fn merge_parts(
    State(ctx): State<Context>,
    Claims(_user): Claims<LoggedUser>,
    Json(request): Json<MergeRequest>,
) -> Result<Response<bool>, AppError> {
    let target_file = ctx
        .storage
        .user()
        .open_file(
            &request.directory,
            &request.filename,
            fs::OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true),
        )
        .await?;

    // 合并分片
    ctx.storage
        .temp()
        .merge(&request.chunk_ids, target_file)
        .await?;

    Ok(Response::ok().msg("file merged successfully").payload(true))
}

// ==================== 日志文件操作 ====================

/// 下载日志文件
#[utoipa::path(
    get,
    path = "/log/{file}",
    responses(
        (status = OK, body = Vec<u8>)
    ),
)]
pub async fn download_log_file(
    State(ctx): State<Context>,
    Claims(_user): Claims<LoggedUser>,
    Path(file): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let file_handle = ctx
        .storage
        .log()
        .open(&file, fs::OpenOptions::new().read(true))
        .await?;

    Ok(make_file_response(file_handle, &file))
}
