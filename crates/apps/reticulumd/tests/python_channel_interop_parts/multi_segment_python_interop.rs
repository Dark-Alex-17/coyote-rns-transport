// A 4 MiB + 1 byte body (plus the msgpack response envelope) spans five
// MAX_EFFICIENT_SIZE (1 MiB - 1) segments.
const MULTI_SEGMENT_RESPONSE_LEN: usize = 4 * 1024 * 1024 + 1;
const MULTI_SEGMENT_RESPONSE_SEGMENTS: u32 = 5;

// Hash blocks keep the body incompressible, so the segment count on the wire
// matches the plaintext length instead of whatever bz2 leaves
// (RNS/Resource.py:395; resource/sender.rs:122).
fn multi_segment_response_body() -> Vec<u8> {
    use sha2::Digest as _;
    let mut body = Vec::with_capacity(MULTI_SEGMENT_RESPONSE_LEN + 32);
    let mut index: u64 = 0;
    while body.len() < MULTI_SEGMENT_RESPONSE_LEN {
        let mut hasher = sha2::Sha256::new();
        hasher.update(b"coyote-rns-transport multi-segment ");
        hasher.update(index.to_be_bytes());
        body.extend_from_slice(&hasher.finalize());
        index += 1;
    }
    body.truncate(MULTI_SEGMENT_RESPONSE_LEN);
    body
}

fn multi_segment_digest_hex(data: &[u8]) -> String {
    use sha2::Digest as _;
    hex::encode(sha2::Sha256::digest(data))
}

// Like `read_ready`, but hands the stdout reader back so the server's later
// JSON lines stay readable.
fn read_ready_keeping_stdout(
    child: &mut std::process::Child,
) -> (ReadyLine, std::io::BufReader<std::process::ChildStdout>) {
    use std::io::BufRead as _;
    let mut reader = std::io::BufReader::new(child.stdout.take().expect("python stdout"));
    let mut line = String::new();
    loop {
        line.clear();
        let read = reader.read_line(&mut line).expect("read ready line");
        assert!(read > 0, "python endpoint exited before reporting ready");
        if let Ok(ready) = serde_json::from_str::<ReadyLine>(&line) {
            if ready.ready {
                return (ready, reader);
            }
        }
    }
}

// Drains the server's stdout from the moment the ready line is read, so RNS
// log output cannot fill the pipe and stall the Python side mid-transfer;
// the served-response report is handed over as soon as it appears and the
// drain keeps running until the endpoint exits.
fn spawn_served_response_reader(
    reader: std::io::BufReader<std::process::ChildStdout>,
) -> tokio::sync::oneshot::Receiver<serde_json::Value> {
    let (report_tx, report_rx) = tokio::sync::oneshot::channel();
    tokio::task::spawn_blocking(move || {
        use std::io::BufRead as _;
        let mut report_tx = Some(report_tx);
        for line in reader.lines() {
            let Ok(line) = line else {
                break;
            };
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) {
                if value.get("served_response_sha256").is_some() {
                    if let Some(tx) = report_tx.take() {
                        let _ = tx.send(value);
                    }
                }
            }
        }
    });
    report_rx
}

#[deny(clippy::await_holding_invalid_type)]
async fn await_served_response_report(
    report: tokio::sync::oneshot::Receiver<serde_json::Value>,
) -> serde_json::Value {
    tokio::time::timeout(Duration::from_secs(10), report)
        .await
        .expect("timed out waiting for the Python served-response report")
        .expect("python endpoint exited without reporting the served response")
}

// Like `wait_for_resource_response`, but also reports the highest
// `total_segments` seen in `SegmentComplete` events for the link, so the test
// can pin that the response really crossed as a split Resource.
#[deny(clippy::await_holding_invalid_type)]
async fn wait_for_split_resource_response(
    events: &mut tokio::sync::broadcast::Receiver<rns_transport::resource::ResourceEvent>,
    link_id: AddressHash,
    request_id: [u8; 16],
    duration: Duration,
) -> (rmpv::Value, Option<u32>) {
    use rns_transport::resource::ResourceEventKind;
    tokio::time::timeout(duration, async {
        let mut segments_seen = None;
        loop {
            let event = match events.recv().await {
                Ok(event) => event,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    panic!("resource event channel closed")
                }
            };
            if event.link_id != link_id {
                continue;
            }
            match event.kind {
                ResourceEventKind::SegmentComplete(segment) => {
                    segments_seen = segments_seen.max(Some(segment.total_segments));
                }
                ResourceEventKind::Complete(complete)
                    if complete.is_response
                        && complete.request_id.as_deref() == Some(request_id.as_slice()) =>
                {
                    let frame = rmp_serde::from_slice::<rmpv::Value>(&complete.data)
                        .expect("response envelope decodes as msgpack");
                    let rmpv::Value::Array(mut entries) = frame else {
                        panic!("response envelope should be a msgpack array");
                    };
                    assert_eq!(entries.len(), 2, "response envelope is [request_id, response]");
                    assert_eq!(
                        entries[0].as_slice(),
                        Some(request_id.as_slice()),
                        "response envelope carries the request id"
                    );
                    return (entries.remove(1), segments_seen);
                }
                _ => {}
            }
        }
    })
    .await
    .expect("timed out waiting for the split resource response")
}

