use boa_engine::object::builtins::{JsArray, JsUint8Array};
use boa_engine::{
    Context, JsNativeError, JsObject, JsResult, JsValue, NativeFunction, js_string,
    object::ObjectInitializer, property::Attribute,
};
use boa_gc::{Finalize, Gc, Trace, empty_trace};

use crate::storage::{EntryKind, UserStorage};

use super::helpers::{check_argument_count, ensure_exists, read_u8_array_from_js_value};

/// 将 UserStorage 包装为可存入 boa 上下文的数据
pub struct UserStorageCell {
    pub storage: UserStorage,
}

impl Finalize for UserStorageCell {}

// SAFETY: UserStorage 只包含 PathBuf，不存储任何来自 JS 引擎的对象，不需要实现 trace
unsafe impl Trace for UserStorageCell {
    empty_trace!();
}

// SAFETY: JS 引擎是单线程的
unsafe impl Sync for UserStorageCell {}

/// 从 boa 上下文中取出 UserStorage
pub(crate) fn get_storage_from_context(ctx: &mut Context) -> JsResult<Gc<UserStorageCell>> {
    ensure_exists(
        ctx.get_data::<Gc<UserStorageCell>>().cloned(),
        "failed to get storage from context",
    )
}

/// 将 Entry 转换为 JS 对象 `{ name, kind, size, modifiedTime }`
fn entry_to_js_object(
    name: &str,
    kind: EntryKind,
    size: u64,
    modified_time: i64,
    ctx: &mut Context,
) -> JsResult<JsObject> {
    let obj = JsObject::with_null_proto();
    obj.set(
        js_string!("name"),
        JsValue::from(js_string!(name)),
        false,
        ctx,
    )?;
    let kind_str = match kind {
        EntryKind::File => "file",
        EntryKind::Directory => "directory",
    };
    obj.set(
        js_string!("kind"),
        JsValue::from(js_string!(kind_str)),
        false,
        ctx,
    )?;
    obj.set(js_string!("size"), JsValue::from(size as f64), false, ctx)?;
    obj.set(
        js_string!("modifiedTime"),
        JsValue::from(modified_time as f64),
        false,
        ctx,
    )?;
    Ok(obj)
}

