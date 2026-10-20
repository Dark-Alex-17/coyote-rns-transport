use rns_transport::destination::link::Link;
use rns_transport::destination::request::{
    RequestError, RequestOptions, RequestReceipt, RequestResponseData, RequestStatus,
};
use rns_transport::hash::Hash;
use rns_transport::transport::SendPacketOutcome;

const REQUEST_RECEIPT_PATH: &str = "/test/request";

// Forwards every JSON line the Python server prints after its ready line and
// keeps draining until the endpoint exits, so RNS log output cannot fill the
// pipe and stall the Python side.
fn rust_request_receipt_spawn_json_reader(
    reader: std::io::BufReader<std::process::ChildStdout>,
) -> tokio::sync::mpsc::UnboundedReceiver<serde_json::Value> {
    let (line_tx, line_rx) = tokio::sync::mpsc::unbounded_channel();
    tokio::task::spawn_blocking(move || {
        use std::io::BufRead as _;
        for line in reader.lines() {
            let line = match line {
                Ok(line) => line,
                Err(err) if err.kind() == std::io::ErrorKind::InvalidData => continue,
                Err(_) => break,
            };
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) {
                let _ = line_tx.send(value);
            }
        }
    });
    line_rx
}

struct RustRequestReceiptFixture {
    transport: Transport,
    link: Arc<tokio::sync::Mutex<Link>>,
    link_id: AddressHash,
    python_lines: tokio::sync::mpsc::UnboundedReceiver<serde_json::Value>,
    _guard: ChildGuard,
    _temp: tempfile::TempDir,
}

impl RustRequestReceiptFixture {
    #[deny(clippy::await_holding_invalid_type)]
    async fn start(config_dir_name: &str, payload_kind: &str, extra_args: &[&str]) -> Self {
        let paths = python_channel_interop_paths();

        let server_port = free_tcp_port();
        let temp = tempfile::tempdir().expect("tempdir");
        let py_config_dir = temp.path().join(config_dir_name);
        fs::create_dir_all(&py_config_dir).expect("python config dir");
        write_python_config(&py_config_dir, server_port);

        let mut child = paths.spawn_endpoint_with_args(&py_config_dir, payload_kind, extra_args);
        let (ready, reader) = read_ready_keeping_stdout(&mut child);
        let guard = ChildGuard { child: Some(child) };
        let python_lines = rust_request_receipt_spawn_json_reader(reader);
        wait_for_port(server_port, Duration::from_secs(5)).await;

        let target_hash =
            AddressHash::new_from_hex_string(&ready.destination_hash).expect("destination hash");
        let rust_identity = PrivateIdentity::new_from_rand(OsRng);
        let rust_identity = to_transport_private_identity(&rust_identity);
        let mut config = TransportConfig::new(
            "python-request-receipt-interop-rust-client",
            &rust_identity,
            false,
        );
        config.set_path_request_timeout_secs(2);
        config.set_resource_retry_interval_secs(1);
        let transport = Transport::new(config);
        transport
            .iface_manager()
            .lock()
            .await
            .spawn(TcpClient::new(format!("127.0.0.1:{server_port}")), TcpClient::spawn);

        let destination = wait_for_announce(&transport, target_hash, Duration::from_secs(8)).await;
        let mut link_events = transport.out_link_events();
        let link = transport.link(destination).await.expect("link request sent");
        let link_id =
            wait_for_out_link_active(&mut link_events, &link, Duration::from_secs(8)).await;
        sleep(Duration::from_millis(100)).await;

        Self { transport, link, link_id, python_lines, _guard: guard, _temp: temp }
    }

    #[deny(clippy::await_holding_invalid_type)]
    async fn request(&self, data: rmpv::Value, opts: RequestOptions) -> RequestReceipt {
        self.transport
            .request(&self.link_id, REQUEST_RECEIPT_PATH, data, opts)
            .await
            .expect("request is sent")
    }

    #[deny(clippy::await_holding_invalid_type)]
    async fn python_report(&mut self, receipt: &RequestReceipt) -> serde_json::Value {
        let report = tokio::time::timeout(Duration::from_secs(20), async {
            loop {
                let value = self.python_lines.recv().await.expect("python endpoint stdout closed");
                if value.get("request_received").is_some() {
                    return value;
                }
            }
        })
        .await
        .expect("timed out waiting for the python request report");
        assert_eq!(report["path"], REQUEST_RECEIPT_PATH);

        let rust_hex = receipt.request_id().to_string();
        let python_hex = report["request_id"].as_str().expect("python request id");
        eprintln!("request id rust={rust_hex} python={python_hex}");
        assert_eq!(rust_hex, python_hex);
        report
    }

    #[deny(clippy::await_holding_invalid_type)]
    async fn send_plain_data(&self, data: &[u8]) -> Hash {
        let packet =
            { self.link.lock().await.data_packet(data).expect("plain data fits a packet") };
        let hash = packet.hash();
        let outcome = self.transport.send_link_packet_on_bound_iface(&self.link, packet).await;
        assert_eq!(outcome, SendPacketOutcome::SentDirect, "plain data reaches the wire");
        hash
    }
}

