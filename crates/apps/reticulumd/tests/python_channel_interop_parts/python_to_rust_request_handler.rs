use std::collections::HashSet;
use std::sync::atomic::{AtomicUsize, Ordering};

use rns_transport::destination::link::LinkEventData;
use rns_transport::destination::request::{
    InboundRequest, RequestAllow, RequestHandler, RequestId, RequestResponse,
};
use rns_transport::destination::SingleInputDestination;

const REQUEST_HANDLER_PATH: &str = "/test/request";

#[derive(Default)]
struct RequestHandlerRecord {
    seen: Arc<StdMutex<Vec<(RequestId, rmpv::Value)>>>,
    calls: Arc<AtomicUsize>,
}

impl RequestHandlerRecord {
    fn handler(
        &self,
        respond: fn(&rmpv::Value) -> Option<RequestResponse>,
    ) -> impl RequestHandler + 'static {
        let seen = self.seen.clone();
        let calls = self.calls.clone();
        move |request: InboundRequest| {
            calls.fetch_add(1, Ordering::SeqCst);
            let response = respond(request.data());
            seen.lock().expect("seen lock").push((request.request_id(), request.into_data()));
            std::future::ready(response)
        }
    }

    fn single_request(&self) -> (RequestId, rmpv::Value) {
        let seen = self.seen.lock().expect("seen lock");
        assert_eq!(seen.len(), 1, "request handler ran {} times", seen.len());
        seen[0].clone()
    }
}

fn request_handler_echo(data: &rmpv::Value) -> Option<RequestResponse> {
    let text = rmpv_to_string(data).unwrap_or_default();
    Some(RequestResponse::Value(rmpv::Value::String(format!("reply:{text}").into())))
}

fn request_handler_bytes_body() -> Vec<u8> {
    (0..8192u32).map(|i| (i % 251) as u8).collect()
}

fn request_handler_bytes(_data: &rmpv::Value) -> Option<RequestResponse> {
    Some(RequestResponse::Bytes(request_handler_bytes_body()))
}

struct RequestHandlerFixture {
    paths: PythonChannelInteropPaths,
    _temp: tempfile::TempDir,
    py_config_dir: std::path::PathBuf,
    transport: Transport,
    destination: Arc<tokio::sync::Mutex<SingleInputDestination>>,
    destination_address: AddressHash,
}

impl RequestHandlerFixture {
    #[deny(clippy::await_holding_invalid_type)]
    async fn start(config_dir_name: &str) -> Self {
        let paths = python_channel_interop_paths();

        let server_port = free_tcp_port();
        let temp = tempfile::tempdir().expect("tempdir");
        let py_config_dir = temp.path().join(config_dir_name);
        fs::create_dir_all(&py_config_dir).expect("python config dir");
        write_python_client_config(&py_config_dir, server_port);

        let rust_identity = PrivateIdentity::new_from_rand(OsRng);
        let rust_identity = to_transport_private_identity(&rust_identity);
        let mut config = TransportConfig::new(
            "python-request-handler-interop-rust-server",
            &rust_identity,
            false,
        );
        config.set_path_request_timeout_secs(2);
        let transport = Transport::new(config);
        let iface_manager = transport.iface_manager();
        transport.iface_manager().lock().await.spawn(
            TcpServer::new(format!("127.0.0.1:{server_port}"), iface_manager),
            TcpServer::spawn,
        );
        wait_for_port(server_port, Duration::from_secs(5)).await;

        let destination = transport
            .add_destination(rust_identity.clone(), DestinationName::new("test", "channel"))
            .await;
        let destination_address = { destination.lock().await.desc.address_hash };

        Self { paths, _temp: temp, py_config_dir, transport, destination, destination_address }
    }

    #[deny(clippy::await_holding_invalid_type)]
    async fn register(&self, handler: impl RequestHandler + 'static, allow: RequestAllow) {
        self.destination
            .lock()
            .await
            .register_request_handler(REQUEST_HANDLER_PATH, Arc::new(handler), allow, true)
            .expect("handler registers");
    }

