use boa_engine::object::builtins::{AlignedVec, JsArrayBuffer, JsUint8Array};
use boa_engine::{Context, JsString, NativeFunction, js_string};
use boa_engine::{JsNativeError, JsResult, JsValue};

use super::helpers::{check_argument_count, ensure_exists, read_data_from_uint8_array};

/// base64 编码函数
fn base64_encode_fn(_this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    check_argument_count(args, 1)?;

    let data = if let Some(string) = args[0].as_string() {
        string.to_std_string_lossy().as_bytes().to_vec()
    } else {
        read_data_from_uint8_array(&args[0], ctx)?
    };

    let encoded = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &data);
    Ok(JsValue::from(js_string!(encoded)))
}

/// base64 解码函数
fn base64_decode_fn(_this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    check_argument_count(args, 1)?;

    let string =
        ensure_exists(args[0].as_string(), "argument must be a string")?.to_std_string_lossy();

    let decoded = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, string)
        .map_err(|e| JsNativeError::typ().with_message(format!("invalid base64 string: {}", e)))?;

    let mut aligned_vec: AlignedVec<u8> = AlignedVec::new(64);
    aligned_vec.extend_from_slice(&decoded);
    let array_buffer = JsArrayBuffer::from_byte_block(aligned_vec, ctx)?;
    let uint8_array = JsUint8Array::from_array_buffer(array_buffer, ctx)?;

    Ok(uint8_array.into())
}

/// URL 编码函数
fn url_encode_fn(_this: &JsValue, args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    check_argument_count(args, 1)?;

    let string =
        ensure_exists(args[0].as_string(), "argument must be a string")?.to_std_string_lossy();

    let encoded = urlencoding::encode(&string);
    Ok(JsValue::from(js_string!(encoded)))
}

/// URL 解码函数
fn url_decode_fn(_this: &JsValue, args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    check_argument_count(args, 1)?;

    let string =
        ensure_exists(args[0].as_string(), "argument must be a string")?.to_std_string_lossy();

    let decoded = urlencoding::decode(&string).map_err(|e| {
        JsNativeError::typ().with_message(format!("invalid url encoded string: {}", e))
    })?;

    Ok(JsValue::from(js_string!(decoded)))
}

/// 注册 utils 工具函数到 JS 上下文（作为全局函数）
pub fn register_utils_to_context(context: &mut Context) {
    context
        .register_global_builtin_callable(
            JsString::from("base64Encode"),
            1,
            NativeFunction::from_fn_ptr(base64_encode_fn),
        )
        .expect("base64Encode shouldn't exist");

    context
        .register_global_builtin_callable(
            JsString::from("base64Decode"),
            1,
            NativeFunction::from_fn_ptr(base64_decode_fn),
        )
        .expect("base64Decode shouldn't exist");

    context
        .register_global_builtin_callable(
            JsString::from("urlEncode"),
            1,
            NativeFunction::from_fn_ptr(url_encode_fn),
        )
        .expect("urlEncode shouldn't exist");

    context
        .register_global_builtin_callable(
            JsString::from("urlDecode"),
            1,
            NativeFunction::from_fn_ptr(url_decode_fn),
        )
        .expect("urlDecode shouldn't exist");
}
