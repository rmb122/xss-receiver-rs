use std::{cell::RefCell, rc::Rc};

use axum::http::{HeaderMap, HeaderName, HeaderValue};
use rquickjs::{
    Ctx, Exception, Function, Object, Result, Value,
    function::{Rest, This},
    object::Property,
};

use crate::storage::UserStorage;

use super::helpers::{check_argument_count, ensure_exists, read_u8_array_from_js_value};

#[derive(Clone)]
pub struct HttpResponse {
    pub status_code: u16,
    pub headers: HeaderMap,
    pub body_file: Option<String>,
    pub body: Vec<u8>,
}

pub fn register_response_to_context<'js>(
    ctx: &Ctx<'js>,
    storage: UserStorage,
    headers: HeaderMap,
) -> Result<Rc<RefCell<HttpResponse>>> {
    let response = Rc::new(RefCell::new(HttpResponse {
        status_code: 200,
        headers,
        body_file: None,
        body: Vec::new(),
    }));
    let object = Object::new(ctx.clone())?;

    let shared = response.clone();
    object.set(
        "send",
        Function::new(ctx.clone(), move |ctx: Ctx<'js>, args: Rest<Value<'js>>| {
            check_argument_count(&args, 1, &ctx)?;
            let data = read_u8_array_from_js_value(&args[0], &ctx)?;
            let mut response = shared.borrow_mut();
            if response.body_file.is_some() {
                return Err(Exception::throw_message(
                    &ctx,
                    "response.send() is mutually exclusive with sendFile()",
                ));
            }
            response.body.extend(data);
            Ok(())
        })?,
    )?;

    let shared = response.clone();
    object.set(
        "sendFile",
        Function::new(ctx.clone(), move |ctx: Ctx<'js>, args: Rest<Value<'js>>| {
            check_argument_count(&args, 1, &ctx)?;
            let path = ensure_exists(args[0].as_string(), "argument 0 must be a string", &ctx)?
                .to_string()?;
            let abs_path = storage.absolute_path(&path).map_err(|error| {
                Exception::throw_message(
                    &ctx,
                    &format!("response.sendFile() invalid path: {error}"),
                )
            })?;
            let metadata = storage.metadata(&path).map_err(|error| {
                Exception::throw_message(
                    &ctx,
                    &format!("response.sendFile() cannot access file: {error}"),
                )
            })?;
            if !metadata.is_file() {
                return Err(Exception::throw_message(
                    &ctx,
                    "response.sendFile() path is not a regular file",
                ));
            }
            let mut response = shared.borrow_mut();
            if response.body_file.is_some() {
                return Err(Exception::throw_message(
                    &ctx,
                    "response.sendFile() can only be called once",
                ));
            }
            if !response.body.is_empty() {
                return Err(Exception::throw_message(
                    &ctx,
                    "response.sendFile() is mutually exclusive with send()",
                ));
            }
            response.body_file = Some(abs_path);
            Ok(())
        })?,
    )?;

    let shared = response.clone();
    object.set(
        "setStatus",
        Function::new(
            ctx.clone(),
            move |ctx: Ctx<'js>, This(receiver): This<Object<'js>>, args: Rest<Value<'js>>| {
                check_argument_count(&args, 1, &ctx)?;
                let status =
                    ensure_exists(args[0].as_number(), "status code must be a number", &ctx)?;
                if !status.is_finite()
                    || status.fract() != 0.0
                    || !(100.0..=999.0).contains(&status)
                {
                    return Err(Exception::throw_range(
                        &ctx,
                        "status code must be an integer between 100 and 999",
                    ));
                }
                shared.borrow_mut().status_code = status as u16;
                Ok::<_, rquickjs::Error>(receiver)
            },
        )?,
    )?;

    let shared = response.clone();
    object.set(
        "setHeader",
        Function::new(
            ctx.clone(),
            move |ctx: Ctx<'js>, This(receiver): This<Object<'js>>, args: Rest<Value<'js>>| {
                check_argument_count(&args, 2, &ctx)?;
                let key =
                    ensure_exists(args[0].as_string(), "not a valid string", &ctx)?.to_string()?;
                let values = if let Some(value) = args[1].as_string() {
                    vec![value.to_string()?]
                } else {
                    let array = ensure_exists(
                        args[1].as_array(),
                        "argument 1 not a valid string or string array",
                        &ctx,
                    )?;
                    let length: u32 = array.as_object().get("length")?;
                    let mut values = Vec::new();
                    for index in 0..length {
                        let value = array.get::<Value>(index as usize)?;
                        values.push(
                            ensure_exists(
                                value.as_string(),
                                &format!("not a valid string in array index {index}"),
                                &ctx,
                            )?
                            .to_string()?,
                        );
                    }
                    values
                };
                let name = HeaderName::from_bytes(key.as_bytes()).map_err(|error| {
                    Exception::throw_type(
                        &ctx,
                        &format!("invalid HTTP header name {key:?}: {error}"),
                    )
                })?;
                let values = values
                    .into_iter()
                    .map(|value| {
                        HeaderValue::from_str(&value).map_err(|error| {
                            Exception::throw_type(
                                &ctx,
                                &format!("invalid value for header {key}: {error}"),
                            )
                        })
                    })
                    .collect::<Result<Vec<_>>>()?;

                let mut response = shared.borrow_mut();
                response.headers.remove(&name);
                for value in values {
                    response.headers.append(name.clone(), value);
                }
                Ok::<_, rquickjs::Error>(receiver)
            },
        )?,
    )?;

    let shared = response.clone();
    object.set(
        "removeHeader",
        Function::new(ctx.clone(), move |ctx: Ctx<'js>, args: Rest<Value<'js>>| {
            check_argument_count(&args, 1, &ctx)?;
            let name =
                ensure_exists(args[0].as_string(), "not a valid string", &ctx)?.to_string()?;
            shared.borrow_mut().headers.remove(name.as_str());
            Ok::<_, rquickjs::Error>(())
        })?,
    )?;
    ctx.globals()
        .prop("response", Property::from(object).enumerable())?;
    Ok(response)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use axum::http::HeaderMap;
    use rquickjs::{Context, Runtime};

    use super::register_response_to_context;
    use crate::{
        dispatcher::script_engine::{
            cache::{ScriptCache, register_cache_to_context},
            storage::register_storage_to_context,
            utils::register_utils_to_context,
        },
        storage::UserStorage,
    };

    #[test]
    fn set_status_rejects_invalid_codes_without_changing_the_response() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();
        context.with(|ctx| {
            let response = register_response_to_context(
                &ctx,
                UserStorage::new(std::env::temp_dir()),
                HeaderMap::new(),
            )
            .unwrap();
            let rejected: bool = ctx
                .eval(
                    r#"
                response.setStatus(201);
                ['200', 200.5, NaN, 99, 1000].every(code => {
                    try { response.setStatus(code); return false; }
                    catch (error) {
                        return error instanceof (typeof code === 'number' ? RangeError : TypeError);
                    }
                });
            "#,
                )
                .unwrap();
            assert!(rejected);
            assert_eq!(response.borrow().status_code, 201);
        });
    }

    #[tokio::test(flavor = "current_thread")]
    async fn oversized_sparse_header_arrays_return_errors() {
        use crate::dispatcher::script_engine::{
            create_context, evaluate_module,
            http_client::{ScriptHttpClient, register_http_client_to_context},
        };

        let (_runtime, context) = create_context().await;
        context
            .with(|ctx| {
                register_response_to_context(
                    &ctx,
                    UserStorage::new(std::env::temp_dir()),
                    HeaderMap::new(),
                )?;
                let client = ScriptHttpClient::new(&crate::startup_config::ScriptHttp {
                    allow_private_network: true,
                    ..Default::default()
                })
                .unwrap();
                register_http_client_to_context(&ctx, client)
            })
            .await
            .unwrap();
        let result = evaluate_module(
            r#"
            const values = new Array(2147483648);
            const errors = [];
            try {
                response.setHeader('X-Test', values);
            } catch (error) {
                errors.push(error instanceof TypeError && error.message.includes('index 0'));
            }
            try {
                await http.get('http://127.0.0.1:1/', { headers: { 'X-Test': values } });
            } catch (error) {
                errors.push(error instanceof TypeError && error.message.includes('item 0'));
            }
            export default errors;
        "#,
            &context,
        )
        .await
        .unwrap();
        assert_eq!(result, serde_json::json!([true, true]));
    }

    #[test]
    fn binary_views_roundtrip_through_bindings() {
        let root = std::env::temp_dir().join(format!("xss-bindings-{}", rand::random::<u64>()));
        fs::create_dir_all(&root).unwrap();
        let storage = UserStorage::new(root.clone());
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();
        context.with(|ctx| {
            let response =
                register_response_to_context(&ctx, storage.clone(), HeaderMap::new()).unwrap();
            register_storage_to_context(&ctx, storage).unwrap();
            register_utils_to_context(&ctx).unwrap();
            register_cache_to_context(
                &ctx,
                ScriptCache::new(&crate::startup_config::ScriptCache {
                    max_entries: 10,
                    max_entry_size: 100,
                    max_ttl: 60,
                }),
            )
            .unwrap();
            let encoded: String = ctx
                .eval(
                    r#"
                const bytes = new Uint8Array([99, 65, 66, 67, 88]).subarray(1, 4);
                cache.set("bytes", bytes);
                bytes[0] = 90;
                storage.mkdir("files");
                storage.write("files/body", cache.get("bytes"));
                storage.append("files/body", "!");
                storage.rename("files/body", "files/result");
                response.send(storage.read("files/result"));
                response.send(base64Decode(base64Encode(bytes)));
                response.send(urlDecode(urlEncode(" hello")));
                response.setStatus(201).setHeader("Set-Cookie", ["a=1", "b=2"]);
                if (storage.list("files")[0].size !== 4 || storage.listAll().length !== 1) {
                    throw new Error("unexpected storage listing");
                }
                storage.remove("files/result");
                if (storage.exists("files/result")) throw new Error("remove failed");
                base64Encode(bytes);
            "#,
                )
                .unwrap();
            assert_eq!(encoded, "WkJD");
            let response = response.borrow();
            assert_eq!(response.body, b"ABC!ZBC hello");
            assert_eq!(response.status_code, 201);
            assert_eq!(
                response
                    .headers
                    .get_all("Set-Cookie")
                    .iter()
                    .collect::<Vec<_>>(),
                ["a=1", "b=2"]
            );
        });
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn send_file_validates_paths_and_excludes_body_writes() {
        let root = std::env::temp_dir().join(format!("xss-send-file-{}", rand::random::<u64>()));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("body"), b"file content").unwrap();
        let storage = UserStorage::new(root.clone());
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();
        context.with(|ctx| {
            let response =
                register_response_to_context(&ctx, storage.clone(), HeaderMap::new()).unwrap();
            let errors: i32 = ctx
                .eval(
                    r#"
                let errors = 0;
                for (const path of ["../body", "missing", ""]) {
                    try { response.sendFile(path); } catch (_) { errors++; }
                }
                response.sendFile("body");
                try { response.send("text"); } catch (_) { errors++; }
                try { response.sendFile("body"); } catch (_) { errors++; }
                errors;
            "#,
                )
                .unwrap();
            assert_eq!(errors, 5);
            assert_eq!(
                response.borrow().body_file.as_deref(),
                root.join("body").to_str()
            );
            assert!(response.borrow().body.is_empty());
        });
        let context = Context::full(&runtime).unwrap();
        context.with(|ctx| {
            let response = register_response_to_context(&ctx, storage, HeaderMap::new()).unwrap();
            let rejected: bool = ctx
                .eval(
                    r#"
                response.send("text");
                let rejected = false;
                try { response.sendFile("body"); } catch (_) { rejected = true; }
                rejected;
            "#,
                )
                .unwrap();
            assert!(rejected);
            assert!(response.borrow().body_file.is_none());
            assert_eq!(response.borrow().body, b"text");
        });
        fs::remove_dir_all(root).unwrap();
    }
}