    #[deny(clippy::await_holding_invalid_type)]
    async fn connect(&self, client_timeout: f64, extra_args: &[&str]) -> RequestHandlerClient {
        let child = self.paths.spawn_channel_client_with_args(
            &self.py_config_dir,
            &hex::encode(self.destination_address.as_slice()),
            "request-forms",
            client_timeout,
            extra_args,
        );
        let guard = ChildGuard { child: Some(child) };

        let mut in_events = self.transport.in_link_events();
        let link_id = wait_for_in_link_active_with_announces(
            &self.transport,
            &self.destination,
            &mut in_events,
            Duration::from_secs(8),
        )
        .await;
        RequestHandlerClient { guard, link_id, in_events }
    }

    // A client started with --identify holds its Link.request until a plain
    // `go` data packet arrives, which gives the allow list time to be built.
    #[deny(clippy::await_holding_invalid_type)]
    async fn release_client(&self) {
        self.transport.send_to_in_links(&self.destination_address, b"go").await;
    }
}

struct RequestHandlerClient {
    guard: ChildGuard,
    link_id: AddressHash,
    in_events: tokio::sync::broadcast::Receiver<LinkEventData>,
}

impl RequestHandlerClient {
    // The client prints its terminal report only at exit, so stdout is read
    // in one piece once the process is gone.
    #[deny(clippy::await_holding_invalid_type)]
    async fn json_lines(&mut self) -> Vec<serde_json::Value> {
        let child = self.guard.child.take().expect("python child");
        let output = tokio::time::timeout(
            Duration::from_secs(60),
            tokio::task::spawn_blocking(move || child.wait_with_output()),
        )
        .await
        .expect("python request client did not exit")
        .expect("join python client")
        .expect("wait for python client");
        if !output.status.success() {
            panic!(
                "python request client failed\nstdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        stdout.lines().filter_map(|line| serde_json::from_str(line).ok()).collect()
    }
}

fn request_handler_line_with<'a>(
    lines: &'a [serde_json::Value],
    key: &str,
) -> &'a serde_json::Value {
    lines
        .iter()
        .find(|line| line.get(key).is_some())
        .unwrap_or_else(|| panic!("python client printed no {key:?} line: {lines:?}"))
}

fn request_handler_assert_response(
    lines: &[serde_json::Value],
    rust_request_id: RequestId,
    expected_body: &[u8],
) {
    let sent = request_handler_line_with(lines, "request_sent");
    let report = request_handler_line_with(lines, "response_size");
    assert_eq!(report["response_size"], expected_body.len(), "python response size");
    assert_eq!(report["response_sha256"], digest_hex(expected_body), "python response digest");
    assert_eq!(sent["request_id"], report["request_id"]);

    let rust_hex = rust_request_id.to_string();
    let python_hex = report["request_id"].as_str().expect("python request id");
    eprintln!("request id rust={rust_hex} python={python_hex}");
    assert_eq!(rust_hex, python_hex);
}

// The interop lock is a tokio MutexGuard that has to outlive the whole test,
// which is exactly what await_holding_invalid_type forbids; the test fns only
// take that lock and everything else runs under the lint.
#[tokio::test]
#[ignore = "requires local Python Reticulum checkout"]
async fn python_to_rust_request_handler_allow_all_answers_a_packet_request() {
    let _interop_guard = python_interop_guard().await;
    run_python_to_rust_request_handler_allow_all_answers_a_packet_request().await;
}

#[deny(clippy::await_holding_invalid_type)]
async fn run_python_to_rust_request_handler_allow_all_answers_a_packet_request() {
    let fixture = RequestHandlerFixture::start("python-rns-request-handler-packet").await;
    let record = RequestHandlerRecord::default();
    fixture.register(record.handler(request_handler_echo), RequestAllow::All).await;

    let mut client = fixture.connect(8.0, &[]).await;
    let lines = client.json_lines().await;

    let (request_id, data) = record.single_request();
    assert_eq!(rmpv_to_string(&data).as_deref(), Some("hello-rust"));
    request_handler_assert_response(&lines, request_id, b"reply:hello-rust");
}

#[tokio::test]
#[ignore = "requires local Python Reticulum checkout"]
async fn python_to_rust_request_handler_answers_a_resource_request_with_a_resource_response() {
    let _interop_guard = python_interop_guard().await;
    run_python_to_rust_request_handler_answers_a_resource_request_with_a_resource_response().await;
}

