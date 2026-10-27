// Coverage for the shared-instance client's state stream: an attach sends `Connecting`
// then `Connected`, a dropped connection sends `Disconnected` then `Connecting` once the
// flush has run and `Connected` when the stream is back, a detach ends the stream, a client
// attached while nothing listens stays in `Connecting` until the socket exists, and a full
// channel drops the event at debug without stalling the flush or the re-announce. The
// consumer driven by hand pins the order the events take within one cycle. Helpers come
// from the sibling `tests_parts` files included into this module.

/// How long a client attached to a closed port is watched for an event that must not come.
const SHARED_INSTANCE_STATE_QUIET: Duration = Duration::from_millis(500);

/// Reads the next state event within `LOOPBACK_TIMEOUT`; `None` is the stream's end.
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_state_next(
    events: &mut tokio::sync::mpsc::Receiver<SharedInstanceEvent>,
) -> Option<SharedInstanceEvent> {
    timeout(LOOPBACK_TIMEOUT, events.recv()).await.expect("timed out waiting for a state event")
}

/// Attaches `name` over `endpoint` with the short reconnect wait and a state stream of
/// `capacity`; returns the client and the stream's receiver.
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_state_client(
    name: &str,
    endpoint: SharedInstanceEndpoint,
    capacity: usize,
) -> (SharedInstanceClient, tokio::sync::mpsc::Receiver<SharedInstanceEvent>) {
    let (events_tx, events) = tokio::sync::mpsc::channel(capacity);
    let client = shared_instance_client_with_options(
        name,
        endpoint,
        SharedInstanceOptions::default()
            .with_reconnect_wait(SHARED_INSTANCE_RECONNECT_WAIT)
            .with_state_events(events_tx),
    )
    .await;
    (client, events)
}

/// The TCP shared instance goes away and comes back on the same port: the stream reads
/// `Connecting`, `Connected`, then `Disconnected`, `Connecting` with the connected posture
/// and the slot untouched, then `Connected` on the restart, and ends on the detach with
/// nothing else in between.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_state_events_follow_a_tcp_restart() {
    let server = shared_instance_server().await;
    let (client, mut events) =
        shared_instance_state_client("shared-instance-state-tcp", server.endpoint(), 8).await;
    server.await_accepted_connections(1).await;
    shared_instance_assert_attached(&client.transport, client.iface).await;
    assert_eq!(
        shared_instance_state_next(&mut events).await,
        Some(SharedInstanceEvent::Connecting)
    );
    assert_eq!(shared_instance_state_next(&mut events).await, Some(SharedInstanceEvent::Connected));

    let port = server.port;
    timeout(STOP_TIMEOUT, server.transport.stop()).await.expect("shared instance stop completes");
    assert_eq!(
        shared_instance_state_next(&mut events).await,
        Some(SharedInstanceEvent::Disconnected)
    );
    assert_eq!(
        shared_instance_state_next(&mut events).await,
        Some(SharedInstanceEvent::Connecting)
    );
    assert!(
        client.transport.connected_to_shared_instance().await,
        "the connected posture survives the drop"
    );
    assert_eq!(client.transport.shared_instance(), Some(client.iface), "the slot keeps the iface");

    let restarted = shared_instance_server_on(port).await;
    restarted.await_accepted_connections(1).await;
    assert_eq!(shared_instance_state_next(&mut events).await, Some(SharedInstanceEvent::Connected));

    assert!(client.transport.detach_shared_instance().await, "the attach is in place");
    assert_eq!(shared_instance_state_next(&mut events).await, None, "the detach ends the stream");

    timeout(STOP_TIMEOUT, client.transport.stop()).await.expect("client stop completes");
    timeout(STOP_TIMEOUT, restarted.transport.stop())
        .await
        .expect("restarted shared instance stop completes");
}

