use rquickjs::{Array, Ctx, Exception, Function, Object, Result, TypedArray, Value};
use rquickjs::{function::This, object::Property};
use std::collections::BTreeSet;

use crate::utils::multimap::MultiMap;
use crate::utils::parsed_request::{ParsedRequest, ParsedRequestBody, normalize_header_name};

use super::helpers::json_value_to_js_value;

/// Read the first value of a multimap entry without coercing the lookup key.
fn multimap_get<'js>(
    ctx: Ctx<'js>,
    This(object): This<Object<'js>>,
    key: Value<'js>,
) -> Result<Value<'js>> {
    let key = key
        .as_string()
        .ok_or_else(|| Exception::throw_type(&ctx, "key must be string"))?
        .to_string()?;
    first_map_value(&ctx, &object, &key)
}

fn first_map_value<'js>(ctx: &Ctx<'js>, object: &Object<'js>, key: &str) -> Result<Value<'js>> {
    let values: Value = object.get(key)?;
    match values.as_object() {
        Some(values) => values.get(0),
        None => Ok(Value::new_undefined(ctx.clone())),
    }
}

fn headers_get<'js>(
    ctx: Ctx<'js>,
    This(object): This<Object<'js>>,
    key: Value<'js>,
) -> Result<Value<'js>> {
    let key = key
        .as_string()
        .ok_or_else(|| Exception::throw_type(&ctx, "key must be string"))?
        .to_string()?;
    // The exact name "get" is reserved for the method. The proxy normalizes
    // every other property name, including "Get", once.
    first_map_value(&ctx, &object, if key == "get" { "Get" } else { &key })
}

fn create_multimap_object<'js>(
    ctx: &Ctx<'js>,
    multimap: &MultiMap<String, String>,
    get: Function<'js>,
) -> Result<Object<'js>> {
    let object = Object::new(ctx.clone())?;
    object.prop("get", Property::from(get).writable().configurable())?;
    let keys: BTreeSet<&String> = multimap.iter().map(|(key, _)| key).collect();
    for key in keys {
        if let Some(values) = multimap.get_all(key) {
            let array = Array::new(ctx.clone())?;
            for (index, value) in values.iter().enumerate() {
                array.set(index, value.as_str())?;
            }
            // Defining an own property also handles names such as "__proto__".
            object.prop(
                key.as_str(),
                Property::from(array).writable().configurable().enumerable(),
            )?;
        }
    }
    Ok(object)
}

fn create_headers_object<'js>(
    ctx: &Ctx<'js>,
    headers: &MultiMap<String, String>,
) -> Result<Object<'js>> {
    let target = create_multimap_object(ctx, headers, Function::new(ctx.clone(), headers_get)?)?;
    let normalize = Function::new(ctx.clone(), |name: String| normalize_header_name(&name))?;
    // Capture the intrinsics before running user code. Header names are folded
    // by the same Rust function that groups incoming request headers.
    let create_proxy: Function = ctx.eval(
        r#"(target, normalize) => {
            const hasOwn = Object.hasOwn;
            const get = Reflect.get;
            return new Proxy(target, {
                get(target, name) {
                    if (typeof name !== 'string') return get(target, name);
                    const key = name === 'get' ? name : normalize(name);
                    return hasOwn(target, key) ? get(target, key) : undefined;
                }
            });
        }"#,
    )?;
    create_proxy.call((target, normalize))
}

fn create_file_object<'js>(filename: &str, content: &[u8], ctx: &Ctx<'js>) -> Result<Object<'js>> {
    let object = Object::new_proto(ctx.clone(), None)?;
    object.set("filename", filename)?;
    object.set("content", TypedArray::new_copy(ctx.clone(), content)?)?;
    Ok(object)
}

