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
#[cfg(test)]
use boa_engine::module::IdleModuleLoader;
use boa_engine::{
    Context, JsError, JsResult, JsValue, Module, Source,
    builtins::promise::PromiseState,
    job::{JobExecutor, SimpleJobExecutor},
    module::SimpleModuleLoader,
};
use boa_gc::Gc;
use cache::ScriptCache;
use dns_response::DnsResponseCell;
use http_client::ScriptHttpClient;
use http_response::HttpResponseCell;
use std::{cell::RefCell, path::Path, rc::Rc};

#[cfg(test)]
pub fn create_context() -> (Context, Rc<SimpleJobExecutor>) {
    let executor = Rc::new(SimpleJobExecutor::new());
    let context = Context::builder()
        .job_executor(executor.clone())
        .module_loader(Rc::new(IdleModuleLoader))
        .build()
        .expect("failed to create JavaScript context");
    (context, executor)
}

pub fn create_module_context(
    module_root: &Path,
) -> JsResult<(Context, Rc<SimpleJobExecutor>, Rc<SimpleModuleLoader>)> {
    let executor = Rc::new(SimpleJobExecutor::new());
    let module_loader = Rc::new(SimpleModuleLoader::new(module_root)?);
    let context = Context::builder()
        .job_executor(executor.clone())
        .module_loader(module_loader.clone())
        .build()?;
    Ok((context, executor, module_loader))
}