#[deny(clippy::await_holding_invalid_type)]
async fn wait_for_link_request(
    events: &mut tokio::sync::broadcast::Receiver<
        rns_transport::destination::link::LinkEventData,
    >,
    link_id: AddressHash,
    duration: Duration,
) -> [u8; 16] {
    tokio::time::timeout(duration, async {
        loop {
            match events.recv().await {
                Ok(event) => {
                    if event.id != link_id {
                        continue;
                    }
                    let rns_transport::destination::link::LinkEvent::Data(payload) = event.event
                    else {
                        continue;
                    };
                    if payload.context() != PacketContext::Request {
                        continue;
                    }
                    if let Some(request_id) = payload.request_id() {
                        return request_id;
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    panic!("inbound link event channel closed")
                }
            }
        }
    })
    .await
    .expect("timed out waiting for the Python request")
}

// The interop lock is a tokio MutexGuard that has to outlive the whole test,
// which is exactly what await_holding_invalid_type forbids; the test fns only
// take that lock and everything else runs under the lint.
#[tokio::test]
#[ignore = "requires local Python Reticulum checkout"]
async fn rust_to_python_multi_segment_response_roundtrip() {
    let _interop_guard = python_interop_guard().await;
    run_rust_to_python_multi_segment_response_roundtrip().await;
}

#[deny(clippy::await_holding_invalid_type)]
async fn run_rust_to_python_multi_segment_response_roundtrip() {
    let paths = python_channel_interop_paths();

    let server_port = free_tcp_port();
    let temp = tempfile::tempdir().expect("tempdir");
    let py_config_dir = temp.path().join("python-rns-multi-segment");
    fs::create_dir_all(&py_config_dir).expect("python config dir");
    write_python_config(&py_config_dir, server_port);

    let mut child = paths.spawn_endpoint(&py_config_dir, "multi-segment-response");
    let (ready, server_stdout) = read_ready_keeping_stdout(&mut child);
    let served_report = spawn_served_response_reader(server_stdout);
    let _guard = ChildGuard { child: Some(child) };
    wait_for_port(server_port, Duration::from_secs(5)).await;

    let target_hash =
        AddressHash::new_from_hex_string(&ready.destination_hash).expect("destination hash");
    let rust_identity = PrivateIdentity::new_from_rand(OsRng);
    let rust_identity = to_transport_private_identity(&rust_identity);
    // Without broadcast, a packet that resolves no path-table route is dropped
    // instead of flooded, so every Resource segment has to leave on the
    // link's own interface for the transfer to complete.
    let mut config = TransportConfig::new("python-multi-segment-interop", &rust_identity, false);
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
    let link = transport.link(destination).await;
    let link_id = wait_for_out_link_active(&mut link_events, &link, Duration::from_secs(8)).await;
    sleep(Duration::from_millis(100)).await;

    let request = rns_transport::destination::link::Link::request_payload(
        "/test/request",
        rmpv::Value::Nil,
    )
    .expect("request payload");
    let mut resource_events = transport.resource_events();
    let request_hash = transport
        .send_request_resource(
            &link_id,
            request.resource_request_id.to_vec(),
            request.packed,
            None,
        )
        .await
        .expect("send multi-segment request resource");
    wait_for_outbound_resource_complete(&mut resource_events, request_hash, Duration::from_secs(8))
        .await;

    let (response, segments_seen) = wait_for_split_resource_response(
        &mut resource_events,
        link_id,
        request.resource_request_id,
        Duration::from_secs(180),
    )
    .await;
    assert_eq!(
        segments_seen,
        Some(MULTI_SEGMENT_RESPONSE_SEGMENTS),
        "the response must arrive as a five-segment split Resource"
    );
    let rmpv::Value::Binary(body) = response else {
        panic!("multi-segment response should arrive as a msgpack binary value");
    };
    assert_eq!(body.len(), MULTI_SEGMENT_RESPONSE_LEN);
    let received_digest = multi_segment_digest_hex(&body);
    assert_eq!(
        received_digest,
        multi_segment_digest_hex(&multi_segment_response_body()),
        "received body must match the deterministic fixture"
    );

    let served = await_served_response_report(served_report).await;
    assert_eq!(
        served["served_response_size"].as_u64(),
        Some(MULTI_SEGMENT_RESPONSE_LEN as u64)
    );
    assert_eq!(
        served["served_response_sha256"].as_str(),
        Some(received_digest.as_str()),
        "Python must report the digest Rust received"
    );
}

#[tokio::test]
#[ignore = "requires local Python Reticulum checkout"]
async fn python_to_rust_multi_segment_response_roundtrip() {
    let _interop_guard = python_interop_guard().await;
    run_python_to_rust_multi_segment_response_roundtrip().await;
}

#[deny(clippy::await_holding_invalid_type)]
async fn run_python_to_rust_multi_segment_response_roundtrip() {
    let paths = python_channel_interop_paths();

    let server_port = free_tcp_port();
    let temp = tempfile::tempdir().expect("tempdir");
    let py_config_dir = temp.path().join("python-rns-multi-segment-client");
    fs::create_dir_all(&py_config_dir).expect("python config dir");
    write_python_client_config(&py_config_dir, server_port);

    let rust_identity = PrivateIdentity::new_from_rand(OsRng);
    let rust_identity = to_transport_private_identity(&rust_identity);
    // See rust_to_python_multi_segment_response_roundtrip for why broadcast
    // is off: the later segments and retries must follow the link, not flood.
    let mut config =
        TransportConfig::new("python-multi-segment-interop-rust-server", &rust_identity, false);
    config.set_path_request_timeout_secs(2);
    config.set_resource_retry_interval_secs(1);
    let transport = Transport::new(config);
    let iface_manager = transport.iface_manager();
    transport
        .iface_manager()
        .lock()
        .await
        .spawn(TcpServer::new(format!("127.0.0.1:{server_port}"), iface_manager), TcpServer::spawn);
    wait_for_port(server_port, Duration::from_secs(5)).await;

    let destination = transport
        .add_destination(rust_identity.clone(), DestinationName::new("test", "channel"))
        .await;
    let destination_hash = {
        let destination = destination.lock().await;
        hex::encode(destination.desc.address_hash.as_slice())
    };

    let child = paths.spawn_multi_segment_client(&py_config_dir, &destination_hash);
    let mut guard = ChildGuard { child: Some(child) };

    let mut in_events = transport.in_link_events();
    let link_id = wait_for_in_link_active_with_announces(
        &transport,
        &destination,
        &mut in_events,
        Duration::from_secs(8),
    )
    .await;
    let request_id = wait_for_link_request(&mut in_events, link_id, Duration::from_secs(30)).await;

    let body = multi_segment_response_body();
    let expected_digest = multi_segment_digest_hex(&body);
    let response_payload = rmp_serde::to_vec(&rmpv::Value::Array(vec![
        rmpv::Value::Binary(request_id.to_vec()),
        rmpv::Value::Binary(body),
    ]))
    .expect("multi-segment response payload");
    let mut resource_events = transport.resource_events();
    let response_hash = transport
        .send_response_resource(&link_id, request_id.to_vec(), response_payload, None)
        .await
        .expect("send multi-segment response resource");
    wait_for_outbound_resource_complete(
        &mut resource_events,
        response_hash,
        Duration::from_secs(180),
    )
    .await;

    let child = guard.child.take().expect("python child");
    let output = tokio::task::spawn_blocking(move || child.wait_with_output())
        .await
        .expect("join python client")
        .expect("wait for python client");
    if !output.status.success() {
        panic!(
            "python multi-segment client failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let report: serde_json::Value = stdout
        .lines()
        .find_map(|line| serde_json::from_str(line).ok())
        .unwrap_or_else(|| panic!("Python response report missing from stdout: {stdout}"));
    assert_eq!(
        report["response_size"].as_u64(),
        Some(MULTI_SEGMENT_RESPONSE_LEN as u64)
    );
    assert_eq!(
        report["response_sha256"].as_str(),
        Some(expected_digest.as_str()),
        "Python must report the exact response digest"
    );
}
