// 模块声明
pub mod cache;
pub mod dns_request;
pub mod dns_response;
mod helpers;
pub mod http_client;
pub mod http_request;
pub mod http_response;
pub mod storage;
pub mod utils;

use crate::dispatcher::DnsRequest;
use crate::storage::UserStorage;
use crate::utils::parsed_request::ParsedRequest;
use boa_engine::{
    Context, JsError, JsResult, JsValue, Module, Source,
    builtins::promise::PromiseState,
    job::{JobExecutor, SimpleJobExecutor},
    module::IdleModuleLoader,
};
use boa_gc::Gc;
use cache::ScriptCache;
use dns_response::DnsResponseCell;
use http_client::ScriptHttpClient;
use http_response::HttpResponseCell;
use std::{cell::RefCell, rc::Rc};

pub fn create_context() -> (Context, Rc<SimpleJobExecutor>) {
    let executor = Rc::new(SimpleJobExecutor::new());
    let context = Context::builder()
        .job_executor(executor.clone())
        .module_loader(Rc::new(IdleModuleLoader))
        .build()
        .expect("failed to create JavaScript context");
    (context, executor)
}

pub async fn evaluate_module(
    source: &str,
    context: &mut Context,
    executor: Rc<SimpleJobExecutor>,
) -> JsResult<serde_json::Value> {
    let module = Module::parse(Source::from_bytes(source.as_bytes()), None, context)?;
    let promise = module.load_link_evaluate(context);

    executor.run_jobs_async(&RefCell::new(context)).await?;

    match promise.state() {
        PromiseState::Fulfilled(_) => {}
        PromiseState::Rejected(error) => return Err(JsError::from_opaque(error)),
        PromiseState::Pending => {
            return Err(JsError::from_opaque(JsValue::from(boa_engine::js_string!(
                "module evaluation remained pending after the job queue drained"
            ))));
        }
    }

    Ok(module
        .get_value(boa_engine::js_string!("default"), context)?
        .to_json(context)?
        .unwrap_or(serde_json::Value::Null))
}

/// 注册所有变量到 JS 上下文的主入口函数
///
/// 该函数会注册以下全局对象：
/// - request: 请求对象，包含 method, path, headers, query, body 等
/// - response: 响应对象，包含 send, sendStatus, sendHeader 等方法
/// - storage: 用户文件存储对象，包含 list, list_all, create_directory, write_file, append_file, delete, rename
/// - cache: 进程内共享缓存
/// - http: 服务端出站 HTTP 客户端
/// - utils: 工具函数，包含 base64Encode, base64Decode, urlEncode, urlDecode
///
/// # 返回值
/// 返回 ResponseCell 的 Gc 指针，用于后续获取响应数据
pub fn register_http_vars_to_context(
    context: &mut Context,
    request: &ParsedRequest,
    user_storage: UserStorage,
    cache: ScriptCache,
    http_client: ScriptHttpClient,
) -> Gc<HttpResponseCell> {
    let response_cell = http_response::register_response_to_context(context);
    http_request::register_http_request_to_context(context, request);
    storage::register_storage_to_context(context, user_storage);
    cache::register_cache_to_context(context, cache);
    http_client::register_http_client_to_context(context, http_client);
    utils::register_utils_to_context(context);
    response_cell
}

pub fn register_dns_vars_to_context(
    context: &mut Context,
    request: &DnsRequest,
    user_storage: UserStorage,
    cache: ScriptCache,
    http_client: ScriptHttpClient,
) -> Gc<DnsResponseCell> {
    let response_cell = dns_response::register_dns_response_to_context(context);
    dns_request::register_dns_request_to_context(context, request);
    storage::register_storage_to_context(context, user_storage);
    cache::register_cache_to_context(context, cache);
    http_client::register_http_client_to_context(context, http_client);
    utils::register_utils_to_context(context);
    response_cell
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{create_context, evaluate_module};

    fn run_module(source: &str) -> boa_engine::JsResult<serde_json::Value> {
        let (mut context, executor) = create_context();
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(evaluate_module(source, &mut context, executor))
    }

    #[test]
    fn top_level_await_exports_default_value() {
        let value = run_module(
            r#"
                const answer = await Promise.resolve(42);
                export default { answer };
            "#,
        )
        .unwrap();

        assert_eq!(value, json!({ "answer": 42 }));
    }

    #[test]
    fn missing_default_export_returns_null() {
        let value = run_module("await Promise.resolve();").unwrap();
        assert_eq!(value, serde_json::Value::Null);
    }

    #[test]
    fn rejected_top_level_await_is_an_error() {
        let error = run_module("await Promise.reject(new Error('failed'));").unwrap_err();
        assert!(error.to_string().contains("failed"));
    }

    #[test]
    fn imports_are_rejected_by_idle_loader() {
        assert!(run_module("import value from './other.js'; export default value;").is_err());
        assert!(run_module("await import('./other.js'); export default null;").is_err());
    }
}
