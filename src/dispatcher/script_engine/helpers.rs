use rquickjs::{Array, Ctx, Exception, IntoJs, Object, Result, TypedArray, Value};

pub(crate) fn ensure_exists<T>(option: Option<T>, message: &str, ctx: &Ctx<'_>) -> Result<T> {
    option.ok_or_else(|| Exception::throw_type(ctx, message))
}

pub(crate) fn read_data_from_uint8_array(value: &Value<'_>, ctx: &Ctx<'_>) -> Result<Vec<u8>> {
    let array = TypedArray::<u8>::from_value(value.clone())
        .map_err(|_| Exception::throw_type(ctx, "not a valid Uint8Array"))?;
    // SAFETY: Copy the view, including its offset and length, without executing JavaScript.
    unsafe { array.as_bytes() }
        .map(<[u8]>::to_vec)
        .ok_or_else(|| Exception::throw_type(ctx, "Uint8Array buffer is detached"))
}

pub(crate) fn read_u8_array_from_js_value(value: &Value<'_>, ctx: &Ctx<'_>) -> Result<Vec<u8>> {
    if let Some(string) = value.as_string() {
        Ok(string.to_string()?.into_bytes())
    } else {
        read_data_from_uint8_array(value, ctx)
    }
}

pub(crate) fn json_value_to_js_value<'js>(
    value: &serde_json::Value,
    ctx: &Ctx<'js>,
) -> Result<Value<'js>> {
    match value {
        serde_json::Value::Null => Ok(Value::new_null(ctx.clone())),
        serde_json::Value::Bool(value) => value.into_js(ctx),
        serde_json::Value::Number(value) => value.as_f64().into_js(ctx),
        serde_json::Value::String(value) => value.as_str().into_js(ctx),
        serde_json::Value::Array(values) => {
            let array = Array::new(ctx.clone())?;
            for (index, value) in values.iter().enumerate() {
                array.set(index, json_value_to_js_value(value, ctx)?)?;
            }
            Ok(array.into_value())
        }
        serde_json::Value::Object(values) => {
            let object = Object::new_proto(ctx.clone(), None)?;
            for (key, value) in values {
                object.set(key.as_str(), json_value_to_js_value(value, ctx)?)?;
            }
            Ok(object.into_value())
        }
    }
}

pub(crate) fn check_argument_count(args: &[Value<'_>], count: usize, ctx: &Ctx<'_>) -> Result<()> {
    if args.len() < count {
        return Err(Exception::throw_type(
            ctx,
            &format!("argument {} not found", args.len()),
        ));
    }
    Ok(())
}