#[deny(clippy::await_holding_invalid_type)]
async fn run_python_to_rust_request_handler_answers_a_resource_request_with_a_resource_response() {
    let fixture = RequestHandlerFixture::start("python-rns-request-handler-resource").await;
    let record = RequestHandlerRecord::default();
    fixture.register(record.handler(request_handler_echo), RequestAllow::All).await;

    let mut client = fixture.connect(20.0, &["--request-size", "4096"]).await;
    let lines = client.json_lines().await;

    let request_text = "x".repeat(4096);
    let (request_id, data) = record.single_request();
    assert_eq!(rmpv_to_string(&data).as_deref(), Some(request_text.as_str()));
    request_handler_assert_response(&lines, request_id, format!("reply:{request_text}").as_bytes());
}

#[tokio::test]
#[ignore = "requires local Python Reticulum checkout"]
async fn python_to_rust_request_handler_answers_a_packet_request_with_a_resource_response() {
    let _interop_guard = python_interop_guard().await;
    run_python_to_rust_request_handler_answers_a_packet_request_with_a_resource_response().await;
}

#[deny(clippy::await_holding_invalid_type)]
async fn run_python_to_rust_request_handler_answers_a_packet_request_with_a_resource_response() {
    let fixture = RequestHandlerFixture::start("python-rns-request-handler-bytes").await;
    let record = RequestHandlerRecord::default();
    fixture.register(record.handler(request_handler_bytes), RequestAllow::All).await;

    let mut client = fixture.connect(20.0, &[]).await;
    let lines = client.json_lines().await;

    let (request_id, data) = record.single_request();
    assert_eq!(rmpv_to_string(&data).as_deref(), Some("hello-rust"));
    request_handler_assert_response(&lines, request_id, &request_handler_bytes_body());
}

#[tokio::test]
#[ignore = "requires local Python Reticulum checkout"]
async fn python_to_rust_request_handler_allow_list_answers_the_identified_peer() {
    let _interop_guard = python_interop_guard().await;
    run_python_to_rust_request_handler_allow_list_answers_the_identified_peer().await;
}

#[deny(clippy::await_holding_invalid_type)]
async fn run_python_to_rust_request_handler_allow_list_answers_the_identified_peer() {
    let fixture = RequestHandlerFixture::start("python-rns-request-handler-allow-list").await;
    let record = RequestHandlerRecord::default();

    let mut client = fixture.connect(12.0, &["--identify"]).await;
    let identity =
        wait_for_link_identify(&mut client.in_events, client.link_id, Duration::from_secs(8)).await;
    fixture
        .register(
            record.handler(request_handler_echo),
            RequestAllow::List(HashSet::from([identity.address_hash])),
        )
        .await;
    fixture.release_client().await;
    let lines = client.json_lines().await;

    let identified = request_handler_line_with(&lines, "identified");
    assert_eq!(identified["identified"], hex::encode(identity.address_hash.as_slice()));
    let (request_id, data) = record.single_request();
    assert_eq!(rmpv_to_string(&data).as_deref(), Some("hello-rust"));
    request_handler_assert_response(&lines, request_id, b"reply:hello-rust");
}

#[tokio::test]
#[ignore = "requires local Python Reticulum checkout"]
async fn python_to_rust_request_handler_allow_list_denies_an_unlisted_peer_silently() {
    let _interop_guard = python_interop_guard().await;
    run_python_to_rust_request_handler_allow_list_denies_an_unlisted_peer_silently().await;
}

#[deny(clippy::await_holding_invalid_type)]
async fn run_python_to_rust_request_handler_allow_list_denies_an_unlisted_peer_silently() {
    let fixture = RequestHandlerFixture::start("python-rns-request-handler-deny-list").await;
    let record = RequestHandlerRecord::default();
    let other = to_transport_private_identity(&PrivateIdentity::new_from_rand(OsRng));

    let mut client = fixture.connect(6.0, &["--identify"]).await;
    let identity =
        wait_for_link_identify(&mut client.in_events, client.link_id, Duration::from_secs(8)).await;
    assert_ne!(identity.address_hash, other.as_identity().address_hash);
    fixture
        .register(
            record.handler(request_handler_echo),
            RequestAllow::List(HashSet::from([other.as_identity().address_hash])),
        )
        .await;
    fixture.release_client().await;
    let lines = client.json_lines().await;

    request_handler_line_with(&lines, "request_sent");
    let report = request_handler_line_with(&lines, "request_no_response");
    assert_eq!(report["request_no_response"], true);
    assert_eq!(record.calls.load(Ordering::SeqCst), 0, "handler ran for an unlisted peer");
}
