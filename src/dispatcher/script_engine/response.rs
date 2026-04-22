use boa_engine::JsValue;
use boa_engine::object::builtins::JsArray;
use boa_engine::{
    Context, JsError, NativeFunction, js_string, object::ObjectInitializer, property::Attribute,
};
use boa_gc::Gc;
use boa_gc::{Finalize, Trace, empty_trace};
use std::{cell::RefCell, collections::HashMap};

use super::helpers::{
    check_argument_count, ensure_exists, get_response_from_context, read_u8_array_from_js_value,
};
use super::storage::get_storage_from_context;

/// Response 数据结构
#[derive(Clone)]
pub struct Response {
    pub status_code: u16,
    pub headers: HashMap<String, Vec<String>>,
    pub body_file: Option<String>,
    pub body: Vec<u8>,
}

impl Response {
    fn new() -> Self {
        Response {
            status_code: 200,
            headers: HashMap::new(),
            body_file: None,
            body: Vec::new(),
        }
    }
}

/// ResponseCell 用于在 JS 引擎中共享 Response
pub struct ResponseCell {
    pub cell: RefCell<Response>,
}

impl Finalize for ResponseCell {}

// SAFETY: Response 里面不存储来自 js 引擎的东西, 不用具体实现 trace
unsafe impl Trace for ResponseCell {
    empty_trace!();
}

// SAFETY: JS 引擎单线程的
unsafe impl Sync for ResponseCell {}

impl ResponseCell {
    fn new() -> Self {
        ResponseCell {
            cell: RefCell::new(Response::new()),
        }
    }
}

/// 注册 Response 对象到 JS 上下文
pub fn register_response_to_context(context: &mut Context) -> Gc<ResponseCell> {
    let response = Gc::new(ResponseCell::new());
    context.insert_data(response.clone());

    let mut object_builder = ObjectInitializer::new(context);

    // response.send(data: String | Uint8Array): void
    object_builder.function(
        NativeFunction::from_copy_closure(move |_this, args, ctx| {
            check_argument_count(args, 1)?;

            let data = read_u8_array_from_js_value(&args[0], ctx)?;
            let mut response = get_response_from_context(ctx)?;
            if response.body_file.is_some() {
                return Err(JsError::from_opaque(
                    js_string!("response.send() is mutually exclusive with sendFile()").into(),
                ));
            }
            response.body.extend(data);

            Ok(JsValue::undefined())
        }),
        js_string!("send"),
        1,
    );

    // response.sendFile(path: String): void
    // 只能调用一次, 且与 send() 互斥
    object_builder.function(
        NativeFunction::from_copy_closure(move |_this, args, ctx| {
            check_argument_count(args, 1)?;

            let path = ensure_exists(args[0].as_string(), "argument 0 must be a string")?
                .to_std_string_lossy();

            // 通过 storage 的 resolve 校验路径并拿到绝对路径 (同时验证文件存在)
            let storage = get_storage_from_context(ctx)?;
            let abs_path = storage.storage.absolute_path(&path).map_err(|e| {
                JsError::from_opaque(
                    js_string!(format!("response.sendFile() invalid path: {}", e)).into(),
                )
            })?;
            let metadata = storage.storage.metadata(&path).map_err(|e| {
                JsError::from_opaque(
                    js_string!(format!("response.sendFile() cannot access file: {}", e)).into(),
                )
            })?;
            if !metadata.is_file() {
                return Err(JsError::from_opaque(
                    js_string!("response.sendFile() path is not a regular file").into(),
                ));
            }

            let mut response = get_response_from_context(ctx)?;
            if response.body_file.is_some() {
                return Err(JsError::from_opaque(
                    js_string!("response.sendFile() can only be called once").into(),
                ));
            }
            if !response.body.is_empty() {
                return Err(JsError::from_opaque(
                    js_string!("response.sendFile() is mutually exclusive with send()").into(),
                ));
            }
            response.body_file = Some(abs_path);

            Ok(JsValue::undefined())
        }),
        js_string!("sendFile"),
        1,
    );

    // response.sendStatus(code: Number): void
    object_builder.function(
        NativeFunction::from_copy_closure(move |_this, args, ctx| {
            check_argument_count(args, 1)?;

            let status_code = ensure_exists(args[0].as_number(), "not a valid number")?;
            let mut response = get_response_from_context(ctx)?;
            response.status_code = status_code as u16;

            Ok(JsValue::undefined())
        }),
        js_string!("sendStatus"),
        1,
    );

    // response.sendHeader(key: String, value: String | Array<String>): void
    object_builder.function(
        NativeFunction::from_copy_closure(move |_this, args, ctx| {
            check_argument_count(args, 2)?;

            let key =
                ensure_exists(args[0].as_string(), "not a valid string")?.to_std_string_lossy();
            let value = &args[1];

            if let Some(value) = value.as_string() {
                get_response_from_context(ctx)?
                    .headers
                    .insert(key, vec![value.to_std_string_lossy()]);
            } else {
                let value_array = JsArray::from_object(
                    ensure_exists(
                        value.as_object(),
                        "argument 1 not a valid string or string array",
                    )?
                    .to_owned(),
                )?;
                let value_array_length = value_array.length(ctx)?;

                let mut value_vec = vec![];

                for idx in 0..value_array_length {
                    value_vec.push(
                        ensure_exists(
                            value_array.get(idx, ctx)?.as_string(),
                            &format!("not a valid string in array index {}", idx),
                        )?
                        .to_std_string_lossy(),
                    );
                }

                get_response_from_context(ctx)?
                    .headers
                    .insert(key, value_vec);
            }

            Ok(JsValue::undefined())
        }),
        js_string!("sendHeader"),
        2,
    );

    let object = object_builder.build();
    context
        .register_global_property(
            js_string!("response"),
            object,
            Attribute::READONLY | Attribute::ENUMERABLE,
        )
        .expect("property shouldn't exist");

    response
}