fn create_upload_files_object<'js>(
    ctx: &Ctx<'js>,
    files: &MultiMap<String, (String, Vec<u8>)>,
) -> Result<Object<'js>> {
    let object = Object::new(ctx.clone())?;
    object.prop(
        "get",
        Property::from(Function::new(ctx.clone(), multimap_get)?)
            .writable()
            .configurable(),
    )?;
    let keys: BTreeSet<&String> = files.iter().map(|(key, _)| key).collect();
    for key in keys {
        if let Some(files) = files.get_all(key) {
            let array = Array::new(ctx.clone())?;
            for (index, (filename, content)) in files.iter().enumerate() {
                array.set(index, create_file_object(filename, content, ctx)?)?;
            }
            object.prop(
                key.as_str(),
                Property::from(array).writable().configurable().enumerable(),
            )?;
        }
    }
    Ok(object)
}

pub fn register_http_request_to_context<'js>(
    ctx: &Ctx<'js>,
    request: &ParsedRequest,
) -> Result<()> {
    let empty_form = MultiMap::new();
    let empty_files = MultiMap::new();
    let (json, forms, files) = match &request.parsed_body {
        ParsedRequestBody::Json(value) => (
            json_value_to_js_value(value, ctx)?,
            &empty_form,
            &empty_files,
        ),
        ParsedRequestBody::Form(forms, files) => (
            Object::new_proto(ctx.clone(), None)?.into_value(),
            forms,
            files,
        ),
        ParsedRequestBody::None | ParsedRequestBody::Failed => (
            Object::new_proto(ctx.clone(), None)?.into_value(),
            &empty_form,
            &empty_files,
        ),
    };
    let object = Object::new(ctx.clone())?;
    object.set("method", request.method.as_str())?;
    object.set("path", request.path.as_str())?;
    object.set("clientAddr", request.client_addr.to_string())?;
    object.set(
        "body",
        TypedArray::new_copy(ctx.clone(), &request.raw_body)?,
    )?;
    object.set("headers", create_headers_object(ctx, &request.headers)?)?;
    object.set(
        "query",
        create_multimap_object(
            ctx,
            &request.parsed_query,
            Function::new(ctx.clone(), multimap_get)?,
        )?,
    )?;
    object.set("json", json)?;
    object.set(
        "forms",
        create_multimap_object(ctx, forms, Function::new(ctx.clone(), multimap_get)?)?,
    )?;
    object.set("files", create_upload_files_object(ctx, files)?)?;
    // Request data is acyclic. The buffer properties are fixed, while their
    // bytes intentionally retain the normal writable Uint8Array API.
    let freeze: Function = ctx.eval(
        r#"(function freeze(value) {
            if (value === null || typeof value !== 'object' || value instanceof Uint8Array) return;
            for (const key of Reflect.ownKeys(value)) freeze(value[key]);
            Object.freeze(value);
        })"#,
    )?;
    freeze.call::<_, ()>((object.clone(),))?;
    ctx.globals()
        .prop("request", Property::from(object).enumerable())
}

#[cfg(test)]
mod tests {
    use axum::{body::Body, http::Request};
    use serde_json::json;

    use super::*;
    use crate::dispatcher::script_engine::{create_context, evaluate_module};

    async fn evaluate_request_module(request: &ParsedRequest, source: &str) -> serde_json::Value {
        let (_runtime, context) = create_context().await;
        context
            .with(|ctx| register_http_request_to_context(&ctx, request).unwrap())
            .await;
        evaluate_module(source, &context).await.unwrap()
    }

    async fn evaluate_request(request: &ParsedRequest, source: &str) -> serde_json::Value {
        let (_runtime, context) = create_context().await;
        context
            .with(|ctx| {
                register_http_request_to_context(&ctx, request).unwrap();
                let value: Value = ctx.eval(source).unwrap();
                let json = ctx
                    .json_stringify(value)
                    .unwrap()
                    .unwrap()
                    .to_string()
                    .unwrap();
                serde_json::from_str(&json).unwrap()
            })
            .await
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
        )
        .await;

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
        )
        .await;

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
            )
            .await,
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
