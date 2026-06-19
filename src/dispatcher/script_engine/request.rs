use boa_engine::object::builtins::{AlignedVec, JsArray, JsArrayBuffer, JsUint8Array};
use boa_engine::{
    Context, JsObject, NativeFunction, js_string, object::ObjectInitializer, property::Attribute,
};
use boa_engine::{JsResult, JsValue};
use serde_json::Value;
use std::collections::BTreeSet;

use crate::utils::multimap::MultiMap;
use crate::utils::parsed_request::{ParsedRequest, ParsedRequestBody};

use super::helpers::{check_argument_count, ensure_exists};

/// `get(key)` method for multimap-style JS objects.
/// Reads `this[key]` and returns the first element of the array, or undefined.
fn multimap_get_fn(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    check_argument_count(args, 1)?;
    let key = ensure_exists(args[0].as_string(), "key must be string")?;
    let this_obj = ensure_exists(this.as_object(), "this must be an object")?;
    let prop = this_obj.get(key.clone(), ctx)?;

    if let Some(arr_obj) = prop.as_object() {
        arr_obj.get(0u32, ctx)
    } else {
        Ok(JsValue::undefined())
    }
}

/// 创建 MultiMap 的 JS 对象 + 数组结构（用于 headers、params、form）
///
/// 结果结构：
/// ```js
/// {
///     "key1": ["val1", "val2"],
///     "key2": ["val3"],
///     get: function(key) { return this[key]?.[0] }
/// }
/// ```
fn create_multimap_object(
    context: &mut Context,
    multimap: &MultiMap<String, String>,
) -> JsResult<JsObject> {
    // 创建带有 get 方法的对象
    let obj = ObjectInitializer::new(context)
        .function(
            NativeFunction::from_fn_ptr(multimap_get_fn),
            js_string!("get"),
            1,
        )
        .build();

    // 遍历所有唯一 key，为每个 key 创建值数组
    let keys: BTreeSet<&String> = multimap.iter().map(|(k, _)| k).collect();
    for key in keys {
        if let Some(values) = multimap.get_all(key) {
            let js_array = JsArray::new(context);
            for (i, value) in values.iter().enumerate() {
                js_array.set(i, JsValue::from(js_string!(value.as_str())), false, context)?;
            }
            obj.set(js_string!(key.as_str()), js_array, false, context)?;
        }
    }

    Ok(obj)
}

/// 将 serde_json::Value 递归转换为 JsValue
fn json_value_to_js_value(value: &Value, ctx: &mut Context) -> JsResult<JsValue> {
    match value {
        Value::Null => Ok(JsValue::null()),
        Value::Bool(b) => Ok(JsValue::from(*b)),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(JsValue::from(i))
            } else if let Some(f) = n.as_f64() {
                Ok(JsValue::from(f))
            } else {
                Ok(JsValue::undefined())
            }
        }
        Value::String(s) => Ok(JsValue::from(js_string!(s.as_str()))),
        Value::Array(arr) => {
            let js_arr = JsArray::new(ctx);
            for (i, item) in arr.iter().enumerate() {
                js_arr.set(i, json_value_to_js_value(item, ctx)?, false, ctx)?;
            }
            Ok(js_arr.into())
        }
        Value::Object(obj) => {
            let js_obj = JsObject::with_null_proto();
            for (key, val) in obj {
                js_obj.set(
                    js_string!(key.as_str()),
                    json_value_to_js_value(val, ctx)?,
                    false,
                    ctx,
                )?;
            }
            Ok(js_obj.into())
        }
    }
}

/// 创建文件对象 { filename: String, content: Uint8Array }
fn create_file_object(filename: &str, content: &[u8], ctx: &mut Context) -> JsResult<JsObject> {
    // 创建 Uint8Array（使用安全 API）
    let mut aligned_vec: AlignedVec<u8> = AlignedVec::new(64);
    aligned_vec.extend_from_slice(content);
    let array_buffer = JsArrayBuffer::from_byte_block(aligned_vec, ctx)?;
    let uint8_array = JsUint8Array::from_array_buffer(array_buffer, ctx)?;

    // 创建文件对象
    let file_obj = JsObject::with_null_proto();
    file_obj.set(
        js_string!("filename"),
        JsValue::from(js_string!(filename)),
        false,
        ctx,
    )?;
    file_obj.set(js_string!("content"), uint8_array, false, ctx)?;

    Ok(file_obj)
}

