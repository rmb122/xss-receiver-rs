use axum::{
    Json,
    body::Body,
    extract::{Multipart, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use tokio::fs::File;
use tokio_util::io::ReaderStream;

use crate::{
    controllers::user::LoggedUser,
    controllers::{AppError, Context},
    storage::Entry,
    utils::{jwt::Claims, response::Response},
};

// ==================== 请求/响应结构体 ====================

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct PathRequest {
    pub path: String,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ListResponse {
    pub entries: Vec<Entry>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ListAllResponse {
    pub files: Vec<String>,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct RenameRequest {
    pub src: String,
    pub dst: String,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct MergeRequest {
    pub chunk_ids: Vec<String>,
    pub path: String,
}

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct PartUploadResponse {
    pub chunk_id: String,
}

#[derive(Debug, Deserialize)]
pub struct PathQuery {
    pub path: String,
}

// ==================== 工具函数 ====================

fn make_file_response(file: File, filename: &str) -> impl IntoResponse + use<> {
    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);

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
    (StatusCode::OK, headers, body)
}

fn basename(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

// ==================== 目录/文件操作 ====================

/// 列出指定目录下的所有直接子项
#[utoipa::path(
    post,
    path = "/list",
    responses((status = OK, body = Response<ListResponse>))
)]
pub async fn list(
    State(ctx): State<Context>,
    Claims(_user): Claims<LoggedUser>,
    Json(req): Json<PathRequest>,
) -> Result<Response<ListResponse>, AppError> {
    let entries = ctx.storage.user().list(&req.path)?;
    Ok(Response::ok().payload(ListResponse { entries }))
}

/// 递归列出所有文件
#[utoipa::path(
    post,
    path = "/listAll",
    responses((status = OK, body = Response<ListAllResponse>))
)]
pub async fn list_all(
    State(ctx): State<Context>,
    Claims(_user): Claims<LoggedUser>,
) -> Result<Response<ListAllResponse>, AppError> {
    let files = ctx.storage.user().list_all_files()?;
    Ok(Response::ok().payload(ListAllResponse { files }))
}

/// 创建目录（递归创建）
#[utoipa::path(
    post,
    path = "/mkdir",
    responses((status = OK, body = Response<bool>))
)]
pub async fn mkdir(
    State(ctx): State<Context>,
    Claims(_user): Claims<LoggedUser>,
    Json(req): Json<PathRequest>,
) -> Result<Response<bool>, AppError> {
    ctx.storage.user().mkdir(&req.path)?;
    Ok(Response::ok().payload(true))
}

/// 删除文件或目录
#[utoipa::path(
    post,
    path = "/remove",
    responses((status = OK, body = Response<bool>))
)]
pub async fn remove(
    State(ctx): State<Context>,
    Claims(_user): Claims<LoggedUser>,
    Json(req): Json<PathRequest>,
) -> Result<Response<bool>, AppError> {
    ctx.storage.user().remove(&req.path)?;
    Ok(Response::ok().payload(true))
}

/// 重命名/移动文件或目录
#[utoipa::path(
    post,
    path = "/rename",
    responses((status = OK, body = Response<bool>))
)]
pub async fn rename(
    State(ctx): State<Context>,
    Claims(_user): Claims<LoggedUser>,
    Json(req): Json<RenameRequest>,
) -> Result<Response<bool>, AppError> {
    ctx.storage.user().rename(&req.src, &req.dst)?;
    Ok(Response::ok().payload(true))
}

/// 直接上传小文件 (multipart form-data, path 从 form field 中读取)
#[utoipa::path(
    post,
    path = "/upload",
    responses((status = OK, body = Response<bool>))
)]
pub async fn upload(
    State(ctx): State<Context>,
    Claims(_user): Claims<LoggedUser>,
    mut multipart: Multipart,
) -> Result<Response<bool>, AppError> {
    let mut path: Option<String> = None;
    let mut content: Option<Vec<u8>> = None;

    while let Some(field) = multipart.next_field().await? {
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "path" => {
                path = Some(field.text().await?);
            }
            "file" => {
                content = Some(field.bytes().await?.to_vec());
            }
            _ => {}
        }
    }

    let path = path.ok_or_else(|| anyhow::anyhow!("missing 'path' form field"))?;
    let content = content.ok_or_else(|| anyhow::anyhow!("missing 'file' form field"))?;

    ctx.storage.user().write(&path, &content)?;
    Ok(Response::ok().payload(true))
}

