use boa_engine::object::builtins::{AlignedVec, JsArray, JsArrayBuffer, JsProxy, JsUint8Array};
use boa_engine::{
    Context, JsNativeError, JsObject, NativeFunction, js_string,
    object::{IntegrityLevel, ObjectInitializer},
    property::Attribute,
};
use boa_engine::{JsResult, JsValue};
use std::collections::BTreeSet;

use crate::utils::multimap::MultiMap;
use crate::utils::parsed_request::{ParsedRequest, ParsedRequestBody, normalize_header_name};

use super::helpers::{check_argument_count, ensure_exists, json_value_to_js_value};

/// Freeze the request data before exposing it to JavaScript. All input objects are
/// acyclic; binary buffers intentionally retain the normal writable Uint8Array API.
fn freeze_request_value(value: &JsValue, ctx: &mut Context) -> JsResult<()> {
    let Some(object) = value.as_object() else {
        return Ok(());
    };
    if object.is_callable() || JsUint8Array::from_object(object.clone()).is_ok() {
        return Ok(());
    }

    for key in object.own_property_keys(ctx)? {
        freeze_request_value(&object.get(key, ctx)?, ctx)?;
    }
    if !object.set_integrity_level(IntegrityLevel::Frozen, ctx)? {
        return Err(JsNativeError::typ()
            .with_message("could not freeze request data")
            .into());
    }
    Ok(())
}

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

/// 创建 MultiMap 的 JS 对象 + 数组结构（用于 headers、query、form）
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
    get_fn: NativeFunction,
) -> JsResult<JsObject> {
    // 创建带有 get 方法的对象
    let obj = ObjectInitializer::new(context)
        .function(get_fn, js_string!("get"), 1)
        .build();

    // 遍历所有唯一 key，为每个 key 创建值数组
    let keys: BTreeSet<&String> = multimap.iter().map(|(k, _)| k).collect();
    for key in keys {
        if let Some(values) = multimap.get_all(key) {
            let js_array = JsArray::new(context);
            for (i, value) in values.iter().enumerate() {
                js_array.set(i, JsValue::from(js_string!(value.as_str())), false, context)?;
            }
            obj.create_data_property_or_throw(js_string!(key.as_str()), js_array, context)?;
        }
    }

    Ok(obj)
}

fn headers_get_fn(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    // Let the proxy normalize the lookup once. Only disambiguate the exact name
    // "get", which is reserved for the method rather than the "Get" header.
    if args.first().and_then(JsValue::as_string) == Some(js_string!("get")) {
        return multimap_get_fn(this, &[js_string!("Get").into()], ctx);
    }
    multimap_get_fn(this, args, ctx)
}

fn headers_proxy_get(_this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let target = ensure_exists(args[0].as_object(), "headers target must be an object")?;
    let Some(name) = args[1].as_string() else {
        return target.get(args[1].to_property_key(ctx)?, ctx);
    };
    let key = if name == js_string!("get") {
        name.clone()
    } else {
        js_string!(normalize_header_name(&name.to_std_string_lossy()))
    };

    // Inherited properties such as __proto__ are not request headers.
    if target.has_own_property(key.clone(), ctx)? {
        target.get(key, ctx)
    } else {
        Ok(JsValue::undefined())
    }
}

fn create_headers_object(
    context: &mut Context,
    headers: &MultiMap<String, String>,
) -> JsResult<JsObject> {
    // ParsedRequest already normalized and grouped the incoming header names.
    let target = create_multimap_object(
        context,
        headers,
        NativeFunction::from_fn_ptr(headers_get_fn),
    )?;

    Ok(JsProxy::builder(target)
        .get(headers_proxy_get)
        .build(context)
        .into())
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
            obj.create_data_property_or_throw(js_string!(key.as_str()), js_array, context)?;
        }
    }

    Ok(obj)
}