/// 创建上传文件的 JS 对象 + 数组结构（用于 files）
///
/// 结果结构：
/// ```js
/// {
///     "avatar": [{ filename: "photo.jpg", content: Uint8Array }],
///     get: function(name) { return this[name]?.[0] }
/// }
/// ```
fn create_upload_files_object(
    context: &mut Context,
    files: &MultiMap<String, (String, Vec<u8>)>,
) -> JsResult<JsObject> {
    // 创建带有 get 方法的对象
    let obj = ObjectInitializer::new(context)
        .function(
            NativeFunction::from_fn_ptr(multimap_get_fn),
            js_string!("get"),
            1,
        )
        .build();

    // 遍历所有唯一 key，为每个 key 创建文件对象数组
    let keys: BTreeSet<&String> = files.iter().map(|(k, _)| k).collect();
    for key in keys {
        if let Some(file_list) = files.get_all(key) {
            let js_array = JsArray::new(context);
            for (i, (filename, content)) in file_list.iter().enumerate() {
                let file_obj = create_file_object(filename, content, context)?;
                js_array.set(i, file_obj, false, context)?;
            }
            obj.set(js_string!(key.as_str()), js_array, false, context)?;
        }
    }

    Ok(obj)
}

/// 注册 Request 对象到 JS 上下文
pub fn register_request_to_context(context: &mut Context, request: &ParsedRequest) {
    // 创建 body 的 Uint8Array
    let mut aligned_vec: AlignedVec<u8> = AlignedVec::new(64);
    aligned_vec.extend_from_slice(&request.body);
    let array_buffer = JsArrayBuffer::from_byte_block(aligned_vec, context)
        .expect("failed to create array buffer");
    let uint8_array = JsUint8Array::from_array_buffer(array_buffer, context)
        .expect("failed to create uint8 array");

    let headers_obj =
        create_multimap_object(context, &request.headers).expect("failed to create headers object");
    let params_obj =
        create_multimap_object(context, &request.params).expect("failed to create params object");

    // 处理 parsed_body — 始终创建 json, form, files 属性
    let empty_form = MultiMap::new();
    let empty_files = MultiMap::new();

    let (json_value, form_data, files_data) = match &request.parsed_body {
        ParsedRequestBody::Json(json_value) => (
            json_value_to_js_value(json_value, context).expect("failed to create json value"),
            &empty_form,
            &empty_files,
        ),
        ParsedRequestBody::Form(form, files) => {
            (JsValue::from(JsObject::with_null_proto()), form, files)
        }
        ParsedRequestBody::None | ParsedRequestBody::Failed => (
            JsValue::from(JsObject::with_null_proto()),
            &empty_form,
            &empty_files,
        ),
    };

    let forms_obj =
        create_multimap_object(context, form_data).expect("failed to create form object");
    let files_obj =
        create_upload_files_object(context, files_data).expect("failed to create files object");

    // 创建 request 对象
    let mut object_builder = ObjectInitializer::new(context);

    // 基础属性（只读）
    object_builder
        .property(
            js_string!("method"),
            JsValue::from(js_string!(request.method.as_str())),
            Attribute::READONLY | Attribute::ENUMERABLE,
        )
        .property(
            js_string!("path"),
            JsValue::from(js_string!(request.path.as_str())),
            Attribute::READONLY | Attribute::ENUMERABLE,
        )
        .property(
            js_string!("clientAddr"),
            JsValue::from(js_string!(request.client_addr.to_string().as_str())),
            Attribute::READONLY | Attribute::ENUMERABLE,
        )
        .property(
            js_string!("body"),
            uint8_array,
            Attribute::READONLY | Attribute::ENUMERABLE,
        )
        .property(
            js_string!("headers"),
            headers_obj,
            Attribute::READONLY | Attribute::ENUMERABLE,
        )
        .property(
            js_string!("params"),
            params_obj,
            Attribute::READONLY | Attribute::ENUMERABLE,
        )
        .property(
            js_string!("json"),
            json_value,
            Attribute::READONLY | Attribute::ENUMERABLE,
        )
        .property(
            js_string!("forms"),
            forms_obj,
            Attribute::READONLY | Attribute::ENUMERABLE,
        )
        .property(
            js_string!("files"),
            files_obj,
            Attribute::READONLY | Attribute::ENUMERABLE,
        );

    let object = object_builder.build();
    context
        .register_global_property(
            js_string!("request"),
            object,
            Attribute::READONLY | Attribute::ENUMERABLE,
        )
        .expect("property shouldn't exist");
}