#[deny(clippy::await_holding_invalid_type)]
async fn rust_request_receipt_response(receipt: &RequestReceipt) -> RequestResponseData {
    tokio::time::timeout(Duration::from_secs(20), receipt.response())
        .await
        .expect("receipt concludes")
        .expect("response")
}

fn rust_request_receipt_is_proof_of(
    event: &LinkEventData,
    link_id: AddressHash,
    hash: Hash,
) -> bool {
    event.id == link_id
        && matches!(event.event, LinkEvent::PacketProven { packet_hash } if packet_hash == hash)
}

// The interop lock is a tokio MutexGuard that has to outlive the whole test,
// which is exactly what await_holding_invalid_type forbids; the test fns only
// take that lock and everything else runs under the lint.
#[tokio::test]
#[ignore = "requires local Python Reticulum checkout"]
async fn rust_to_python_request_receipt_packet_roundtrip_matches_request_id() {
    let _interop_guard = python_interop_guard().await;
    run_rust_to_python_request_receipt_packet_roundtrip_matches_request_id().await;
}

#[deny(clippy::await_holding_invalid_type)]
async fn run_rust_to_python_request_receipt_packet_roundtrip_matches_request_id() {
    let mut fixture =
        RustRequestReceiptFixture::start("python-rns-request-receipt-packet", "request-forms", &[])
            .await;

    let receipt = fixture
        .request(rmpv::Value::String("hello-python".into()), RequestOptions::default())
        .await;
    let response = rust_request_receipt_response(&receipt).await;
    assert_eq!(response.value().and_then(|v| v.as_str()), Some("reply:hello-python"));
    assert_eq!(receipt.status(), RequestStatus::Ready);

    let report = fixture.python_report(&receipt).await;
    assert_eq!(report["response_kind"], "text");
    assert_eq!(report["response_size"], 18);
}

#[tokio::test]
#[ignore = "requires local Python Reticulum checkout"]
async fn rust_to_python_request_receipt_none_response_fails_with_timeout() {
    let _interop_guard = python_interop_guard().await;
    run_rust_to_python_request_receipt_none_response_fails_with_timeout().await;
}

#[deny(clippy::await_holding_invalid_type)]
async fn run_rust_to_python_request_receipt_none_response_fails_with_timeout() {
    let mut fixture =
        RustRequestReceiptFixture::start("python-rns-request-receipt-none", "request-forms", &[])
            .await;

    let data = rmpv::Value::Map(vec![(
        rmpv::Value::String("response".into()),
        rmpv::Value::String("none".into()),
    )]);
    let receipt =
        fixture.request(data, RequestOptions::default().with_timeout(Duration::from_secs(3))).await;

    let report = fixture.python_report(&receipt).await;
    assert_eq!(report["response_kind"], "none");

    let outcome = tokio::time::timeout(Duration::from_secs(20), receipt.response())
        .await
        .expect("receipt concludes");
    assert_eq!(outcome, Err(RequestError::Timeout));
    assert_eq!(receipt.status(), RequestStatus::Failed);
}

#[tokio::test]
#[ignore = "requires local Python Reticulum checkout"]
async fn rust_to_python_plain_data_packet_is_proven_by_a_prove_all_destination() {
    let _interop_guard = python_interop_guard().await;
    run_rust_to_python_plain_data_packet_is_proven_by_a_prove_all_destination().await;
}

#[deny(clippy::await_holding_invalid_type)]
async fn run_rust_to_python_plain_data_packet_is_proven_by_a_prove_all_destination() {
    let fixture = RustRequestReceiptFixture::start(
        "python-rns-request-receipt-prove-all",
        "link-data",
        &["--prove-all"],
    )
    .await;

    let mut link_events = fixture.transport.out_link_events();
    let hash = fixture.send_plain_data(b"prove me").await;
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let event = link_events.recv().await.expect("link event");
            if rust_request_receipt_is_proof_of(&event, fixture.link_id, hash) {
                return;
            }
        }
    })
    .await
    .expect("timed out waiting for the python proof");
}

#[tokio::test]
#[ignore = "requires local Python Reticulum checkout"]
async fn rust_to_python_plain_data_packet_is_not_proven_by_a_default_destination() {
    let _interop_guard = python_interop_guard().await;
    run_rust_to_python_plain_data_packet_is_not_proven_by_a_default_destination().await;
}

#[deny(clippy::await_holding_invalid_type)]
async fn run_rust_to_python_plain_data_packet_is_not_proven_by_a_default_destination() {
    let fixture =
        RustRequestReceiptFixture::start("python-rns-request-receipt-prove-none", "link-data", &[])
            .await;

    let mut link_events = fixture.transport.out_link_events();
    let hash = fixture.send_plain_data(b"prove me").await;
    while let Ok(event) = tokio::time::timeout(Duration::from_secs(3), link_events.recv()).await {
        let event = event.expect("link event");
        assert!(
            !rust_request_receipt_is_proof_of(&event, fixture.link_id, hash),
            "a PROVE_NONE destination proved a plain data packet"
        );
    }
}
