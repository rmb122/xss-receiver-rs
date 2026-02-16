use std::cell::RefMut;
use std::ops::Deref;
use std::{cell::RefCell, collections::HashMap};

use boa_engine::object::builtins::{JsArray, JsArrayBuffer, JsUint8Array};
use boa_engine::{
    Context, JsString, NativeFunction, js_string, object::ObjectInitializer, property::Attribute,
};
use boa_engine::{JsError, JsResult, JsValue};
use boa_gc::Gc;
use boa_gc::{Finalize, Trace, empty_trace};

use crate::utils::parsed_request::ParsedRequest;

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

#[derive(Clone)]
pub struct Response {
    pub status_code: u16,
    pub headers: HashMap<String, Vec<String>>,
    pub body: Vec<u8>,
}

impl Response {
    fn new() -> Self {
        Response {
            status_code: 200,
            headers: HashMap::new(),
            body: Vec::new(),
        }
    }
}

fn ensure_exists<T>(option: Option<T>, msg: &str) -> JsResult<T> {
    Ok(option.ok_or_else(|| JsError::from_opaque(js_string!(msg).into()))?)
}

fn read_data_from_uint8_array(value: &JsValue, ctx: &mut Context) -> JsResult<Vec<u8>> {
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

fn read_u8_array_from_js_value(value: &JsValue, ctx: &mut Context) -> JsResult<Vec<u8>> {
    if let Some(string) = value.as_string() {
        Ok(string.to_std_string_lossy().as_bytes().to_vec())
    } else {
        read_data_from_uint8_array(value, ctx)
    }
}

fn get_response_from_context(ctx: &mut Context) -> JsResult<RefMut<'_, Response>> {
    Ok(ensure_exists(
        ctx.get_data::<Gc<ResponseCell>>(),
        "failed get response from context",
    )?
    .cell
    .borrow_mut())
}

fn register_request_to_context(context: &mut Context, request: &ParsedRequest) {
    let object = ObjectInitializer::new(context)
        .property(
            js_string!("client_addr"),
            JsString::from(request.client_addr.to_string()),
            Attribute::READONLY | Attribute::ENUMERABLE,
        )
        .build();

    context
        .register_global_property(
            js_string!("request"),
            object,
            Attribute::READONLY | Attribute::ENUMERABLE,
        )
        .expect("property shouldn't exist");
}

fn check_argument_count(args: &[JsValue], count: usize) -> JsResult<()> {
    for i in 0..count {
        ensure_exists(args.get(i), &format!("argument {} not found", i))?;
    }
    Ok(())
}

fn register_response_to_context(context: &mut Context) -> Gc<ResponseCell> {
    let response = Gc::new(ResponseCell::new());
    context.insert_data(response.clone());

    let mut object_builder = ObjectInitializer::new(context);

    object_builder.function(
        NativeFunction::from_copy_closure(move |_this, args, ctx| {
            check_argument_count(args, 1)?;

            let data = read_u8_array_from_js_value(&args[0], ctx)?;
            let mut response = get_response_from_context(ctx)?;
            response.body.extend(data);

            Ok(boa_engine::JsValue::undefined())
        }),
        js_string!("send"),
        1,
    );

    object_builder.function(
        NativeFunction::from_copy_closure(move |_this, args, ctx| {
            check_argument_count(args, 1)?;

            let status_code = ensure_exists(args[0].as_number(), "not a valid number")?;
            let mut response = get_response_from_context(ctx)?;
            response.status_code = status_code as u16;

            Ok(boa_engine::JsValue::undefined())
        }),
        js_string!("sendStatus"),
        1,
    );

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

            Ok(boa_engine::JsValue::undefined())
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

pub fn register_vars_to_context(
    context: &mut Context,
    request: &ParsedRequest,
) -> Gc<ResponseCell> {
    register_request_to_context(context, request);
    register_response_to_context(context)
}
