use boa_engine::{
    Context, JsError, JsResult, JsValue, NativeFunction, js_string, object::ObjectInitializer,
    property::Attribute,
};
use boa_gc::Gc;
use boa_gc::{Finalize, Trace, empty_trace};
use std::{cell::RefCell, cell::RefMut};

use crate::dispatcher::{DnsAnswer, DnsAnswerKind, DnsResponse};

use super::helpers::{check_argument_count, ensure_exists};

#[derive(Clone)]
pub struct ScriptDnsResponse {
    response: Option<DnsResponse>,
}

impl ScriptDnsResponse {
    fn new() -> Self {
        Self { response: None }
    }

    pub fn into_response(self) -> Option<DnsResponse> {
        self.response
    }

    fn response_mut(&mut self) -> &mut DnsResponse {
        self.response.get_or_insert_with(DnsResponse::default)
    }
}

pub struct DnsResponseCell {
    pub cell: RefCell<ScriptDnsResponse>,
}

impl Finalize for DnsResponseCell {}

// SAFETY: ScriptDnsResponse only stores Rust-owned DNS response data and does not contain
// any Boa GC-managed JavaScript values.
unsafe impl Trace for DnsResponseCell {
    empty_trace!();
}

impl DnsResponseCell {
    fn new() -> Self {
        DnsResponseCell {
            cell: RefCell::new(ScriptDnsResponse::new()),
        }
    }
}

fn get_dns_response_from_context(ctx: &mut Context) -> JsResult<RefMut<'_, ScriptDnsResponse>> {
    Ok(ensure_exists(
        ctx.get_data::<Gc<DnsResponseCell>>(),
        "failed get dns response from context",
    )?
    .cell
    .borrow_mut())
}

fn parse_answer_kind(kind: &str) -> JsResult<DnsAnswerKind> {
    match kind.to_ascii_uppercase().as_str() {
        "A" => Ok(DnsAnswerKind::A),
        "AAAA" => Ok(DnsAnswerKind::AAAA),
        "CNAME" => Ok(DnsAnswerKind::CNAME),
        "TXT" => Ok(DnsAnswerKind::TXT),
        _ => Err(JsError::from_opaque(
            js_string!("unsupported DNS answer type").into(),
        )),
    }
}

pub fn register_dns_response_to_context(context: &mut Context) -> Gc<DnsResponseCell> {
    let response = Gc::new(DnsResponseCell::new());
    context.insert_data(response.clone());

    let mut object_builder = ObjectInitializer::new(context);

    object_builder.function(
        NativeFunction::from_copy_closure(move |_this, args, ctx| {
            check_argument_count(args, 2)?;

            let kind = ensure_exists(args[0].as_string(), "argument 0 must be a string")?
                .to_std_string_lossy();
            let value = ensure_exists(args[1].as_string(), "argument 1 must be a string")?
                .to_std_string_lossy();
            let ttl = args.get(2).and_then(|v| v.as_number()).map(|v| v as u32);
            let kind = parse_answer_kind(&kind)?;

            let mut response = get_dns_response_from_context(ctx)?;
            response
                .response_mut()
                .answers
                .push(DnsAnswer { kind, value, ttl });

            Ok(JsValue::undefined())
        }),
        js_string!("answer"),
        3,
    );

    object_builder.function(
        NativeFunction::from_copy_closure(move |_this, args, ctx| {
            check_argument_count(args, 1)?;

            let code = ensure_exists(args[0].as_string(), "argument 0 must be a string")?
                .to_std_string_lossy();
            let mut response = get_dns_response_from_context(ctx)?;
            response.response_mut().rcode = code;

            Ok(JsValue::undefined())
        }),
        js_string!("rcode"),
        1,
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
