use std::{net::SocketAddr, str::FromStr, sync::Arc};

use hickory_proto::{
    op::Message,
    rr::{
        Name, RData, Record,
        rdata::{A, AAAA, CNAME, TXT},
    },
};
use tokio::net::UdpSocket;

use crate::{
    controllers::Context,
    db::dns_log::{helper::insert_dns_log, model::NewDnsLog},
    dispatcher::{DnsAnswer, DnsAnswerKind, DnsRequest, DnsResponse, normalize_dns_name},
};

use super::diesel_bytea;

pub async fn start(ctx: Context) -> anyhow::Result<()> {
    let listen = ctx.config.dns_server.listen.clone();
    let socket = Arc::new(UdpSocket::bind(&listen).await?);
    log::info!("dns udp server listening on {}", listen);

    let mut buf = vec![0u8; 4096];
    loop {
        let (len, client_addr) = socket.recv_from(&mut buf).await?;
        let packet = buf[..len].to_vec();
        let socket = socket.clone();
        let ctx = ctx.clone();

        tokio::spawn(async move {
            if let Err(error) = process_packet(ctx, socket, client_addr, packet).await {
                log::error!("dns packet from {} failed: {:?}", client_addr, error);
            }
        });
    }
}

async fn process_packet(
    ctx: Context,
    socket: Arc<UdpSocket>,
    client_addr: SocketAddr,
    packet: Vec<u8>,
) -> anyhow::Result<()> {
    let message = Message::from_vec(&packet)?;
    let query = if let Some(query) = message.queries.first() {
        query.clone()
    } else {
        return Ok(());
    };

    let query_name = normalize_dns_name(&query.name().to_utf8());
    let query_type = query.query_type();
    let query_class = query.query_class().to_string();

    let route = {
        ctx.dns_dispatcher
            .read()
            .expect("lock poisoned")
            .dispatch_key(&query_name)
    };

    let Some(route) = route else {
        return Ok(());
    };

    let request = DnsRequest {
        client_addr,
        name: query_name.clone(),
        query_type,
        query_class: query_class.clone(),
    };

    let result = route.handler.handle(request).await;
    let mut extra_info = serde_json::Value::Null;
    let mut error_log = None;
    let mut response = None;

    match result {
        Ok((handler_extra_info, handler_response)) => {
            extra_info = handler_extra_info;
            response = handler_response;
        }
        Err(error) => {
            error_log = Some(error.to_string());
        }
    }

    if route.write_log {
        let mut conn = ctx.db_conn().await?;
        let _ = insert_dns_log(
            &mut conn,
            &NewDnsLog {
                client_ip: client_addr.ip().to_string(),
                client_port: client_addr.port() as i32,
                location: ctx.locator.locate(&client_addr.ip().to_string()),
                query_name: query_name.clone(),
                query_type: query_type.to_string(),
                query_class,
                extra_info: diesel_bytea::Json(extra_info),
                error_log,
            },
        )
        .await?;
    }

    if let Some(response) = response {
        let response = build_message_response(&message, response)?;
        socket.send_to(&response, client_addr).await?;
    }

    Ok(())
}

fn build_message_response(request: &Message, response: DnsResponse) -> anyhow::Result<Vec<u8>> {
    let mut message = Message::response(request.metadata.id, request.metadata.op_code);
    message.metadata.recursion_desired = request.metadata.recursion_desired;
    message.metadata.response_code = response.response_code();
    message.add_queries(request.queries.clone());

    let Some(query) = request.queries.first() else {
        return Ok(message.to_vec()?);
    };
    let owner = query.name().clone();

    for answer in response.answers {
        let record = build_record(owner.clone(), response.ttl, answer)?;
        message.add_answer(record);
    }

    Ok(message.to_vec()?)
}

fn build_record(owner: Name, default_ttl: u32, answer: DnsAnswer) -> anyhow::Result<Record> {
    let ttl = answer.ttl.unwrap_or(default_ttl);
    let rdata = match answer.kind {
        DnsAnswerKind::A => RData::A(A(std::net::Ipv4Addr::from_str(&answer.value)?)),
        DnsAnswerKind::AAAA => RData::AAAA(AAAA(std::net::Ipv6Addr::from_str(&answer.value)?)),
        DnsAnswerKind::CNAME => RData::CNAME(CNAME(Name::from_ascii(&answer.value)?)),
        DnsAnswerKind::TXT => RData::TXT(TXT::new(vec![answer.value])),
    };

    Ok(Record::from_rdata(owner, ttl, rdata))
}