async fn evaluate_parsed_module(
    module: Module,
    context: &mut Context,
    executor: Rc<SimpleJobExecutor>,
) -> JsResult<serde_json::Value> {
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

#[cfg(test)]
pub async fn evaluate_module(
    source: &str,
    context: &mut Context,
    executor: Rc<SimpleJobExecutor>,
) -> JsResult<serde_json::Value> {
    let module = Module::parse(Source::from_bytes(source.as_bytes()), None, context)?;
    evaluate_parsed_module(module, context, executor).await
}

pub async fn evaluate_module_from_path(
    source: &str,
    source_path: &Path,
    context: &mut Context,
    executor: Rc<SimpleJobExecutor>,
    module_loader: Rc<SimpleModuleLoader>,
) -> JsResult<serde_json::Value> {
    let module = Module::parse(
        Source::from_bytes(source.as_bytes()).with_path(source_path),
        None,
        context,
    )?;

    // The entry module is parsed by the dispatcher instead of the loader. Register it so
    // imports that form a cycle back to the entry resolve to the same module record.
    module_loader.insert(source_path.to_path_buf(), module.clone());

    evaluate_parsed_module(module, context, executor).await
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
    use std::{
        fs,
        path::{Path, PathBuf},
        sync::atomic::{AtomicU64, Ordering},
    };

    use boa_engine::{JsValue, js_string, property::Attribute};
    use serde_json::json;

    use super::{
        create_context, create_module_context, evaluate_module, evaluate_module_from_path,
    };

    static NEXT_TEMP_ROOT: AtomicU64 = AtomicU64::new(0);

    struct TempModuleRoot {
        path: PathBuf,
    }

    impl TempModuleRoot {
        fn new() -> Self {
            let id = NEXT_TEMP_ROOT.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir()
                .join(format!("xss-receiver-modules-{}-{id}", std::process::id()));
            fs::create_dir_all(&path).unwrap();
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }

        fn write(&self, path: &str, source: &str) {
            self.write_bytes(path, source.as_bytes());
        }

        fn write_bytes(&self, path: &str, source: &[u8]) {
            let path = self.path.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, source).unwrap();
        }
    }

    impl Drop for TempModuleRoot {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.path).unwrap();
        }
    }

    fn run_module(source: &str) -> boa_engine::JsResult<serde_json::Value> {
        let (mut context, executor) = create_context();
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(evaluate_module(source, &mut context, executor))
    }

    fn run_storage_module(
        root: &TempModuleRoot,
        entry_path: &str,
        source: &str,
    ) -> boa_engine::JsResult<serde_json::Value> {
        root.write(entry_path, source);
        let entry_path = fs::canonicalize(root.path().join(entry_path)).unwrap();
        let (mut context, executor, module_loader) = create_module_context(root.path()).unwrap();
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(evaluate_module_from_path(
                source,
                &entry_path,
                &mut context,
                executor,
                module_loader,
            ))
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

    #[test]
    fn storage_modules_support_relative_and_root_imports() {
        let root = TempModuleRoot::new();
        root.write("shared/base.js", "export const base = 40;");
        root.write(
            "handlers/lib/helper.hjs",
            r#"
                import { base } from "shared/base.js";
                export default base + 1;
            "#,
        );

        let value = run_storage_module(
            &root,
            "handlers/main.hjs",
            r#"
                import helper from "./lib/helper.hjs";
                export default { answer: helper + 1 };
            "#,
        )
        .unwrap();

        assert_eq!(value, json!({ "answer": 42 }));
    }

    #[test]
    fn dynamic_import_waits_for_dependency_top_level_await() {
        let root = TempModuleRoot::new();
        root.write(
            "shared/dynamic.djs",
            r#"
                const value = await Promise.resolve(42);
                export { value };
            "#,
        );

        let value = run_storage_module(
            &root,
            "main.hjs",
            r#"
                const dependency = await import("shared/dynamic.djs");
                export default { value: dependency.value };
            "#,
        )
        .unwrap();

        assert_eq!(value, json!({ "value": 42 }));
    }

    #[test]
    fn static_import_graph_shares_one_module_instance() {
        let root = TempModuleRoot::new();
        root.write(
            "once.js",
            r#"
                globalThis.moduleRuns = (globalThis.moduleRuns || 0) + 1;
                export const runs = globalThis.moduleRuns;
            "#,
        );
        root.write("left.js", "export { runs } from './once.js';");
        root.write("right.js", "export { runs } from './once.js';");

        let value = run_storage_module(
            &root,
            "main.hjs",
            r#"
                import { runs as leftRuns } from "./left.js";
                import { runs as rightRuns } from "./right.js";
                export default { leftRuns, rightRuns, total: globalThis.moduleRuns };
            "#,
        )
        .unwrap();

        assert_eq!(value, json!({ "leftRuns": 1, "rightRuns": 1, "total": 1 }));
    }

    #[test]
    fn imported_extension_does_not_change_the_calling_context() {
        let root = TempModuleRoot::new();
        root.write("dns-helper.djs", "export default runtimeKind;");
        let source = r#"
            import runtimeKind from "./dns-helper.djs";
            export default runtimeKind;
        "#;
        root.write("main.hjs", source);

        let entry_path = fs::canonicalize(root.path().join("main.hjs")).unwrap();
        let (mut context, executor, module_loader) = create_module_context(root.path()).unwrap();
        context
            .register_global_property(
                js_string!("runtimeKind"),
                JsValue::from(js_string!("http")),
                Attribute::READONLY,
            )
            .unwrap();

        let value = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(evaluate_module_from_path(
                source,
                &entry_path,
                &mut context,
                executor,
                module_loader,
            ))
            .unwrap();

        assert_eq!(value, json!("http"));
    }

    #[test]
    fn entry_module_is_registered_for_cycles() {
        let root = TempModuleRoot::new();
        root.write(
            "dep.js",
            r#"
                import { fromMain } from "./main.hjs";
                export function fromDependency() { return `${fromMain()}-dependency`; }
            "#,
        );

        let value = run_storage_module(
            &root,
            "main.hjs",
            r#"
                import { fromDependency } from "./dep.js";
                export function fromMain() { return "main"; }
                export default fromDependency();
            "#,
        )
        .unwrap();

        assert_eq!(value, json!("main-dependency"));
    }

    #[test]
    fn new_context_reloads_changed_dependencies() {
        let root = TempModuleRoot::new();
        let entry = r#"
            import value from "./dependency.js";
            export default value;
        "#;

        root.write("dependency.js", "export default 1;");
        assert_eq!(
            run_storage_module(&root, "main.hjs", entry).unwrap(),
            json!(1)
        );

        root.write("dependency.js", "export default 2;");
        assert_eq!(
            run_storage_module(&root, "main.hjs", entry).unwrap(),
            json!(2)
        );
    }

    #[test]
    fn storage_module_errors_include_resolution_and_parse_failures() {
        let root = TempModuleRoot::new();

        let traversal = run_storage_module(
            &root,
            "handlers/main.hjs",
            "import '../../outside.js'; export default null;",
        )
        .unwrap_err();
        assert!(traversal.to_string().contains("outside the module root"));

        let missing = run_storage_module(
            &root,
            "main.hjs",
            "import './missing.js'; export default null;",
        )
        .unwrap_err();
        assert!(missing.to_string().contains("could not open file"));

        root.write("broken.js", "export const = 1;");
        let syntax = run_storage_module(
            &root,
            "main.hjs",
            "import './broken.js'; export default null;",
        )
        .unwrap_err();
        assert!(syntax.to_string().contains("could not parse module"));

        root.write_bytes("invalid.js", &[0xff]);
        let encoding = run_storage_module(
            &root,
            "main.hjs",
            "import './invalid.js'; export default null;",
        )
        .unwrap_err();
        assert!(encoding.to_string().contains("could not parse module"));

        root.write("throws.js", "throw new Error('dependency failed');");
        let execution = run_storage_module(
            &root,
            "main.hjs",
            "import './throws.js'; export default null;",
        )
        .unwrap_err();
        assert!(execution.to_string().contains("dependency failed"));
    }
}
