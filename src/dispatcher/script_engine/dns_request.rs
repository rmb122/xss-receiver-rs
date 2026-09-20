use rquickjs::{Ctx, Function, Object, Result, object::Property};

use crate::dispatcher::DnsRequest;

pub fn register_dns_request_to_context<'js>(ctx: &Ctx<'js>, request: &DnsRequest) -> Result<()> {
    let object = Object::new(ctx.clone())?;
    object.set("name", request.name.as_str())?;
    object.set("type", request.query_type.to_string())?;
    object.set("class", request.query_class.as_str())?;
    object.set("clientAddr", request.client_addr.to_string())?;
    let freeze: Function = ctx.eval("Object.freeze")?;
    freeze.call::<_, ()>((object.clone(),))?;
    ctx.globals()
        .prop("request", Property::from(object).enumerable())
}

#[cfg(test)]
mod tests {
    use hickory_proto::rr::RecordType;
    use serde_json::json;

    use super::*;
    use crate::dispatcher::script_engine::{create_context, evaluate_module};

    #[tokio::test(flavor = "current_thread")]
    async fn dns_request_is_readonly() {
        let (_runtime, context) = create_context().await;
        context
            .with(|ctx| {
                register_dns_request_to_context(
                    &ctx,
                    &DnsRequest {
                        client_addr: "127.0.0.1:1234".parse().unwrap(),
                        name: "example.test".to_owned(),
                        query_type: RecordType::A,
                        query_class: "IN".to_owned(),
                    },
                )
                .unwrap()
            })
            .await;
        let value = evaluate_module(
            r#"
                const blocked = [
                    () => { request.name = 'changed'; },
                    () => { request.added = 'changed'; },
                    () => { delete request.name; },
                ].every(attempt => {
                    try { attempt(); return false; }
                    catch (error) { return error instanceof TypeError; }
                });
                export default { blocked, frozen: Object.isFrozen(request), name: request.name };
            "#,
            &context,
        )
        .await
        .unwrap();
        assert_eq!(
            value,
            json!({ "blocked": true, "frozen": true, "name": "example.test" })
        );
    }
}
