use boa_engine::object::builtins::{JsArray, JsUint8Array};
use boa_engine::{
    Context, JsNativeError, JsObject, JsResult, JsValue, NativeFunction, js_string,
    object::ObjectInitializer, property::Attribute,
};
use boa_gc::{Finalize, Gc, Trace, empty_trace};

use crate::storage::UserStorage;

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
fn get_storage_from_context(ctx: &mut Context) -> JsResult<Gc<UserStorageCell>> {
    ensure_exists(
        ctx.get_data::<Gc<UserStorageCell>>().cloned(),
        "failed to get storage from context",
    )
}

/// 将 FileInfo 转换为 JS 对象 `{ name, size, modifiedTime }`
fn file_info_to_js_object(
    name: &str,
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

    // storage.list(directory: string) -> Array<{ name, size, modifiedTime }>
    object_builder.function(
        NativeFunction::from_copy_closure(move |_this, args, ctx| {
            check_argument_count(args, 1)?;
            let directory = ensure_exists(args[0].as_string(), "argument 0 must be a string")?
                .to_std_string_lossy();

            let storage = get_storage_from_context(ctx)?;
            let files = storage.storage.list_directory(&directory).map_err(|e| {
                JsNativeError::error().with_message(format!("storage.list failed: {}", e))
            })?;

            let js_array = JsArray::new(ctx);
            for (i, file) in files.iter().enumerate() {
                let obj = file_info_to_js_object(&file.name, file.size, file.modified_time, ctx)?;
                js_array.set(i, obj, false, ctx)?;
            }
            Ok(js_array.into())
        }),
        js_string!("list"),
        1,
    );

    // storage.list_all() -> { [directory: string]: Array<{ name, size, modifiedTime }> }
    object_builder.function(
        NativeFunction::from_copy_closure(move |_this, _args, ctx| {
            let storage = get_storage_from_context(ctx)?;
            let dirs = storage.storage.list_all_directory().map_err(|e| {
                JsNativeError::error().with_message(format!("storage.list_all failed: {}", e))
            })?;

            let result_obj = JsObject::with_null_proto();
            for (dir_name, files) in &dirs {
                let js_array = JsArray::new(ctx);
                for (i, file) in files.iter().enumerate() {
                    let obj =
                        file_info_to_js_object(&file.name, file.size, file.modified_time, ctx)?;
                    js_array.set(i, obj, false, ctx)?;
                }
                result_obj.set(js_string!(dir_name.as_str()), js_array, false, ctx)?;
            }
            Ok(result_obj.into())
        }),
        js_string!("list_all"),
        0,
    );

    // storage.create_directory(directory: string) -> void
    object_builder.function(
        NativeFunction::from_copy_closure(move |_this, args, ctx| {
            check_argument_count(args, 1)?;
            let directory = ensure_exists(args[0].as_string(), "argument 0 must be a string")?
                .to_std_string_lossy();

            let storage = get_storage_from_context(ctx)?;
            storage.storage.new_directory(&directory).map_err(|e| {
                JsNativeError::error()
                    .with_message(format!("storage.create_directory failed: {}", e))
            })?;
            Ok(JsValue::undefined())
        }),
        js_string!("create_directory"),
        1,
    );

    // storage.read_file(directory: string, filename: string) -> Uint8Array
    object_builder.function(
        NativeFunction::from_copy_closure(move |_this, args, ctx| {
            check_argument_count(args, 2)?;
            let directory = ensure_exists(args[0].as_string(), "argument 0 must be a string")?
                .to_std_string_lossy();
            let filename = ensure_exists(args[1].as_string(), "argument 1 must be a string")?
                .to_std_string_lossy();

            let storage = get_storage_from_context(ctx)?;
            let content = storage
                .storage
                .read_file(&directory, &filename)
                .map_err(|e| {
                    JsNativeError::error().with_message(format!("storage.read_file failed: {}", e))
                })?;

            let array = JsUint8Array::from_iter(content, ctx)?;
            Ok(array.into())
        }),
        js_string!("read_file"),
        2,
    );

    // storage.write_file(directory: string, filename: string, content: string | ArrayBuffer) -> void
    object_builder.function(
        NativeFunction::from_copy_closure(move |_this, args, ctx| {
            check_argument_count(args, 3)?;
            let directory = ensure_exists(args[0].as_string(), "argument 0 must be a string")?
                .to_std_string_lossy();
            let filename = ensure_exists(args[1].as_string(), "argument 1 must be a string")?
                .to_std_string_lossy();
            let content = read_u8_array_from_js_value(&args[2], ctx)?;

            let storage = get_storage_from_context(ctx)?;
            storage
                .storage
                .write_file(&directory, &filename, &content)
                .map_err(|e| {
                    JsNativeError::error().with_message(format!("storage.write_file failed: {}", e))
                })?;
            Ok(JsValue::undefined())
        }),
        js_string!("write_file"),
        3,
    );

    // storage.append_file(directory: string, filename: string, content: string | ArrayBuffer) -> void
    object_builder.function(
        NativeFunction::from_copy_closure(move |_this, args, ctx| {
            check_argument_count(args, 3)?;
            let directory = ensure_exists(args[0].as_string(), "argument 0 must be a string")?
                .to_std_string_lossy();
            let filename = ensure_exists(args[1].as_string(), "argument 1 must be a string")?
                .to_std_string_lossy();
            let content = read_u8_array_from_js_value(&args[2], ctx)?;

            let storage = get_storage_from_context(ctx)?;
            storage
                .storage
                .append_file(&directory, &filename, &content)
                .map_err(|e| {
                    JsNativeError::error()
                        .with_message(format!("storage.append_file failed: {}", e))
                })?;
            Ok(JsValue::undefined())
        }),
        js_string!("append_file"),
        3,
    );

    // storage.delete(directory: string, filename?: string) -> void
    object_builder.function(
        NativeFunction::from_copy_closure(move |_this, args, ctx| {
            check_argument_count(args, 1)?;
            let directory = ensure_exists(args[0].as_string(), "argument 0 must be a string")?
                .to_std_string_lossy();
            // filename 是可选参数
            let filename_owned: Option<String> = args.get(1).and_then(|v| {
                if v.is_undefined() || v.is_null() {
                    None
                } else {
                    v.as_string().map(|s| s.to_std_string_lossy())
                }
            });
            let filename_ref = filename_owned.as_deref();

            let storage = get_storage_from_context(ctx)?;
            storage
                .storage
                .delete(&directory, filename_ref)
                .map_err(|e| {
                    JsNativeError::error().with_message(format!("storage.delete failed: {}", e))
                })?;
            Ok(JsValue::undefined())
        }),
        js_string!("delete"),
        1,
    );

    // storage.rename(directory, filename | null, new_directory, new_filename | null) -> void
    object_builder.function(
        NativeFunction::from_copy_closure(move |_this, args, ctx| {
            check_argument_count(args, 4)?;
            let directory = ensure_exists(args[0].as_string(), "argument 0 must be a string")?
                .to_std_string_lossy();
            let filename_owned: Option<String> = {
                let v = &args[1];
                if v.is_null() || v.is_undefined() {
                    None
                } else {
                    Some(
                        ensure_exists(v.as_string(), "argument 1 must be a string or null")?
                            .to_std_string_lossy(),
                    )
                }
            };
            let new_directory = ensure_exists(args[2].as_string(), "argument 2 must be a string")?
                .to_std_string_lossy();
            let new_filename_owned: Option<String> = {
                let v = &args[3];
                if v.is_null() || v.is_undefined() {
                    None
                } else {
                    Some(
                        ensure_exists(v.as_string(), "argument 3 must be a string or null")?
                            .to_std_string_lossy(),
                    )
                }
            };

            let storage = get_storage_from_context(ctx)?;
            storage
                .storage
                .rename(
                    &directory,
                    filename_owned.as_deref(),
                    &new_directory,
                    new_filename_owned.as_deref(),
                )
                .map_err(|e| {
                    JsNativeError::error().with_message(format!("storage.rename failed: {}", e))
                })?;
            Ok(JsValue::undefined())
        }),
        js_string!("rename"),
        4,
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
