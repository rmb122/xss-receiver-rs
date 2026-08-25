use boa_engine::object::builtins::JsArray;
use boa_engine::{Context, JsError, JsNativeError, JsResult, JsValue, js_string};
use serde_json::Value;
use std::cell::RefMut;

use super::http_response::HttpResponse;

/// 确保 Option 值存在，否则返回错误
pub(crate) fn ensure_exists<T>(option: Option<T>, msg: &str) -> JsResult<T> {
    Ok(option.ok_or_else(|| JsError::from_opaque(js_string!(msg).into()))?)
}

/// 从 Uint8Array 读取数据
pub(crate) fn read_data_from_uint8_array(value: &JsValue, ctx: &mut Context) -> JsResult<Vec<u8>> {
    use boa_engine::object::builtins::{JsArrayBuffer, JsUint8Array};
    const ERR_MSG: &'static str = "not a valid Uint8Array";

    let array = JsUint8Array::from_object(ensure_exists(value.as_object(), ERR_MSG)?.to_owned())?;
    let byte_offset = array.byte_offset(ctx)?;
    let byte_length = array.byte_length(ctx)?;
    let buffer = array.buffer(ctx)?;
    let buffer =
        JsArrayBuffer::from_object(ensure_exists(buffer.as_object(), ERR_MSG)?.to_owned())?;

    let Some(data) = buffer.data() else {
        return Ok(Vec::new());
    };
    let end = byte_offset
        .checked_add(byte_length)
        .ok_or_else(|| JsNativeError::range().with_message("Uint8Array range overflow"))?;
    let data = data
        .get(byte_offset..end)
        .ok_or_else(|| JsNativeError::range().with_message("Uint8Array range is out of bounds"))?;
    Ok(data.to_vec())
}

/// 从 JsValue 读取 u8 数组（支持字符串和 Uint8Array）
pub(crate) fn read_u8_array_from_js_value(value: &JsValue, ctx: &mut Context) -> JsResult<Vec<u8>> {
    if let Some(string) = value.as_string() {
        Ok(string.to_std_string_lossy().as_bytes().to_vec())
    } else {
        read_data_from_uint8_array(value, ctx)
    }
}

/// Convert a serde JSON value into a JavaScript value owned by `ctx`.
pub(crate) fn json_value_to_js_value(value: &Value, ctx: &mut Context) -> JsResult<JsValue> {
    match value {
        Value::Null => Ok(JsValue::null()),
        Value::Bool(value) => Ok(JsValue::from(*value)),
        Value::Number(value) => {
            if let Some(value) = value.as_i64() {
                Ok(JsValue::from(value))
            } else if let Some(value) = value.as_f64() {
                Ok(JsValue::from(value))
            } else {
                Ok(JsValue::undefined())
            }
        }
        Value::String(value) => Ok(JsValue::from(js_string!(value.as_str()))),
        Value::Array(values) => {
            let array = JsArray::new(ctx);
            for (index, value) in values.iter().enumerate() {
                array.set(index, json_value_to_js_value(value, ctx)?, false, ctx)?;
            }
            Ok(array.into())
        }
        Value::Object(values) => {
            let object = boa_engine::JsObject::with_null_proto();
            for (key, value) in values {
                object.set(
                    js_string!(key.as_str()),
                    json_value_to_js_value(value, ctx)?,
                    false,
                    ctx,
                )?;
            }
            Ok(object.into())
        }
    }
}

/// 检查参数数量是否符合预期
pub(crate) fn check_argument_count(args: &[JsValue], count: usize) -> JsResult<()> {
    for i in 0..count {
        ensure_exists(args.get(i), &format!("argument {} not found", i))?;
    }
    Ok(())
}

/// 从 Context 获取 Response 对象
pub(crate) fn get_response_from_context(ctx: &mut Context) -> JsResult<RefMut<'_, HttpResponse>> {
    use super::http_response::HttpResponseCell;
    use boa_gc::Gc;

    Ok(ensure_exists(
        ctx.get_data::<Gc<HttpResponseCell>>(),
        "failed get response from context",
    )?
    .cell
    .borrow_mut())
}
