use std::{cell::RefCell, rc::Rc};

use rquickjs::{Ctx, Exception, Function, Object, Result, Value, function::Rest, object::Property};

use crate::dispatcher::{DnsAnswer, DnsAnswerKind, DnsResponse};

use super::helpers::{check_argument_count, ensure_exists};

#[derive(Clone)]
pub struct ScriptDnsResponse {
    response: Option<DnsResponse>,
}

impl ScriptDnsResponse {
    pub fn into_response(self) -> Option<DnsResponse> {
        self.response
    }

    fn response_mut(&mut self) -> &mut DnsResponse {
        self.response.get_or_insert_with(DnsResponse::default)
    }
}

fn parse_answer_kind(kind: &str, ctx: &Ctx<'_>) -> Result<DnsAnswerKind> {
    match kind.to_ascii_uppercase().as_str() {
        "A" => Ok(DnsAnswerKind::A),
        "AAAA" => Ok(DnsAnswerKind::AAAA),
        "CNAME" => Ok(DnsAnswerKind::CNAME),
        "TXT" => Ok(DnsAnswerKind::TXT),
        _ => Err(Exception::throw_type(ctx, "unsupported DNS answer type")),
    }
}

pub fn register_dns_response_to_context<'js>(
    ctx: &Ctx<'js>,
) -> Result<Rc<RefCell<ScriptDnsResponse>>> {
    let response = Rc::new(RefCell::new(ScriptDnsResponse { response: None }));
    let object = Object::new(ctx.clone())?;

    let shared = response.clone();
    object.set(
        "answer",
        Function::new(ctx.clone(), move |ctx: Ctx<'js>, args: Rest<Value<'js>>| {
            check_argument_count(&args, 2, &ctx)?;
            let kind = ensure_exists(args[0].as_string(), "argument 0 must be a string", &ctx)?
                .to_string()?;
            let value = ensure_exists(args[1].as_string(), "argument 1 must be a string", &ctx)?
                .to_string()?;
            let ttl = args
                .get(2)
                .and_then(Value::as_number)
                .map(|value| value as u32);
            let kind = parse_answer_kind(&kind, &ctx)?;
            shared
                .borrow_mut()
                .response_mut()
                .answers
                .push(DnsAnswer { kind, value, ttl });
            Ok::<_, rquickjs::Error>(())
        })?,
    )?;

    let shared = response.clone();
    object.set(
        "rcode",
        Function::new(ctx.clone(), move |ctx: Ctx<'js>, args: Rest<Value<'js>>| {
            check_argument_count(&args, 1, &ctx)?;
            let code = ensure_exists(args[0].as_string(), "argument 0 must be a string", &ctx)?
                .to_string()?;
            shared.borrow_mut().response_mut().rcode = code;
            Ok::<_, rquickjs::Error>(())
        })?,
    )?;
    ctx.globals()
        .prop("response", Property::from(object).enumerable())?;
    Ok(response)
}
