use boa_engine::{
    Context, JsValue, js_string,
    object::{IntegrityLevel, ObjectInitializer},
    property::Attribute,
};

use crate::dispatcher::DnsRequest;

pub fn register_dns_request_to_context(context: &mut Context, request: &DnsRequest) {
    let object = ObjectInitializer::new(context)
        .property(
            js_string!("name"),
            JsValue::from(js_string!(request.name.as_str())),
            Attribute::READONLY | Attribute::ENUMERABLE,
        )
        .property(
            js_string!("type"),
            JsValue::from(js_string!(request.query_type.to_string().as_str())),
            Attribute::READONLY | Attribute::ENUMERABLE,
        )
        .property(
            js_string!("class"),
            JsValue::from(js_string!(request.query_class.as_str())),
            Attribute::READONLY | Attribute::ENUMERABLE,
        )
        .property(
            js_string!("clientAddr"),
            JsValue::from(js_string!(request.client_addr.to_string().as_str())),
            Attribute::READONLY | Attribute::ENUMERABLE,
        )
        .build();
    assert!(
        object
            .set_integrity_level(IntegrityLevel::Frozen, context)
            .expect("failed to freeze DNS request data")
    );

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
    use hickory_proto::rr::RecordType;
    use serde_json::json;

    use super::*;
    use crate::dispatcher::script_engine::{create_context, evaluate_module};

    #[tokio::test(flavor = "current_thread")]
    async fn dns_request_is_readonly() {
        let (mut context, executor) = create_context();
        register_dns_request_to_context(
            &mut context,
            &DnsRequest {
                client_addr: "127.0.0.1:1234".parse().unwrap(),
                name: "example.test".to_owned(),
                query_type: RecordType::A,
                query_class: "IN".to_owned(),
            },
        );
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
            &mut context,
            executor,
        )
        .await
        .unwrap();
        assert_eq!(
            value,
            json!({ "blocked": true, "frozen": true, "name": "example.test" })
        );
    }
}
