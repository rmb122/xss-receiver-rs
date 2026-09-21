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
            let ttl = if let Some(value) = args.get(2).filter(|value| !value.is_undefined()) {
                let ttl = ensure_exists(value.as_number(), "ttl must be a number", &ctx)?;
                if !ttl.is_finite() || ttl.fract() != 0.0 || !(0.0..=u32::MAX as f64).contains(&ttl)
                {
                    return Err(Exception::throw_range(
                        &ctx,
                        "ttl must be an integer between 0 and 4294967295",
                    ));
                }
                Some(ttl as u32)
            } else {
                None
            };
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

#[cfg(test)]
mod tests {
    use rquickjs::{Context, Runtime};

    use super::register_dns_response_to_context;

    #[test]
    fn answer_rejects_invalid_ttl_without_adding_records() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();
        context.with(|ctx| {
            let response = register_dns_response_to_context(&ctx).unwrap();
            let rejected: bool = ctx
                .eval(
                    r#"
                response.answer('A', '192.0.2.1');
                ['60', 0.5, NaN, -1, 4294967296].every(ttl => {
                    try { response.answer('A', '192.0.2.2', ttl); return false; }
                    catch (error) {
                        return error instanceof (typeof ttl === 'number' ? RangeError : TypeError);
                    }
                });
            "#,
                )
                .unwrap();
            assert!(rejected);
            let response = response.borrow().clone().into_response().unwrap();
            assert_eq!(response.answers.len(), 1);
        });
    }
}
