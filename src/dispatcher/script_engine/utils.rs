use rquickjs::{Ctx, Exception, Function, Result, TypedArray, Value, function::Rest};

use super::helpers::{check_argument_count, ensure_exists, read_u8_array_from_js_value};

pub fn register_utils_to_context<'js>(ctx: &Ctx<'js>) -> Result<()> {
    let globals = ctx.globals();
    globals.set(
        "base64Encode",
        Function::new(ctx.clone(), |ctx: Ctx<'js>, args: Rest<Value<'js>>| {
            check_argument_count(&args, 1, &ctx)?;
            let data = read_u8_array_from_js_value(&args[0], &ctx)?;
            Ok::<_, rquickjs::Error>(base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                &data,
            ))
        })?,
    )?;
    globals.set(
        "base64Decode",
        Function::new(ctx.clone(), |ctx: Ctx<'js>, args: Rest<Value<'js>>| {
            check_argument_count(&args, 1, &ctx)?;
            let string = ensure_exists(args[0].as_string(), "argument must be a string", &ctx)?
                .to_string()?;
            let decoded =
                base64::Engine::decode(&base64::engine::general_purpose::STANDARD, string)
                    .map_err(|error| {
                        Exception::throw_type(&ctx, &format!("invalid base64 string: {error}"))
                    })?;
            TypedArray::new(ctx, decoded)
        })?,
    )?;
    globals.set(
        "urlEncode",
        Function::new(ctx.clone(), |ctx: Ctx<'js>, args: Rest<Value<'js>>| {
            check_argument_count(&args, 1, &ctx)?;
            let string = ensure_exists(args[0].as_string(), "argument must be a string", &ctx)?
                .to_string()?;
            Ok::<_, rquickjs::Error>(urlencoding::encode(&string).into_owned())
        })?,
    )?;
    globals.set(
        "urlDecode",
        Function::new(ctx.clone(), |ctx: Ctx<'js>, args: Rest<Value<'js>>| {
            check_argument_count(&args, 1, &ctx)?;
            let string = ensure_exists(args[0].as_string(), "argument must be a string", &ctx)?
                .to_string()?;
            urlencoding::decode(&string)
                .map(|decoded| decoded.into_owned())
                .map_err(|error| {
                    Exception::throw_type(&ctx, &format!("invalid url encoded string: {error}"))
                })
        })?,
    )?;
    Ok(())
}
