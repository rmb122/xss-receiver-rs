use boa_engine::{Context, JsValue, js_string, object::ObjectInitializer, property::Attribute};

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

    context
        .register_global_property(
            js_string!("request"),
            object,
            Attribute::READONLY | Attribute::ENUMERABLE,
        )
        .expect("property shouldn't exist");
}
