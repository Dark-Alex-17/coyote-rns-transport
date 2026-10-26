// Usage-pattern coverage for the lifecycle surface from the consumer's side: a
// `try_send_announce` that succeeds is observed by the peer with its app data, `stop`
// returns promptly on a transport that never attached an interface and on one whose
// `TcpClient` is stuck reconnecting to a dead port, the public API keeps answering
// (without panicking or hanging) after `stop`, `stop` returns within its budget while
// the peer floods announces, and two concurrent `stop` calls both return. Helpers come
// from the sibling `tests_parts` files included into this module.

use crate::iface::tcp_client::TcpClient as ProbeTcpClient;

const PROBE_STOP_TIMEOUT: Duration = Duration::from_secs(5);

/// A successful `try_send_announce` is not just `Ok(())`: the peer learns the destination
/// and sees the same app data the caller passed in.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_try_send_announce_delivers_app_data_to_peer() {
    let listener = listener("probe-announce-listener", ProbeTcpClient::DEFAULT_MTU).await;
    let mut connector =
        connector("probe-announce-connector", listener.port, ProbeTcpClient::DEFAULT_MTU).await;
    listener.await_accepted_connections(1).await;

    let identity = PrivateIdentity::new_from_rand(OsRng);
    let destination = listener
        .transport
        .add_destination(identity, DestinationName::new("probe", "announce"))
        .await;
    let address = destination.lock().await.desc.address_hash;

    let sent = listener.transport.try_send_announce(&destination, Some(b"probe app data")).await;
    assert!(sent.is_ok(), "a buildable announce is sent: {sent:?}");

    let event = timeout(LOOPBACK_TIMEOUT, async {
        loop {
            match connector.announces.recv().await {
                Ok(event) => {
                    if event.destination.lock().await.desc.address_hash == address {
                        return event;
                    }
                }
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => panic!("announce channel closed"),
            }
        }
    })
    .await
    .expect("peer learns the announce sent with try_send_announce");
    assert_eq!(event.app_data.as_slice(), b"probe app data");

    timeout(PROBE_STOP_TIMEOUT, listener.transport.stop()).await.expect("listener stop");
    timeout(PROBE_STOP_TIMEOUT, connector.transport.stop()).await.expect("connector stop");
}

/// A transport that never attached an interface still stops promptly, twice.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn transport_stop_with_no_interfaces_returns_promptly_and_twice() {
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let transport = Transport::new(TransportConfig::new("probe-bare", &identity, false));

    timeout(PROBE_STOP_TIMEOUT, transport.stop()).await.expect("first stop completes");
    timeout(PROBE_STOP_TIMEOUT, transport.stop()).await.expect("second stop completes");
    assert!(transport.interface_hashes().await.is_empty());
}

/// The consumer pattern (`iface_manager().spawn(TcpClient::new(..), TcpClient::spawn)`)
/// with a peer that is not listening: the client sits in its reconnect loop. `stop` must
/// still return within the budget and leave the client worker closed, not reconnecting.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn transport_stop_releases_a_client_stuck_reconnecting_to_a_dead_port() {
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let transport = Transport::new(TransportConfig::new("probe-dead-port", &identity, false));
    let dead_port = free_loopback_port().await;
    let client = ProbeTcpClient::new(format!("127.0.0.1:{dead_port}"));
    let status = client.runtime_status_handle();
    transport.iface_manager().lock().await.spawn(client, ProbeTcpClient::spawn);

    wait_until(
        "client to attempt a connection to the dead port",
        || {
            let state = status.to_json();
            state["reconnect_attempts"].as_u64().unwrap_or(0) >= 1
                || state["stream_state"].as_str() == Some("reconnecting")
        },
        || status.to_json().to_string(),
    )
    .await;
    assert_eq!(transport.interface_hashes().await.len(), 1, "client is attached before stop");

    timeout(PROBE_STOP_TIMEOUT, transport.stop()).await.expect("stop completes with a dead peer");

    assert!(transport.interface_hashes().await.is_empty(), "stop detaches the client");
    let state = status.to_json();
    assert_eq!(
        state["stream_state"].as_str(),
        Some("closed"),
        "the client worker has exited instead of reconnecting: {state}"
    );
    timeout(PROBE_STOP_TIMEOUT, transport.stop()).await.expect("second stop is a no-op");
}