// ==================== 分片上传 ====================

/// 分片上传到 temp
#[utoipa::path(
    post,
    path = "/part",
    responses((status = OK, body = Response<PartUploadResponse>))
)]
pub async fn upload_part(
    State(ctx): State<Context>,
    Claims(_user): Claims<LoggedUser>,
    mut multipart: Multipart,
) -> Result<Response<PartUploadResponse>, AppError> {
    let mut chunk_content: Option<Vec<u8>> = None;

    while let Some(field) = multipart.next_field().await? {
        let data = field.bytes().await?;
        chunk_content = Some(data.to_vec());
        break;
    }

    let content = chunk_content.ok_or_else(|| anyhow::anyhow!("no chunk content provided"))?;
    let chunk_id = ctx.storage.temp().save(&content)?;
    Ok(Response::ok().payload(PartUploadResponse { chunk_id }))
}

/// 合并分片到指定路径
#[utoipa::path(
    post,
    path = "/merge",
    responses((status = OK, body = Response<bool>))
)]
pub async fn merge_parts(
    State(ctx): State<Context>,
    Claims(_user): Claims<LoggedUser>,
    Json(req): Json<MergeRequest>,
) -> Result<Response<bool>, AppError> {
    let target_file = ctx.storage.user().open(
        &req.path,
        OpenOptions::new().create(true).truncate(true).write(true),
    )?;
    ctx.storage.temp().merge(&req.chunk_ids, target_file)?;
    Ok(Response::ok().payload(true))
}

// ==================== 下载/读取 ====================

/// 下载文件（浏览器下载）
#[utoipa::path(
    get,
    path = "/download",
    params(("path" = String, Query)),
    responses(
        (status = 200, description = "文件流", body = Vec<u8>),
        (status = 404, description = "文件不存在")
    )
)]
pub async fn download(
    State(ctx): State<Context>,
    Claims(_user): Claims<LoggedUser>,
    Query(q): Query<PathQuery>,
) -> impl IntoResponse {
    let file_handle = ctx
        .storage
        .user()
        .open(&q.path, OpenOptions::new().read(true));

    match file_handle {
        Ok(f) => make_file_response(File::from_std(f), basename(&q.path)).into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ContentResponse {
    /// 文件文本内容
    pub content: String,
    /// 文件大小（字节）
    pub size: u64,
}

/// 最大在线编辑文件大小: 3 MiB
const MAX_EDIT_FILE_SIZE: u64 = 3 * 1024 * 1024;

/// 读取文件内容为 text（供编辑器使用，超过 3 MiB 返回错误）
#[utoipa::path(
    get,
    path = "/content",
    params(("path" = String, Query)),
    responses((status = OK, body = Response<ContentResponse>))
)]
pub async fn content(
    State(ctx): State<Context>,
    Claims(_user): Claims<LoggedUser>,
    Query(q): Query<PathQuery>,
) -> Result<Response<ContentResponse>, AppError> {
    let metadata = ctx.storage.user().metadata(&q.path)?;
    if !metadata.is_file() {
        return Err(anyhow::anyhow!("not a file").into());
    }
    let size = metadata.len();
    if size > MAX_EDIT_FILE_SIZE {
        return Err(anyhow::anyhow!(
            "file too large ({:.2} MiB), max {} MiB allowed for editing",
            size as f64 / 1024.0 / 1024.0,
            MAX_EDIT_FILE_SIZE / 1024 / 1024
        )
        .into());
    }
    let bytes = ctx.storage.user().read(&q.path)?;
    let content =
        String::from_utf8(bytes).map_err(|e| anyhow::anyhow!("file is not valid UTF-8: {}", e))?;
    Ok(Response::ok().payload(ContentResponse { content, size }))
}

// ==================== 日志文件 ====================

#[utoipa::path(
    get,
    path = "/log/{file}",
    responses(
        (status = 200, description = "日志文件内容", body = Vec<u8>),
        (status = 404, description = "文件不存在")
    )
)]
pub async fn download_log_file(
    State(ctx): State<Context>,
    Claims(_user): Claims<LoggedUser>,
    axum::extract::Path(file): axum::extract::Path<String>,
) -> impl IntoResponse {
    let file_handle = ctx.storage.log().open(&file, OpenOptions::new().read(true));

    match file_handle {
        Ok(f) => make_file_response(File::from_std(f), &file).into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}