/// 注册 Request 对象到 JS 上下文
pub fn register_http_request_to_context(context: &mut Context, request: &ParsedRequest) {
    // 创建 body 的 Uint8Array
    let mut aligned_vec: AlignedVec<u8> = AlignedVec::new(64);
    aligned_vec.extend_from_slice(&request.raw_body);
    let array_buffer = JsArrayBuffer::from_byte_block(aligned_vec, context)
        .expect("failed to create array buffer");
    let uint8_array = JsUint8Array::from_array_buffer(array_buffer, context)
        .expect("failed to create uint8 array");

    let headers_obj =
        create_headers_object(context, &request.headers).expect("failed to create headers object");
    let query_obj = create_multimap_object(
        context,
        &request.parsed_query,
        NativeFunction::from_fn_ptr(multimap_get_fn),
    )
    .expect("failed to create query object");

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

    let forms_obj = create_multimap_object(
        context,
        form_data,
        NativeFunction::from_fn_ptr(multimap_get_fn),
    )
    .expect("failed to create form object");
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
            js_string!("query"),
            query_obj,
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
    freeze_request_value(&object.clone().into(), context).expect("failed to freeze request data");
    context
        .register_global_property(
            js_string!("request"),
            object,
            Attribute::READONLY | Attribute::ENUMERABLE,
        )
        .expect("property shouldn't exist");
}

#[cfg(test)]
mod tests {
    use axum::{body::Body, http::Request};
    use boa_engine::Source;
    use serde_json::json;

    use super::*;
    use crate::dispatcher::script_engine::{create_context, evaluate_module};

    async fn evaluate_request_module(request: &ParsedRequest, source: &str) -> serde_json::Value {
        let (mut context, executor) = create_context();
        register_http_request_to_context(&mut context, request);
        evaluate_module(source, &mut context, executor)
            .await
            .unwrap()
    }

    fn evaluate_request(request: &ParsedRequest, source: &str) -> serde_json::Value {
        let (mut context, _) = create_context();
        register_http_request_to_context(&mut context, request);
        context
            .eval(Source::from_bytes(source))
            .unwrap()
            .to_json(&mut context)
            .unwrap()
            .unwrap()
    }

