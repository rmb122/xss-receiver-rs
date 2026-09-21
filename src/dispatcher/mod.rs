mod dispatcher;
mod dns_route;
mod http_route;
mod script_engine;

pub use dispatcher::{DispatchRoute, DnsDispatcher, HttpDispatcher};
pub use dns_route::{
    DnsAnswer, DnsAnswerKind, DnsRequest, DnsResponse, DnsRoute, normalize_dns_name,
};
pub use http_route::HttpRoute;
pub use script_engine::cache::ScriptCache;
pub use script_engine::http_client::ScriptHttpClient;

#[cfg(test)]
mod script_timeout_tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        process::{Command, Stdio},
        sync::atomic::{AtomicU64, Ordering},
        thread,
        time::{Duration, Instant},
    };

    use axum::{
        body::{Body, to_bytes},
        http::{HeaderMap, Request},
    };
    use hickory_proto::rr::RecordType;
    use tokio::{io::AsyncReadExt, net::TcpListener, sync::oneshot};

    use crate::{storage::UserStorage, utils::parsed_request::ParsedRequest};

    use super::{
        DnsRequest, ScriptCache, ScriptHttpClient,
        dns_route::{DnsRouteHandler, ScriptDnsHandler},
        http_route::{HttpRouteHandler, ScriptHttpHandler},
        script_engine::ScriptError,
    };

    const CHILD_TEST: &str = "XSS_RECEIVER_TIMEOUT_TEST_CHILD";
    const CHILD_ROOT: &str = "XSS_RECEIVER_TIMEOUT_TEST_ROOT";
    static NEXT_TEMP_ROOT: AtomicU64 = AtomicU64::new(0);

    struct TempScriptRoot(PathBuf);

    impl TempScriptRoot {
        fn new() -> Self {
            let id = NEXT_TEMP_ROOT.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "xss-receiver-handler-timeout-{}-{id}",
                std::process::id()
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempScriptRoot {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    // A Tokio timeout cannot stop a stuck spawn_blocking worker, even during runtime
    // shutdown. Run each regression in a subprocess so a broken VM cannot hang cargo test.
    fn with_watchdog(test_name: &str, test: impl FnOnce(&Path)) {
        // libtest lists test paths without the crate name included by module_path!().
        let test_name = test_name.split_once("::").unwrap().1;
        if std::env::var(CHILD_TEST).as_deref() == Ok(test_name) {
            test(&PathBuf::from(std::env::var_os(CHILD_ROOT).unwrap()));
            return;
        }

        let root = TempScriptRoot::new();
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", test_name, "--nocapture"])
            .env(CHILD_TEST, test_name)
            .env(CHILD_ROOT, &root.0)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let started = Instant::now();
        loop {
            if child.try_wait().unwrap().is_some() {
                let output = child.wait_with_output().unwrap();
                assert!(String::from_utf8_lossy(&output.stdout).contains("running 1 test"));
                assert!(
                    output.status.success(),
                    "{test_name} failed:\n{}\n{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr),
                );
                return;
            }
            if started.elapsed() > Duration::from_secs(5) {
                let _ = child.kill();
                let output = child.wait_with_output().unwrap();
                panic!(
                    "{test_name} exceeded the 5 second watchdog:\n{}\n{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr),
                );
            }
            thread::sleep(Duration::from_millis(10));
        }
    }

    fn write_script(root: &Path, source: &str) -> String {
        let path = root.join("handler.js");
        fs::write(&path, source).unwrap();
        path.to_str().unwrap().to_owned()
    }

    fn http_handler(root: &Path, source: &str, timeout: i32) -> ScriptHttpHandler {
        ScriptHttpHandler::new(
            write_script(root, source),
            timeout,
            UserStorage::new(root.to_path_buf()),
            ScriptCache::new(&Default::default()),
            ScriptHttpClient::new(&crate::startup_config::ScriptHttp {
                allow_private_network: true,
                ..Default::default()
            })
            .unwrap(),
        )
    }

    async fn http_request() -> ParsedRequest {
        ParsedRequest::new(
            "127.0.0.1:12345".parse().unwrap(),
            Request::builder().uri("/test").body(Body::empty()).unwrap(),
            1024,
        )
        .await
        .unwrap()
    }

    fn assert_timeout<T>(result: anyhow::Result<T>, started: Instant) {
        let error = result.err().expect("script should time out");
        assert!(error.downcast_ref::<ScriptError>().is_some(), "{error:#}");
        assert!(
            error.to_string().contains("script running timeout"),
            "{error:#}"
        );
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    async fn assert_http_recovers(handler: &ScriptHttpHandler, root: &Path) {
        write_script(root, "response.send('healthy'); export default 42;");
        let (value, response) = handler
            .handle(http_request().await, HeaderMap::new())
            .await
            .unwrap();
        assert_eq!(value, serde_json::json!(42));
        assert_eq!(
            to_bytes(response.into_body(), 1024).await.unwrap(),
            "healthy"
        );
    }

    fn assert_http_timeout(root: &Path, source: &str) {
        let handler = http_handler(root, source, 20);
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let request = http_request().await;
                let started = Instant::now();
                assert_timeout(handler.handle(request, HeaderMap::new()).await, started);
                assert_http_recovers(&handler, root).await;
            });
    }

    macro_rules! http_timeout_test {
        ($name:ident, $source:literal) => {
            #[test]
            fn $name() {
                with_watchdog(concat!(module_path!(), "::", stringify!($name)), |root| {
                    assert_http_timeout(root, $source);
                });
            }
        };
    }

    http_timeout_test!(
        http_handler_timeout_cannot_be_caught_by_script,
        "try { while (true) {} } catch (_) {} export default 'escaped';"
    );
    http_timeout_test!(
        http_handler_interrupts_loop_during_serialization,
        "export default { toJSON() { while (true) {} } };"
    );

    #[test]
    fn dns_handler_interrupts_loop_after_await() {
        with_watchdog(
            concat!(module_path!(), "::dns_handler_interrupts_loop_after_await"),
            |root| {
                let handler = ScriptDnsHandler::new(
                    write_script(
                        root,
                        "await Promise.resolve(); while (true) {} export default null;",
                    ),
                    20,
                    UserStorage::new(root.to_path_buf()),
                    ScriptCache::new(&Default::default()),
                    ScriptHttpClient::new(&Default::default()).unwrap(),
                );
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap()
                    .block_on(async {
                        let request = DnsRequest {
                            client_addr: "127.0.0.1:12345".parse().unwrap(),
                            name: "loop.example".to_owned(),
                            query_type: RecordType::A,
                            query_class: "IN".to_owned(),
                        };
                        let started = Instant::now();
                        assert_timeout(handler.handle(request.clone()).await, started);
                        write_script(
                            root,
                            "response.answer('A', '192.0.2.1'); export default 42;",
                        );
                        let (value, response) = handler.handle(request).await.unwrap();
                        assert_eq!(value, serde_json::json!(42));
                        assert_eq!(response.unwrap().answers[0].value, "192.0.2.1");
                    });
            },
        );
    }

    #[test]
    fn http_handler_timeout_drops_pending_outbound_request() {
        with_watchdog(
            concat!(
                module_path!(),
                "::http_handler_timeout_drops_pending_outbound_request"
            ),
            |root| {
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap()
                    .block_on(async {
                        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
                        let address = listener.local_addr().unwrap();
                        let (received, mut request_received) = oneshot::channel();
                        let server = tokio::spawn(async move {
                            let (mut stream, _) = listener.accept().await.unwrap();
                            let mut buffer = [0; 1024];
                            assert!(stream.read(&mut buffer).await.unwrap() > 0);
                            received.send(()).unwrap();
                            // Keep the response pending until the route timeout cancels the request.
                            std::future::pending::<()>().await;
                            drop(stream);
                        });
                        let handler = http_handler(
                            root,
                            &format!(
                                "await http.get('http://{address}/slow'); export default 'unexpected';"
                            ),
                            250,
                        );
                        let request = http_request().await;
                        let started = Instant::now();
                        assert_timeout(handler.handle(request, HeaderMap::new()).await, started);
                        request_received
                            .try_recv()
                            .expect("outbound HTTP request must have started");
                        assert_http_recovers(&handler, root).await;
                        server.abort();
                        let _ = server.await;
                    });
            },
        );
    }
}