/// Drives a hand-built consumer named `name` over a queued connect and the reconnect
/// behind it, with a disconnect queued ahead of both when `with_disconnect`, and returns
/// what the state stream read until it ended; the consumer's return drops the sender.
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_state_drive_consumer(
    name: &str,
    with_disconnect: bool,
) -> Vec<SharedInstanceEvent> {
    let iface = AddressHash::new_from_rand(OsRng);
    let (reconnects_tx, reconnects) = tokio::sync::mpsc::channel(1);
    let (connects_tx, connects) = tokio::sync::mpsc::channel(1);
    let (disconnects_tx, disconnects) = tokio::sync::mpsc::channel(1);
    let (state_tx, mut state) = tokio::sync::mpsc::channel(8);
    if with_disconnect {
        disconnects_tx.send(iface).await.expect("queue the disconnect");
    }
    connects_tx.send(iface).await.expect("queue the connect");
    reconnects_tx.send(iface).await.expect("queue the reconnect");
    drop(reconnects_tx);

    let (_transport, consumer) = shared_instance_reconnect_consumer(name, iface);
    let cancel = consumer.cancel.clone();
    timeout(STOP_TIMEOUT, consumer.run(reconnects, connects, disconnects, Some(state_tx)))
        .await
        .expect("the consumer exits once the reconnect channel is drained and closed");
    cancel.cancel();
    drop(connects_tx);
    drop(disconnects_tx);

    timeout(STOP_TIMEOUT, async {
        let mut read = Vec::new();
        while let Some(event) = state.recv().await {
            read.push(event);
        }
        read
    })
    .await
    .expect("the consumer's return ends the stream")
}

/// A connect and the reconnect behind it: the stream reads `Connected` alone and the
/// reconnect is still served, so the stream rides beside the re-synthesis rather than in
/// place of it.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_state_consumer_reports_connected_and_serves_the_reconnect() {
    install_test_logger();
    let name = "shared-instance-state-consumer-connect";
    let read = shared_instance_state_drive_consumer(name, false).await;
    assert_eq!(read, [SharedInstanceEvent::Connected]);
    let records: Vec<String> =
        shared_instance_records_for(name).into_iter().map(|(_, record)| record).collect();
    assert!(records.iter().any(|record| record.contains("reconnected")), "{records:?}");
}

/// A disconnect, a connect and a reconnect all waiting: the stream reads `Disconnected`,
/// `Connecting`, `Connected` in that order, the disconnect's pair before the connect's
/// event. The consumer is driven unattached, so the pair is sent even though the flush is
/// skipped and its skipped-flush record is left behind; the biased select itself is pinned
/// by `shared_instance_reconnect_consumer_serves_the_disconnect_before_the_reconnect`,
/// this test pins the arm order.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_state_consumer_orders_a_whole_cycle() {
    install_test_logger();
    let name = "shared-instance-state-consumer-cycle";
    let read = shared_instance_state_drive_consumer(name, true).await;
    assert_eq!(
        read,
        [
            SharedInstanceEvent::Disconnected,
            SharedInstanceEvent::Connecting,
            SharedInstanceEvent::Connected
        ]
    );
    let records: Vec<String> =
        shared_instance_records_for(name).into_iter().map(|(_, record)| record).collect();
    assert!(
        records.iter().any(|record| record.contains("stream ended after detach, tables kept")),
        "the unattached disconnect skips the flush: {records:?}"
    );
}

/// A client attached to a port nothing listens on reads `Connecting` and nothing more
/// until a shared instance binds the port, then `Connected`.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_state_events_wait_in_connecting_while_the_listener_is_down() {
    let port = free_loopback_port().await;
    let (client, mut events) = shared_instance_state_client(
        "shared-instance-state-down",
        SharedInstanceEndpoint::Tcp(SocketAddr::from(([127, 0, 0, 1], port))),
        8,
    )
    .await;
    assert_eq!(
        shared_instance_state_next(&mut events).await,
        Some(SharedInstanceEvent::Connecting)
    );
    assert!(
        timeout(SHARED_INSTANCE_STATE_QUIET, events.recv()).await.is_err(),
        "no event while nothing listens"
    );
    assert!(
        client.transport.connected_to_shared_instance().await,
        "the connected posture is set by the attach"
    );
    assert_eq!(client.transport.shared_instance(), Some(client.iface), "the slot keeps the iface");

    let server = shared_instance_server_on(port).await;
    server.await_accepted_connections(1).await;
    assert_eq!(shared_instance_state_next(&mut events).await, Some(SharedInstanceEvent::Connected));

    timeout(STOP_TIMEOUT, client.transport.stop()).await.expect("client stop completes");
    timeout(STOP_TIMEOUT, server.transport.stop()).await.expect("shared instance stop completes");
}

