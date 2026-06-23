// 模块声明
mod helpers;
pub mod request;
pub mod response;
pub mod storage;
pub mod utils;

use crate::storage::UserStorage;
use crate::utils::parsed_request::ParsedRequest;
use boa_engine::Context;
use boa_gc::Gc;
use response::ResponseCell;

/// 注册所有变量到 JS 上下文的主入口函数
///
/// 该函数会注册以下全局对象：
/// - request: 请求对象，包含 method, path, headers, query, body 等
/// - response: 响应对象，包含 send, sendStatus, sendHeader 等方法
/// - storage: 用户文件存储对象，包含 list, list_all, create_directory, write_file, append_file, delete, rename
/// - utils: 工具函数，包含 base64Encode, base64Decode, urlEncode, urlDecode
///
/// # 返回值
/// 返回 ResponseCell 的 Gc 指针，用于后续获取响应数据
pub fn register_vars_to_context(
    context: &mut Context,
    request: &ParsedRequest,
    user_storage: UserStorage,
) -> Gc<ResponseCell> {
    let response_cell = response::register_response_to_context(context);
    request::register_request_to_context(context, request);
    storage::register_storage_to_context(context, user_storage);
    utils::register_utils_to_context(context);
    response_cell
}
