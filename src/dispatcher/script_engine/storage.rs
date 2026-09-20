use rquickjs::{
    Array, Ctx, Exception, Function, Object, Result, TypedArray, Value, function::Rest,
    object::Property,
};

use crate::storage::{EntryKind, UserStorage};

use super::helpers::{check_argument_count, ensure_exists, read_u8_array_from_js_value};

fn entry_to_js_object<'js>(
    name: &str,
    kind: EntryKind,
    size: u64,
    modified_time: i64,
    ctx: &Ctx<'js>,
) -> Result<Object<'js>> {
    let object = Object::new_proto(ctx.clone(), None)?;
    object.set("name", name)?;
    object.set(
        "kind",
        match kind {
            EntryKind::File => "file",
            EntryKind::Directory => "directory",
        },
    )?;
    object.set("size", size as f64)?;
    object.set("modifiedTime", modified_time as f64)?;
    Ok(object)
}

pub fn register_storage_to_context<'js>(ctx: &Ctx<'js>, storage: UserStorage) -> Result<()> {
    let object = Object::new(ctx.clone())?;

    let shared = storage.clone();
    object.set(
        "list",
        Function::new(ctx.clone(), move |ctx: Ctx<'js>, args: Rest<Value<'js>>| {
            check_argument_count(&args, 1, &ctx)?;
            let path = ensure_exists(args[0].as_string(), "argument 0 must be a string", &ctx)?
                .to_string()?;
            let entries = shared.list(&path).map_err(|error| {
                Exception::throw_message(&ctx, &format!("storage.list failed: {error}"))
            })?;
            let array = Array::new(ctx.clone())?;
            for (index, entry) in entries.iter().enumerate() {
                array.set(
                    index,
                    entry_to_js_object(
                        &entry.name,
                        entry.kind,
                        entry.size,
                        entry.modified_time,
                        &ctx,
                    )?,
                )?;
            }
            Ok::<_, rquickjs::Error>(array)
        })?,
    )?;

    let shared = storage.clone();
    object.set(
        "listAll",
        Function::new(ctx.clone(), move |ctx: Ctx<'js>| {
            shared.list_all_files().map_err(|error| {
                Exception::throw_message(&ctx, &format!("storage.listAll failed: {error}"))
            })
        })?,
    )?;

    let shared = storage.clone();
    object.set(
        "mkdir",
        Function::new(ctx.clone(), move |ctx: Ctx<'js>, args: Rest<Value<'js>>| {
            check_argument_count(&args, 1, &ctx)?;
            let path = ensure_exists(args[0].as_string(), "argument 0 must be a string", &ctx)?
                .to_string()?;
            shared.mkdir(&path).map_err(|error| {
                Exception::throw_message(&ctx, &format!("storage.mkdir failed: {error}"))
            })
        })?,
    )?;

    let shared = storage.clone();
    object.set(
        "read",
        Function::new(ctx.clone(), move |ctx: Ctx<'js>, args: Rest<Value<'js>>| {
            check_argument_count(&args, 1, &ctx)?;
            let path = ensure_exists(args[0].as_string(), "argument 0 must be a string", &ctx)?
                .to_string()?;
            let content = shared.read(&path).map_err(|error| {
                Exception::throw_message(&ctx, &format!("storage.read failed: {error}"))
            })?;
            TypedArray::new(ctx, content)
        })?,
    )?;

    let shared = storage.clone();
    object.set(
        "write",
        Function::new(ctx.clone(), move |ctx: Ctx<'js>, args: Rest<Value<'js>>| {
            check_argument_count(&args, 2, &ctx)?;
            let path = ensure_exists(args[0].as_string(), "argument 0 must be a string", &ctx)?
                .to_string()?;
            let content = read_u8_array_from_js_value(&args[1], &ctx)?;
            shared.write(&path, &content).map_err(|error| {
                Exception::throw_message(&ctx, &format!("storage.write failed: {error}"))
            })
        })?,
    )?;

    let shared = storage.clone();
    object.set(
        "append",
        Function::new(ctx.clone(), move |ctx: Ctx<'js>, args: Rest<Value<'js>>| {
            check_argument_count(&args, 2, &ctx)?;
            let path = ensure_exists(args[0].as_string(), "argument 0 must be a string", &ctx)?
                .to_string()?;
            let content = read_u8_array_from_js_value(&args[1], &ctx)?;
            shared.append(&path, &content).map_err(|error| {
                Exception::throw_message(&ctx, &format!("storage.append failed: {error}"))
            })
        })?,
    )?;

    let shared = storage.clone();
    object.set(
        "remove",
        Function::new(ctx.clone(), move |ctx: Ctx<'js>, args: Rest<Value<'js>>| {
            check_argument_count(&args, 1, &ctx)?;
            let path = ensure_exists(args[0].as_string(), "argument 0 must be a string", &ctx)?
                .to_string()?;
            shared.remove(&path).map_err(|error| {
                Exception::throw_message(&ctx, &format!("storage.remove failed: {error}"))
            })
        })?,
    )?;

    let shared = storage.clone();
    object.set(
        "rename",
        Function::new(ctx.clone(), move |ctx: Ctx<'js>, args: Rest<Value<'js>>| {
            check_argument_count(&args, 2, &ctx)?;
            let src = ensure_exists(args[0].as_string(), "argument 0 must be a string", &ctx)?
                .to_string()?;
            let dst = ensure_exists(args[1].as_string(), "argument 1 must be a string", &ctx)?
                .to_string()?;
            shared.rename(&src, &dst).map_err(|error| {
                Exception::throw_message(&ctx, &format!("storage.rename failed: {error}"))
            })
        })?,
    )?;

    object.set(
        "exists",
        Function::new(ctx.clone(), move |ctx: Ctx<'js>, args: Rest<Value<'js>>| {
            check_argument_count(&args, 1, &ctx)?;
            let path = ensure_exists(args[0].as_string(), "argument 0 must be a string", &ctx)?
                .to_string()?;
            Ok::<_, rquickjs::Error>(storage.exists(&path))
        })?,
    )?;
    ctx.globals()
        .prop("storage", Property::from(object).enumerable())?;
    Ok(())
}