/// The unix twin of the TCP restart, read on a raw `UnixListener`: the accepted stream and
/// the listener are dropped together, and a fresh listener is bound at the same path.
#[cfg(unix)]
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_state_events_follow_a_unix_restart() {
    let temp = tempfile::tempdir().expect("tempdir");
    let socket = temp.path().join("state.sock");
    let listener = tokio::net::UnixListener::bind(&socket).expect("bind the socket");
    let (client, mut events) = shared_instance_state_client(
        "shared-instance-state-unix",
        SharedInstanceEndpoint::Unix(LocalUnixEndpoint::filesystem(socket.clone())),
        8,
    )
    .await;
    let (stream, _) = timeout(LOOPBACK_TIMEOUT, listener.accept())
        .await
        .expect("timed out waiting for the unix client to connect")
        .expect("accept");
    shared_instance_assert_attached(&client.transport, client.iface).await;
    assert_eq!(
        shared_instance_state_next(&mut events).await,
        Some(SharedInstanceEvent::Connecting)
    );
    assert_eq!(shared_instance_state_next(&mut events).await, Some(SharedInstanceEvent::Connected));

    drop(stream);
    drop(listener);
    assert_eq!(
        shared_instance_state_next(&mut events).await,
        Some(SharedInstanceEvent::Disconnected)
    );
    assert_eq!(
        shared_instance_state_next(&mut events).await,
        Some(SharedInstanceEvent::Connecting)
    );
    assert!(
        client.transport.connected_to_shared_instance().await,
        "the connected posture survives the drop"
    );
    assert_eq!(client.transport.shared_instance(), Some(client.iface), "the slot keeps the iface");

    std::fs::remove_file(&socket).expect("remove the stale socket");
    let listener = tokio::net::UnixListener::bind(&socket).expect("bind the socket again");
    let (stream, _) = timeout(LOOPBACK_TIMEOUT, listener.accept())
        .await
        .expect("timed out waiting for the unix client to redial")
        .expect("accept");
    assert_eq!(shared_instance_state_next(&mut events).await, Some(SharedInstanceEvent::Connected));

    assert!(client.transport.detach_shared_instance().await, "the attach is in place");
    assert_eq!(shared_instance_state_next(&mut events).await, None, "the detach ends the stream");

    drop(stream);
    timeout(STOP_TIMEOUT, client.transport.stop()).await.expect("client stop completes");
}

/// A state channel of capacity one that is never drained: the attach's `Connecting` fills
/// it and every later event is dropped at debug, while the consumer keeps serving the
/// restart, so the re-announce lands and the tunnel is re-synthesized.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_state_events_full_channel_drops_at_debug_without_stalling() {
    install_test_logger();
    let name = "shared-instance-state-full";
    let server = shared_instance_server().await;
    let mut server_announces = server.transport.recv_announces().await;
    let (client, events) = shared_instance_state_client(name, server.endpoint(), 1).await;
    server.await_accepted_connections(1).await;
    shared_instance_assert_attached(&client.transport, client.iface).await;
    let registered =
        shared_instance_reconnect_register(&client, &mut server_announces, "full").await;

    let (restarted, mut restarted_announces, _) = shared_instance_reconnect_restart(server).await;
    shared_instance_reconnect_await_reannounces(&mut restarted_announces, vec![registered]).await;
    restarted.await_tunneled_destinations(vec![registered]).await;

    let dropped: Vec<String> = shared_instance_records_for(name)
        .into_iter()
        .filter(|(level, record)| {
            *level == log::Level::Debug
                && record.contains("state event")
                && record.contains("dropped, channel full")
        })
        .map(|(_, record)| record)
        .collect();
    assert!(
        !dropped.is_empty(),
        "a full channel drops at debug: {:?}",
        shared_instance_records_for(name)
    );

    drop(events);
    timeout(STOP_TIMEOUT, client.transport.stop()).await.expect("client stop completes");
    timeout(STOP_TIMEOUT, restarted.transport.stop())
        .await
        .expect("restarted shared instance stop completes");
}