    #[tokio::test(flavor = "current_thread")]
    async fn header_lookups_ignore_case_and_preserve_values_and_enumeration() {
        let request = ParsedRequest::new(
            "127.0.0.1:1234".parse().unwrap(),
            Request::builder()
                .method("POST")
                .uri("/?Token=query-upper&token=query-lower")
                .header("HOST", "example.test")
                .header("x-multi", "first")
                .header("X-MULTI", "second")
                .header("Content-Type", "application/x-www-form-urlencoded")
                .body(Body::from("Token=form-upper&token=form-lower"))
                .unwrap(),
            1024,
        )
        .await
        .unwrap();

        let result = evaluate_request(
            &request,
            r#"
                const h = request.headers;
                ({
                    first: ['Host', 'host', 'HOST', 'hOsT'].map(key => h.get(key)),
                    host: h.host,
                    sameArray: h.Host === h['HOST'] && h['HOST'] === h.host,
                    multi: h['x-MULTI'],
                    firstMulti: h.get('X-multi'),
                    missing: h.get('missing') === undefined && h.MISSING === undefined,
                    keys: Object.keys(h),
                    serialized: JSON.parse(JSON.stringify(h)),
                    query: [request.query.get('Token'), request.query.get('token')],
                    form: [request.forms.get('Token'), request.forms.get('token')],
                    otherMapsKeepCase: request.query.TOKEN === undefined
                        && request.query.get('TOKEN') === undefined
                        && request.forms.TOKEN === undefined
                        && request.forms.get('TOKEN') === undefined,
                });
            "#,
        );

        assert_eq!(
            result,
            json!({
                "first": ["example.test", "example.test", "example.test", "example.test"],
                "host": ["example.test"],
                "sameArray": true,
                "multi": ["first", "second"],
                "firstMulti": "first",
                "missing": true,
                "keys": ["Content-Type", "Host", "X-Multi"],
                "serialized": serde_json::to_value(&request.headers).unwrap(),
                "query": ["query-upper", "query-lower"],
                "form": ["form-upper", "form-lower"],
                "otherMapsKeepCase": true,
            })
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn header_names_do_not_replace_get_or_change_the_object_prototype() {
        let request = ParsedRequest::new(
            "127.0.0.1:1234".parse().unwrap(),
            Request::builder()
                .header("get", "header-get")
                .header("__proto__", "header-proto")
                .header("constructor", "header-constructor")
                .header("toString", "header-to-string")
                .body(Body::empty())
                .unwrap(),
            1024,
        )
        .await
        .unwrap();

        let result = evaluate_request(
            &request,
            r#"
                const h = request.headers;
                const symbol = Symbol('marker');
                ({
                    getIsMethod: typeof h.get === 'function',
                    getHeader: h.get('get'),
                    mixedCaseGetHeader: h.get('gEt'),
                    getArray: h.GET,
                    sameArray: h.Get === h.GET,
                    protoHeader: h.get('__PROTO__'),
                    constructorHeader: h.constructor,
                    toStringHeader: h.toString,
                    prototypeIntact: Object.getPrototypeOf(h) === Object.prototype,
                    symbolWriteRejected: Reflect.set(h, symbol, 'symbol-value') === false
                        && h[symbol] === undefined,
                    serialized: JSON.parse(JSON.stringify(h)),
                });
            "#,
        );

        assert_eq!(
            result,
            json!({
                "getIsMethod": true,
                "getHeader": "header-get",
                "mixedCaseGetHeader": "header-get",
                "getArray": ["header-get"],
                "sameArray": true,
                "protoHeader": "header-proto",
                "constructorHeader": ["header-constructor"],
                "toStringHeader": ["header-to-string"],
                "prototypeIntact": true,
                "symbolWriteRejected": true,
                "serialized": serde_json::to_value(&request.headers).unwrap(),
            })
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn empty_headers_return_undefined_and_get_still_validates_arguments() {
        let request = ParsedRequest::new(
            "127.0.0.1:1234".parse().unwrap(),
            Request::new(Body::empty()),
            1024,
        )
        .await
        .unwrap();

        assert_eq!(
            evaluate_request(
                &request,
                r#"
                    const h = request.headers;
                    ({
                        empty: Object.keys(h).length === 0,
                        missing: ['Host', 'get', '__proto__', 'constructor', 'toString']
                            .every(key => h.get(key) === undefined),
                        missingProperties: ['Host', 'GET', '__proto__', 'constructor', 'toString']
                            .every(key => h[key] === undefined),
                        invalidArguments: [() => h.get(), () => h.get(123)].every(call => {
                            try { call(); return false; } catch { return true; }
                        }),
                    });
                "#,
            ),
            json!({
                "empty": true,
                "missing": true,
                "missingProperties": true,
                "invalidArguments": true,
            })
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn default_exports_preserve_headers_directly_and_in_nested_objects() {
        let request = ParsedRequest::new(
            "127.0.0.1:1234".parse().unwrap(),
            Request::builder()
                .header("host", "example.test")
                .header("x-multi", "first")
                .header("x-multi", "second")
                .body(Body::empty())
                .unwrap(),
            1024,
        )
        .await
        .unwrap();
        let headers = serde_json::to_value(&request.headers).unwrap();

        assert_eq!(
            evaluate_request_module(&request, "export default request.headers;").await,
            headers
        );
        assert_eq!(
            evaluate_request_module(
                &request,
                "export default { headers: request.headers, nested: [request.headers, { headers: request.headers }] };",
            )
            .await,
            json!({ "headers": headers, "nested": [headers, { "headers": headers }] })
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn request_maps_and_their_value_arrays_are_readonly() {
        let request = ParsedRequest::new(
            "127.0.0.1:1234".parse().unwrap(),
            Request::builder()
                .method("POST")
                .uri("/?Token=query&__proto__=query-proto")
                .header("host", "example.test")
                .header("Content-Type", "application/x-www-form-urlencoded")
                .body(Body::from("Token=form&__proto__=form-proto"))
                .unwrap(),
            1024,
        )
        .await
        .unwrap();

        let result = evaluate_request_module(
            &request,
            r#"
                const maps = [
                    [request.headers, 'Host'],
                    [request.query, 'Token'],
                    [request.forms, 'Token'],
                    [request.query, '__proto__'],
                    [request.forms, '__proto__'],
                ];
                const attempts = maps.flatMap(([map, key]) => [
                    () => { map[key] = ['changed']; },
                    () => { map.added = ['changed']; },
                    () => { delete map[key]; },
                    () => { map.get = () => 'changed'; },
                    () => { Object.defineProperty(map, key, { value: ['changed'] }); },
                    () => { Object.setPrototypeOf(map, null); },
                    () => { map[key][0] = 'changed'; },
                    () => { map[key].push('changed'); },
                    () => { map[key].splice(0, 1); },
                    () => { map[key].length = 0; },
                    () => { delete map[key][0]; },
                ]);
                attempts.push(
                    () => { request.headers['x-added'] = ['changed']; },
                    () => { request.headers.host = ['changed']; },
                    () => { request.headers = {}; },
                    () => { request.body = new Uint8Array(); },
                    () => { request.method = 'DELETE'; },
                    () => { request.added = true; },
                    () => { delete request.query; },
                    () => { request.files.added = []; },
                );
                const failures = [];
                attempts.forEach((attempt, index) => {
                    try { attempt(); failures.push(index); }
                    catch (error) { if (!(error instanceof TypeError)) failures.push(index); }
                });
                const copy = [...request.headers.host];
                copy[0] = 'local-copy';
                export default {
                    failures,
                    frozen: Object.isFrozen(request) && Object.isFrozen(request.files)
                        && Object.isFrozen(request.json)
                        && maps.every(([map, key]) => Object.isFrozen(map) && Object.isFrozen(map[key])),
                    reflectWriteRejected: !Reflect.set(request.headers, 'x-added', ['changed']),
                    headers: request.headers,
                    query: request.query,
                    forms: request.forms,
                    copy,
                };
            "#,
        )
        .await;

        assert_eq!(
            result,
            json!({
                "failures": [],
                "frozen": true,
                "reflectWriteRejected": true,
                "headers": serde_json::to_value(&request.headers).unwrap(),
                "query": { "Token": ["query"], "__proto__": ["query-proto"] },
                "forms": { "Token": ["form"], "__proto__": ["form-proto"] },
                "copy": ["local-copy"],
            })
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn parsed_json_is_deeply_readonly_but_body_bytes_remain_writable() {
        let request = ParsedRequest::new(
            "127.0.0.1:1234".parse().unwrap(),
            Request::builder()
                .header("content-type", "application/json")
                .body(Body::from(r#"{"items":[{"name":"original"}]}"#))
                .unwrap(),
            1024,
        )
        .await
        .unwrap();

        assert_eq!(
            evaluate_request_module(
                &request,
                r#"
                    const attempts = [
                        () => { request.json.items = []; },
                        () => { request.json.items.push({}); },
                        () => { request.json.items[0].name = 'changed'; },
                        () => { delete request.json.items[0].name; },
                        () => { Object.defineProperty(request.json.items[0], 'name', { value: 'changed' }); },
                    ];
                    const blocked = attempts.every(attempt => {
                        try { attempt(); return false; }
                        catch (error) { return error instanceof TypeError; }
                    });
                    request.body[0] = 65;
                    request.body.set([66], 1);
                    export default {
                        blocked,
                        frozen: Object.isFrozen(request.json) && Object.isFrozen(request.json.items)
                            && Object.isFrozen(request.json.items[0]),
                        json: request.json,
                        bytes: [request.body[0], request.body[1]],
                    };
                "#,
            )
            .await,
            json!({
                "blocked": true,
                "frozen": true,
                "json": { "items": [{ "name": "original" }] },
                "bytes": [65, 66],
            })
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn uploaded_file_descriptors_are_readonly_but_content_bytes_remain_writable() {
        let request = ParsedRequest::new(
            "127.0.0.1:1234".parse().unwrap(),
            Request::builder()
                .header(
                    "content-type",
                    "multipart/form-data; boundary=readonly-test",
                )
                .body(Body::from(concat!(
                    "--readonly-test\r\n",
                    "Content-Disposition: form-data; name=upload; filename=test.txt\r\n",
                    "Content-Type: application/octet-stream\r\n\r\n",
                    "abc\r\n--readonly-test--\r\n",
                )))
                .unwrap(),
            1024,
        )
        .await
        .unwrap();

        assert_eq!(
            evaluate_request_module(
                &request,
                r#"
                    const files = request.files.upload;
                    const file = request.files.get('upload');
                    const attempts = [
                        () => { request.files.upload = []; },
                        () => { request.files.get = () => null; },
                        () => { files[0] = {}; },
                        () => { files.push({}); },
                        () => { file.filename = 'changed'; },
                        () => { file.content = new Uint8Array(); },
                        () => { delete file.content; },
                        () => { Object.defineProperty(file, 'filename', { value: 'changed' }); },
                    ];
                    const blocked = attempts.every(attempt => {
                        try { attempt(); return false; }
                        catch (error) { return error instanceof TypeError; }
                    });
                    file.content[0] = 65;
                    file.content.set([66], 1);
                    export default {
                        blocked,
                        frozen: Object.isFrozen(request.files) && Object.isFrozen(files) && Object.isFrozen(file),
                        filename: file.filename,
                        bytes: Array.from(file.content),
                        sameFile: file === files[0],
                    };
                "#,
            )
            .await,
            json!({
                "blocked": true,
                "frozen": true,
                "filename": "test.txt",
                "bytes": [65, 66, 99],
                "sameFile": true,
            })
        );
    }
}
