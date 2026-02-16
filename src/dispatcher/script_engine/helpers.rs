use boa_engine::{Context, JsError, JsResult, JsValue, js_string};
use std::cell::RefMut;

use super::response::Response;

/// 确保 Option 值存在，否则返回错误
pub(crate) fn ensure_exists<T>(option: Option<T>, msg: &str) -> JsResult<T> {
    Ok(option.ok_or_else(|| JsError::from_opaque(js_string!(msg).into()))?)
}

/// 从 Uint8Array 读取数据
pub(crate) fn read_data_from_uint8_array(value: &JsValue, ctx: &mut Context) -> JsResult<Vec<u8>> {
    use boa_engine::object::builtins::{JsArrayBuffer, JsUint8Array};
    use std::ops::Deref;

    const ERR_MSG: &'static str = "not a valid Uint8Array";

    let buffer = JsUint8Array::from_object(ensure_exists(value.as_object(), ERR_MSG)?.to_owned())?
        .buffer(ctx)?;
    let buffer =
        JsArrayBuffer::from_object(ensure_exists(buffer.as_object(), ERR_MSG)?.to_owned())?;

    Ok(if let Some(data) = buffer.data() {
        Vec::from(data.deref())
    } else {
        Vec::new()
    })
}

/// 从 JsValue 读取 u8 数组（支持字符串和 Uint8Array）
pub(crate) fn read_u8_array_from_js_value(value: &JsValue, ctx: &mut Context) -> JsResult<Vec<u8>> {
    if let Some(string) = value.as_string() {
        Ok(string.to_std_string_lossy().as_bytes().to_vec())
    } else {
        read_data_from_uint8_array(value, ctx)
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
pub(crate) fn get_response_from_context(ctx: &mut Context) -> JsResult<RefMut<'_, Response>> {
    use super::response::ResponseCell;
    use boa_gc::Gc;

    Ok(ensure_exists(
        ctx.get_data::<Gc<ResponseCell>>(),
        "failed get response from context",
    )?
    .cell
    .borrow_mut())
}