/// 注册 storage 全局对象到 JS 上下文
pub fn register_storage_to_context(context: &mut Context, user_storage: UserStorage) {
    let data = Gc::new(UserStorageCell {
        storage: user_storage,
    });
    context.insert_data(data.clone());

    let mut object_builder = ObjectInitializer::new(context);

    // storage.list(path: string) -> Array<{ name, kind, size, modifiedTime }>
    object_builder.function(
        NativeFunction::from_copy_closure(move |_this, args, ctx| {
            check_argument_count(args, 1)?;
            let path = ensure_exists(args[0].as_string(), "argument 0 must be a string")?
                .to_std_string_lossy();

            let storage = get_storage_from_context(ctx)?;
            let entries = storage.storage.list(&path).map_err(|e| {
                JsNativeError::error().with_message(format!("storage.list failed: {}", e))
            })?;

            let js_array = JsArray::new(ctx);
            for (i, entry) in entries.iter().enumerate() {
                let obj = entry_to_js_object(
                    &entry.name,
                    entry.kind,
                    entry.size,
                    entry.modified_time,
                    ctx,
                )?;
                js_array.set(i, obj, false, ctx)?;
            }
            Ok(js_array.into())
        }),
        js_string!("list"),
        1,
    );

    // storage.listAll() -> Array<string>
    object_builder.function(
        NativeFunction::from_copy_closure(move |_this, _args, ctx| {
            let storage = get_storage_from_context(ctx)?;
            let files = storage.storage.list_all_files().map_err(|e| {
                JsNativeError::error().with_message(format!("storage.listAll failed: {}", e))
            })?;

            let js_array = JsArray::new(ctx);
            for (i, path) in files.iter().enumerate() {
                js_array.set(i, JsValue::from(js_string!(path.as_str())), false, ctx)?;
            }
            Ok(js_array.into())
        }),
        js_string!("listAll"),
        0,
    );

    // storage.mkdir(path: string) -> undefined
    object_builder.function(
        NativeFunction::from_copy_closure(move |_this, args, ctx| {
            check_argument_count(args, 1)?;
            let path = ensure_exists(args[0].as_string(), "argument 0 must be a string")?
                .to_std_string_lossy();

            let storage = get_storage_from_context(ctx)?;
            storage.storage.mkdir(&path).map_err(|e| {
                JsNativeError::error().with_message(format!("storage.mkdir failed: {}", e))
            })?;
            Ok(JsValue::undefined())
        }),
        js_string!("mkdir"),
        1,
    );

    // storage.read(path: string) -> Uint8Array
    object_builder.function(
        NativeFunction::from_copy_closure(move |_this, args, ctx| {
            check_argument_count(args, 1)?;
            let path = ensure_exists(args[0].as_string(), "argument 0 must be a string")?
                .to_std_string_lossy();

            let storage = get_storage_from_context(ctx)?;
            let content = storage.storage.read(&path).map_err(|e| {
                JsNativeError::error().with_message(format!("storage.read failed: {}", e))
            })?;

            let array = JsUint8Array::from_iter(content, ctx)?;
            Ok(array.into())
        }),
        js_string!("read"),
        1,
    );

    // storage.write(path: string, content: string | ArrayBuffer) -> undefined
    object_builder.function(
        NativeFunction::from_copy_closure(move |_this, args, ctx| {
            check_argument_count(args, 2)?;
            let path = ensure_exists(args[0].as_string(), "argument 0 must be a string")?
                .to_std_string_lossy();
            let content = read_u8_array_from_js_value(&args[1], ctx)?;

            let storage = get_storage_from_context(ctx)?;
            storage.storage.write(&path, &content).map_err(|e| {
                JsNativeError::error().with_message(format!("storage.write failed: {}", e))
            })?;
            Ok(JsValue::undefined())
        }),
        js_string!("write"),
        2,
    );

    // storage.append(path: string, content: string | ArrayBuffer) -> undefined
    object_builder.function(
        NativeFunction::from_copy_closure(move |_this, args, ctx| {
            check_argument_count(args, 2)?;
            let path = ensure_exists(args[0].as_string(), "argument 0 must be a string")?
                .to_std_string_lossy();
            let content = read_u8_array_from_js_value(&args[1], ctx)?;

            let storage = get_storage_from_context(ctx)?;
            storage.storage.append(&path, &content).map_err(|e| {
                JsNativeError::error().with_message(format!("storage.append failed: {}", e))
            })?;
            Ok(JsValue::undefined())
        }),
        js_string!("append"),
        2,
    );

    // storage.remove(path: string) -> undefined
    object_builder.function(
        NativeFunction::from_copy_closure(move |_this, args, ctx| {
            check_argument_count(args, 1)?;
            let path = ensure_exists(args[0].as_string(), "argument 0 must be a string")?
                .to_std_string_lossy();

            let storage = get_storage_from_context(ctx)?;
            storage.storage.remove(&path).map_err(|e| {
                JsNativeError::error().with_message(format!("storage.remove failed: {}", e))
            })?;
            Ok(JsValue::undefined())
        }),
        js_string!("remove"),
        1,
    );

    // storage.rename(src: string, dst: string) -> undefined
    object_builder.function(
        NativeFunction::from_copy_closure(move |_this, args, ctx| {
            check_argument_count(args, 2)?;
            let src = ensure_exists(args[0].as_string(), "argument 0 must be a string")?
                .to_std_string_lossy();
            let dst = ensure_exists(args[1].as_string(), "argument 1 must be a string")?
                .to_std_string_lossy();

            let storage = get_storage_from_context(ctx)?;
            storage.storage.rename(&src, &dst).map_err(|e| {
                JsNativeError::error().with_message(format!("storage.rename failed: {}", e))
            })?;
            Ok(JsValue::undefined())
        }),
        js_string!("rename"),
        2,
    );

    // storage.exists(path: string) -> boolean
    object_builder.function(
        NativeFunction::from_copy_closure(move |_this, args, ctx| {
            check_argument_count(args, 1)?;
            let path = ensure_exists(args[0].as_string(), "argument 0 must be a string")?
                .to_std_string_lossy();

            let storage = get_storage_from_context(ctx)?;
            let result = storage.storage.exists(&path);
            Ok(JsValue::from(result))
        }),
        js_string!("exists"),
        1,
    );

    let object = object_builder.build();
    context
        .register_global_property(
            js_string!("storage"),
            object,
            Attribute::READONLY | Attribute::ENUMERABLE,
        )
        .expect("storage property shouldn't exist");
}
