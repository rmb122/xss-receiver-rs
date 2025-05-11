use std::cell::RefMut;
use std::ops::Deref;
use std::{cell::RefCell, collections::HashMap};

use boa_engine::object::builtins::{JsArrayBuffer, JsUint8Array};
use boa_engine::{
    Context, JsString, NativeFunction, js_string, object::ObjectInitializer, property::Attribute,
};
use boa_engine::{JsError, JsResult, JsValue};
use boa_gc::Gc;
use boa_gc::{Finalize, Trace, empty_trace};

use crate::parsed_request::ParsedRequest;

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

fn register_response_to_context(context: &mut Context) -> Gc<ResponseCell> {
    let response = Gc::new(ResponseCell::new());
    context.insert_data(response.clone());

    let object = ObjectInitializer::new(context)
        .function(
            NativeFunction::from_copy_closure(move |_this, args, ctx| {
                let data = read_u8_array_from_js_value(&args[0], ctx)?;

                let mut response = get_response_from_context(ctx)?;
                response.body.extend(data);

                Ok(boa_engine::JsValue::Undefined)
            }),
            js_string!("send"),
            1,
        )
        .build();

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
