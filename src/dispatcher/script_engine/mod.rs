pub mod cache;
pub mod dns_request;
pub mod dns_response;
mod helpers;
pub mod http_client;
pub mod http_request;
pub mod http_response;
mod module_loader;
mod runtime;
pub mod storage;
pub mod utils;

use crate::{dispatcher::DnsRequest, storage::UserStorage, utils::parsed_request::ParsedRequest};
use cache::ScriptCache;
use http_client::ScriptHttpClient;
use module_loader::StorageModuleLoader;
use rquickjs::{AsyncContext, AsyncRuntime, Ctx, Function, Module, Object, Persistent, Value};
pub use runtime::ScriptError;
use std::{cell::RefCell, path::Path, rc::Rc, time::Duration};

#[cfg(test)]
pub async fn create_context() -> (AsyncRuntime, AsyncContext) {
    let runtime = AsyncRuntime::new().unwrap();
    let context = AsyncContext::full(&runtime).await.unwrap();
    (runtime, context)
}

pub async fn create_module_context(
    root: &Path,
) -> Result<(AsyncRuntime, AsyncContext), ScriptError> {
    let runtime = AsyncRuntime::new()?;
    let resolver =
        StorageModuleLoader::new(root).map_err(|error| ScriptError(error.to_string()))?;
    let loader = StorageModuleLoader::new(root).map_err(|error| ScriptError(error.to_string()))?;
    runtime.set_loader(resolver, loader).await;
    let context = AsyncContext::full(&runtime).await?;
    Ok((runtime, context))
}

async fn evaluate_module_inner(
    source: &str,
    name: &str,
    context: &AsyncContext,
) -> Result<serde_json::Value, ScriptError> {
    let (namespace, promise, stringify) = context
        .with(|ctx| {
            let result = (|| -> rquickjs::Result<_> {
                // Retain the intrinsic before user code can replace JSON.stringify.
                let json: Object = ctx.globals().get("JSON")?;
                let stringify: Function = json.get("stringify")?;
                let declared = Module::declare(ctx.clone(), name, source)?;
                let (module, promise) = declared.eval()?;
                Ok((
                    Persistent::save(&ctx, module.namespace()?),
                    Persistent::save(&ctx, promise),
                    Persistent::save(&ctx, stringify),
                ))
            })();
            result.map_err(|error| ScriptError::from_js(&ctx, error))
        })
        .await?;

    runtime::run_jobs(context.runtime()).await?;

    context
        .with(|ctx| {
            let result = (|| -> rquickjs::Result<_> {
                let promise = promise.restore(&ctx)?;
                promise.result::<Value>().ok_or_else(|| {
                    rquickjs::Exception::throw_message(
                        &ctx,
                        "module evaluation remained pending after the job queue drained",
                    )
                })??;
                let namespace = namespace.restore(&ctx)?;
                let value: Value = namespace.get("default")?;
                let stringify = stringify.restore(&ctx)?;
                let serialized: Value = stringify.call((value,))?;
                match serialized.as_string() {
                    Some(value) => Ok(Some(value.to_string()?)),
                    None => Ok(None),
                }
            })();
            let serialized = result.map_err(|error| ScriptError::from_js(&ctx, error))?;
            match serialized {
                Some(value) => serde_json::from_str(&value).map_err(|error| {
                    ScriptError(format!(
                        "could not serialize module default export: {error}"
                    ))
                }),
                None => Ok(serde_json::Value::Null),
            }
        })
        .await
}

#[cfg(test)]
pub async fn evaluate_module(
    source: &str,
    context: &AsyncContext,
) -> Result<serde_json::Value, ScriptError> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT_ENTRY: AtomicU64 = AtomicU64::new(0);
    let name = format!("test-{}.js", NEXT_ENTRY.fetch_add(1, Ordering::Relaxed));
    runtime::with_timeout(
        context,
        Duration::from_secs(5),
        evaluate_module_inner(source, &name, context),
    )
    .await
}

pub async fn evaluate_module_from_path(
    source: &str,
    path: &Path,
    context: &AsyncContext,
    timeout: Duration,
) -> Result<serde_json::Value, ScriptError> {
    let name = path
        .to_str()
        .ok_or_else(|| ScriptError("module path must be UTF-8".into()))?;
    runtime::with_timeout(
        context,
        timeout,
        evaluate_module_inner(source, name, context),
    )
    .await
}