/// After `stop`, a consumer that still holds the `Transport` (and its peer) can keep
/// calling the public API: every call returns within the budget and nothing panics.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_api_keeps_answering_after_stop() {
    let listener = listener("probe-after-stop-listener", ProbeTcpClient::DEFAULT_MTU).await;
    let mut connector =
        connector("probe-after-stop-connector", listener.port, ProbeTcpClient::DEFAULT_MTU).await;
    listener.await_accepted_connections(1).await;
    let (destination, desc) =
        register_in_destination(&listener.transport, "probe", "after-stop").await;
    let learned = learn_announce(&mut connector.announces, &desc.address_hash).await;
    let (_link, _link_id) = open_link(&connector.transport, learned).await;

    timeout(PROBE_STOP_TIMEOUT, listener.transport.stop()).await.expect("listener stop");

    // Stopped side: announce, link fan-out and destination registration all return.
    let announce = timeout(
        PROBE_STOP_TIMEOUT,
        listener.transport.try_send_announce(&destination, Some(b"after stop")),
    )
    .await
    .expect("try_send_announce returns after stop");
    let _ = announce; // Ok or Err are both acceptable; hanging or panicking is not.
    timeout(PROBE_STOP_TIMEOUT, listener.transport.send_announce(&destination, None))
        .await
        .expect("send_announce returns after stop");
    let report = timeout(
        PROBE_STOP_TIMEOUT,
        listener.transport.send_to_in_links_with_report(&desc.address_hash, b"after stop"),
    )
    .await
    .expect("send_to_in_links_with_report returns after stop");
    assert_eq!(report.sent_links, 0, "nothing is sent after stop: {report:?}");
    assert_eq!(
        report.matched_links,
        report.sent_links + report.failed_links,
        "every matched link is accounted for as sent or failed: {report:?}"
    );
    let extra = PrivateIdentity::new_from_rand(OsRng);
    timeout(
        PROBE_STOP_TIMEOUT,
        listener.transport.add_destination(extra, DestinationName::new("probe", "late")),
    )
    .await
    .expect("add_destination returns after stop");

    // Peer side: its sends toward the stopped transport still return a report.
    let report = timeout(
        PROBE_STOP_TIMEOUT,
        connector.transport.send_to_out_links_with_report(&desc.address_hash, b"to stopped peer"),
    )
    .await
    .expect("peer send toward a stopped transport returns");
    assert!(report.matched_links <= 1, "report is well formed: {report:?}");

    timeout(PROBE_STOP_TIMEOUT, connector.transport.stop()).await.expect("connector stop");
}

/// Daemon-shutdown pattern: the peer keeps pushing announces into the stopping transport
/// the whole time. `stop` must not wait for the inbound traffic to drain or for the
/// interface worker to find a quiet moment; it returns within the budget and the
/// flooding peer's own sends keep returning afterwards.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_stop_returns_within_budget_under_inbound_flood() {
    let listener = listener("probe-flood-listener", ProbeTcpClient::DEFAULT_MTU).await;
    let connector =
        connector("probe-flood-connector", listener.port, ProbeTcpClient::DEFAULT_MTU).await;
    listener.await_accepted_connections(1).await;

    let identity = PrivateIdentity::new_from_rand(OsRng);
    let destination =
        connector.transport.add_destination(identity, DestinationName::new("probe", "flood")).await;
    let mut learned = listener.transport.recv_announces().await;
    connector.transport.send_announce(&destination, Some(b"flood warm-up")).await;
    timeout(LOOPBACK_TIMEOUT, async {
        loop {
            match learned.recv().await {
                Ok(_) => return,
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => panic!("announce channel closed before the flood"),
            }
        }
    })
    .await
    .expect("listener sees traffic from the flooding peer before stop");

    let stopped = std::sync::atomic::AtomicBool::new(false);
    let payload = vec![0x5a_u8; 200];
    let flood = async {
        let mut sent = 0_u64;
        while !stopped.load(std::sync::atomic::Ordering::Acquire) {
            connector.transport.send_announce(&destination, Some(&payload)).await;
            sent += 1;
            tokio::task::yield_now().await;
        }
        sent
    };
    let stop = async {
        let outcome = timeout(PROBE_STOP_TIMEOUT, listener.transport.stop()).await;
        stopped.store(true, std::sync::atomic::Ordering::Release);
        outcome
    };
    let (sent, outcome) = tokio::join!(flood, stop);
    outcome.expect("stop returns within the budget while the peer floods announces");
    assert!(sent >= 1, "the peer was actually sending during stop");

    assert!(listener.transport.interface_hashes().await.is_empty(), "stop detaches the server");
    let state = listener.status.to_json();
    assert_eq!(state["listener_state"].as_str(), Some("closed"), "server worker exited: {state}");

    // The flooding side still answers after its peer vanished mid-stream.
    timeout(PROBE_STOP_TIMEOUT, connector.transport.send_announce(&destination, Some(b"late")))
        .await
        .expect("send_announce toward a stopped peer returns");
    timeout(PROBE_STOP_TIMEOUT, connector.transport.stop()).await.expect("connector stop");
}

/// Two shutdown paths racing (signal handler plus supervisor, say) both call `stop` at
/// once. Both calls return within the budget, nothing panics, and the transport ends up
/// detached with its listening socket released.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_concurrent_stop_calls_both_return() {
    let listener = listener("probe-race-listener", ProbeTcpClient::DEFAULT_MTU).await;
    let connector =
        connector("probe-race-connector", listener.port, ProbeTcpClient::DEFAULT_MTU).await;
    listener.await_accepted_connections(1).await;
    let port = listener.port;

    let (first, second) = tokio::join!(
        timeout(PROBE_STOP_TIMEOUT, listener.transport.stop()),
        timeout(PROBE_STOP_TIMEOUT, listener.transport.stop()),
    );
    first.expect("first concurrent stop returns");
    second.expect("second concurrent stop returns");

    assert!(listener.transport.interface_hashes().await.is_empty(), "stop detaches the server");
    let rebound = tokio::net::TcpListener::bind(("127.0.0.1", port)).await;
    assert!(rebound.is_ok(), "the listening socket is released: {rebound:?}");
    drop(rebound);

    timeout(PROBE_STOP_TIMEOUT, listener.transport.stop())
        .await
        .expect("a third stop after the race is still a no-op");
    timeout(PROBE_STOP_TIMEOUT, connector.transport.stop()).await.expect("connector stop");
}
