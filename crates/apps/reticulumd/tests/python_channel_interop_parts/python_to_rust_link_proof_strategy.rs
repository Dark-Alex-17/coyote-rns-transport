// A Python RNS 1.5.2 initiator sends one plain link packet and watches its
// `PacketReceipt`; the Rust destination holds `ProofStrategy::App` with no
// callback, so the link does not prove on receipt and the receipt stays
// `SENT` until the application proves the packet through
// `Transport::prove_link_packet` with the hash from `LinkPayload::packet_hash`,
// the way a Python application calls `packet.prove()` (`RNS/Packet.py:336-341`).
//
// Reuses the client fixture shape of `python_to_rust_link_data_roundtrip`.

use rns_transport::destination::link::LinkPayload;
use rns_transport::destination::ProofStrategy;

/// How long the Rust side sits on the packet before proving it; long enough
/// that Python's default link receipt timeout, `max(rtt * 6, 5 ms)`, would
/// have failed the receipt had the client not raised it.
const DEFERRED_PROOF_DELAY: Duration = Duration::from_millis(1500);

/// One `{"receipt": ..., "elapsed": ...}` line the Python client printed.
#[derive(Debug)]
struct ReceiptLine {
    status: String,
    elapsed: f64,
}

fn receipt_lines(stdout: &str) -> Vec<ReceiptLine> {
    stdout
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter_map(|value| {
            Some(ReceiptLine {
                status: value.get("receipt")?.as_str()?.to_string(),
                elapsed: value.get("elapsed")?.as_f64()?,
            })
        })
        .collect()
}

async fn wait_for_in_link_payload(
    events: &mut tokio::sync::broadcast::Receiver<LinkEventData>,
    link_id: AddressHash,
    expected: &[u8],
    duration: Duration,
) -> LinkPayload {
    tokio::time::timeout(duration, async {
        loop {
            let event = events.recv().await.expect("in-link event");
            if event.id != link_id {
                continue;
            }
            if let LinkEvent::Data(payload) = event.event {
                if payload.as_slice() == expected {
                    return *payload;
                }
            }
        }
    })
    .await
    .expect("timed out waiting for the link payload")
}

#[tokio::test]
#[ignore = "requires local Python Reticulum checkout"]
async fn python_to_rust_link_proof_strategy_app_defers_the_receipt_until_the_app_proves() {
    let _interop_guard = python_interop_guard().await;
    run_python_to_rust_link_proof_strategy_app_defers_the_receipt_until_the_app_proves().await;
}

#[deny(clippy::await_holding_invalid_type)]
async fn run_python_to_rust_link_proof_strategy_app_defers_the_receipt_until_the_app_proves() {
    let paths = python_channel_interop_paths();

    let server_port = free_tcp_port();
    let temp = tempfile::tempdir().expect("tempdir");
    let py_config_dir = temp.path().join("python-rns-link-proof-strategy-client");
    fs::create_dir_all(&py_config_dir).expect("python config dir");
    write_python_client_config(&py_config_dir, server_port);

    let rust_identity = PrivateIdentity::new_from_rand(OsRng);
    let rust_identity = to_transport_private_identity(&rust_identity);
    let mut config =
        TransportConfig::new("python-link-proof-strategy-rust-server", &rust_identity, true);
    config.set_path_request_timeout_secs(2);
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
    destination.lock().await.set_link_proof_strategy(ProofStrategy::App);
    let destination_hash = {
        let destination = destination.lock().await;
        hex::encode(destination.desc.address_hash.as_slice())
    };

    let child = paths.spawn_channel_client_with_args(
        &py_config_dir,
        &destination_hash,
        "link-data-receipt",
        20.0,
        &[],
    );
    let mut guard = ChildGuard { child: Some(child) };

    let mut in_events = transport.in_link_events();
    let link_id = wait_for_in_link_active_with_announces(
        &transport,
        &destination,
        &mut in_events,
        Duration::from_secs(8),
    )
    .await;
    let payload =
        wait_for_in_link_payload(&mut in_events, link_id, b"hello-rust", Duration::from_secs(8))
            .await;
    let packet_hash = payload.packet_hash().expect("a received payload names its packet");

    sleep(DEFERRED_PROOF_DELAY).await;
    transport
        .prove_link_packet(&link_id, &packet_hash)
        .await
        .expect("the application proof goes out on the link");

    let child = guard.child.take().expect("python child");
    let output = tokio::task::spawn_blocking(move || child.wait_with_output())
        .await
        .expect("join python client")
        .expect("wait for python client");
    let stdout = String::from_utf8_lossy(&output.stdout);
    if !output.status.success() {
        panic!(
            "python link-data-receipt client failed\nstdout:\n{stdout}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let lines = receipt_lines(&stdout);
    assert!(
        lines.iter().any(|line| line.status == "SENT"),
        "the receipt started out SENT: {lines:?}"
    );
    let delivered = lines
        .iter()
        .find(|line| line.status == "DELIVERED")
        .unwrap_or_else(|| panic!("the receipt became DELIVERED: {lines:?}"));
    assert!(
        delivered.elapsed >= 1.0,
        "the receipt was delivered only after the deferred proof: {lines:?}"
    );
}