pub fn register_http_vars_to_context(
    ctx: &Ctx<'_>,
    request: &ParsedRequest,
    user_storage: UserStorage,
    cache: ScriptCache,
    http_client: ScriptHttpClient,
) -> rquickjs::Result<Rc<RefCell<http_response::HttpResponse>>> {
    let response = http_response::register_response_to_context(ctx, user_storage.clone())?;
    http_request::register_http_request_to_context(ctx, request)?;
    storage::register_storage_to_context(ctx, user_storage)?;
    cache::register_cache_to_context(ctx, cache)?;
    http_client::register_http_client_to_context(ctx, http_client)?;
    utils::register_utils_to_context(ctx)?;
    Ok(response)
}

pub fn register_dns_vars_to_context(
    ctx: &Ctx<'_>,
    request: &DnsRequest,
    user_storage: UserStorage,
    cache: ScriptCache,
    http_client: ScriptHttpClient,
) -> rquickjs::Result<Rc<RefCell<dns_response::ScriptDnsResponse>>> {
    let response = dns_response::register_dns_response_to_context(ctx)?;
    dns_request::register_dns_request_to_context(ctx, request)?;
    storage::register_storage_to_context(ctx, user_storage)?;
    cache::register_cache_to_context(ctx, cache)?;
    http_client::register_http_client_to_context(ctx, http_client)?;
    utils::register_utils_to_context(ctx)?;
    Ok(response)
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        sync::atomic::{AtomicU64, Ordering},
    };

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

    fn run_module(source: &str) -> Result<serde_json::Value, super::ScriptError> {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let (_runtime, context) = create_context().await;
                evaluate_module(source, &context).await
            })
    }

    fn run_storage_module(
        root: &TempModuleRoot,
        entry_path: &str,
        source: &str,
    ) -> Result<serde_json::Value, super::ScriptError> {
        root.write(entry_path, source);
        let entry_path = fs::canonicalize(root.path().join(entry_path)).unwrap();
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let (_runtime, context) = create_module_context(root.path()).await.unwrap();
                evaluate_module_from_path(
                    source,
                    &entry_path,
                    &context,
                    std::time::Duration::from_secs(5),
                )
                .await
            })
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
    fn default_export_serializes_nested_object_and_array_proxies() {
        let value = run_module(
            r#"
                const item = new Proxy({ value: 42 }, {});
                const items = new Proxy([item, item], {});
                export default { items, item };
            "#,
        )
        .unwrap();
        assert_eq!(
            value,
            json!({ "items": [{ "value": 42 }, { "value": 42 }], "item": { "value": 42 } })
        );
    }

    #[test]
    fn default_export_uses_original_json_serializer_and_standard_json_rules() {
        let value = run_module(
            r#"
                JSON.stringify = () => '"replaced"';
                globalThis.JSON = null;
                const result = { value: 42, omitted: undefined, method() {} };
                result.values = [undefined, NaN, Infinity];
                Object.defineProperty(result, 'hidden', { value: 'hidden' });
                export default result;
            "#,
        )
        .unwrap();
        assert_eq!(value, json!({ "value": 42, "values": [null, null, null] }));
    }

    #[test]
    fn default_export_rejects_cycles_bigints_and_revoked_proxies() {
        for source in [
            "const x = {}; x.self = x; export default x;",
            "export default { value: 1n };",
            "const { proxy, revoke } = Proxy.revocable({}, {}); revoke(); export default proxy;",
        ] {
            assert!(run_module(source).is_err());
        }
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
        let value = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let (_runtime, context) = create_module_context(root.path()).await.unwrap();
                context
                    .with(|ctx| ctx.globals().set("runtimeKind", "http"))
                    .await
                    .unwrap();
                evaluate_module_from_path(
                    source,
                    &entry_path,
                    &context,
                    std::time::Duration::from_secs(5),
                )
                .await
                .unwrap()
            });

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
        let outside = TempModuleRoot::new();
        outside.write("secret.js", "export default 42;");
        let outside_name = outside.path().file_name().unwrap().to_str().unwrap();

        let traversal = run_storage_module(
            &root,
            "handlers/main.hjs",
            &format!("import '../../{outside_name}/secret.js'; export default null;"),
        )
        .unwrap_err();
        assert!(traversal.to_string().contains("outside the module root"));

        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(
                outside.path().join("secret.js"),
                root.path().join("escape.js"),
            )
            .unwrap();
            let escape =
                run_storage_module(&root, "main.hjs", "export { default } from './escape.js';")
                    .unwrap_err();
            assert!(escape.to_string().contains("outside the module root"));
        }

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
