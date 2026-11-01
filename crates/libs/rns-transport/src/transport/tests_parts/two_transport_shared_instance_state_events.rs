// Coverage for the shared-instance client's state stream: an attach sends `Connecting`
// then `Connected`, a dropped connection sends `Disconnected` then `Connecting` after the
// drop is served (the flush itself runs only while the transport is still attached) and
// `Connected` when the stream is back, a detach ends the stream, a client attached while
// nothing listens stays in `Connecting` until the socket exists, and a full channel drops
// the event at debug without stalling the flush or the re-announce. The consumer driven by
// hand (its `DriveSpec` and driver live in the `_spec` sibling) pins the order the events
// take within one cycle. Helpers come from the sibling `tests_parts` files included into
// this module.

/// How long a client attached to a closed port is watched for an event that must not come.
const SHARED_INSTANCE_STATE_QUIET: Duration = Duration::from_millis(500);

/// The state channel capacity a pin that dials a closed port passes: the client redials
/// every `SHARED_INSTANCE_RECONNECT_WAIT` (100 ms) and each refusal queues a `DialFailed`
/// that nobody drains while the pin waits on the rebind.
const SHARED_INSTANCE_STATE_REDIAL_CAPACITY: usize = 64;

/// Reads the next state event within `LOOPBACK_TIMEOUT`; `None` is the stream's end.
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_state_next(
    events: &mut tokio::sync::mpsc::Receiver<SharedInstanceEvent>,
) -> Option<SharedInstanceEvent> {
    timeout(LOOPBACK_TIMEOUT, events.recv()).await.expect("timed out waiting for a state event")
}

/// Reads the next cycle event (`Connecting`, `Connected`, `Disconnected`,
/// `DisconnectedWithReason`) within `LOOPBACK_TIMEOUT`; `None` is the stream's end. A
/// `DialFailed` is skipped, so this is for the reads a client makes while nothing listens:
/// whether a redial was refused before the rebind landed is a race, and the cycle order is
/// what such a pin cares about. The stream-end read after a `Connected` that followed
/// redials takes it too: a refusal queued on the dial-failure channel before the connect
/// may still land after `Connected`.
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_state_next_cycle_event(
    events: &mut tokio::sync::mpsc::Receiver<SharedInstanceEvent>,
) -> Option<SharedInstanceEvent> {
    loop {
        match shared_instance_state_next(events).await {
            Some(SharedInstanceEvent::DialFailed { .. }) => continue,
            other => return other,
        }
    }
}

/// Watches `events` for `SHARED_INSTANCE_STATE_QUIET` and fails on any cycle event or on the
/// stream's end; the `DialFailed` of a client dialing a closed port are let through.
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_state_assert_no_cycle_event(
    events: &mut tokio::sync::mpsc::Receiver<SharedInstanceEvent>,
    what: &str,
) {
    let watched = timeout(SHARED_INSTANCE_STATE_QUIET, async {
        while let Some(event) = events.recv().await {
            assert!(
                matches!(event, SharedInstanceEvent::DialFailed { .. }),
                "{what}, got {event:?}"
            );
        }
        panic!("{what}, but the stream ended");
    })
    .await;
    assert!(watched.is_err(), "{what}: the quiet window runs out first");
}

/// Reads `state` until the stream ends, within `STOP_TIMEOUT`; the hand-driven consumer's
/// return drops the last sender, so this is what it left on the stream.
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_state_read_to_end(
    state: &mut tokio::sync::mpsc::Receiver<SharedInstanceEvent>,
) -> Vec<SharedInstanceEvent> {
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
/// nothing else in between (the `DialFailed` of the refused redials aside).
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_state_events_follow_a_tcp_restart() {
    let server = shared_instance_server().await;
    let (client, mut events) = shared_instance_state_client(
        "shared-instance-state-tcp",
        server.endpoint(),
        SHARED_INSTANCE_STATE_REDIAL_CAPACITY,
    )
    .await;
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
    assert_eq!(
        shared_instance_state_next_cycle_event(&mut events).await,
        Some(SharedInstanceEvent::Connected)
    );

    assert!(client.transport.detach_shared_instance().await, "the attach is in place");
    assert_eq!(
        shared_instance_state_next_cycle_event(&mut events).await,
        None,
        "the detach ends the stream"
    );

    timeout(STOP_TIMEOUT, client.transport.stop()).await.expect("client stop completes");
    timeout(STOP_TIMEOUT, restarted.transport.stop())
        .await
        .expect("restarted shared instance stop completes");
}

/// Drives a hand-built consumer named `name` over a queued connect and the reconnect
/// behind it, with a disconnect queued ahead of both when `with_disconnect`; the
/// reconnect sender is gone before the consumer polls, the other two after it returns.
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_state_drive_consumer(
    name: &str,
    with_disconnect: bool,
) -> Vec<SharedInstanceEvent> {
    let queued: &[Queued] = if with_disconnect {
        &[Queued::Disconnect, Queued::Connect, Queued::Reconnect]
    } else {
        &[Queued::Connect, Queued::Reconnect]
    };
    shared_instance_state_drive_consumer_with(
        name,
        DriveSpec {
            queued,
            close_before_run: &[Queued::Reconnect],
            cancel_before_run: false,
            first_connect_abandoned: false,
            disconnect_reasons: false,
            exits: "the consumer exits once the reconnect channel is drained and closed",
        },
    )
    .await
}

/// A connect and the reconnect behind it: the stream reads `Connected` alone and the
/// reconnect is still served, so the stream rides beside the re-synthesis rather than in
/// place of it. The hand-built consumer's random iface is registered nowhere, so the arm
/// served is the one that records the reconnect without re-synthesizing the tunnel.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_state_consumer_reports_connected_and_serves_the_reconnect() {
    install_test_logger();
    let name = "shared-instance-state-consumer-connect";
    let read = shared_instance_state_drive_consumer(name, false).await;
    assert_eq!(read, [SharedInstanceEvent::Connected]);
    let records: Vec<String> =
        shared_instance_records_for(name).into_iter().map(|(_, record)| record).collect();
    assert!(
        records.iter().any(|record| record.contains("reconnected but no longer registered")),
        "{records:?}"
    );
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

/// A client attached to a port nothing listens on reads `Connecting` and no cycle event
/// (only the `DialFailed` of each refused attempt) until a shared instance binds the port,
/// then `Connected`.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_state_events_wait_in_connecting_while_the_listener_is_down() {
    let port = free_loopback_port().await;
    let (client, mut events) = shared_instance_state_client(
        "shared-instance-state-down",
        SharedInstanceEndpoint::Tcp(SocketAddr::from(([127, 0, 0, 1], port))),
        SHARED_INSTANCE_STATE_REDIAL_CAPACITY,
    )
    .await;
    assert_eq!(
        shared_instance_state_next(&mut events).await,
        Some(SharedInstanceEvent::Connecting)
    );
    shared_instance_state_assert_no_cycle_event(
        &mut events,
        "no cycle event while nothing listens",
    )
    .await;
    assert!(
        client.transport.connected_to_shared_instance().await,
        "the connected posture is set by the attach"
    );
    assert_eq!(client.transport.shared_instance(), Some(client.iface), "the slot keeps the iface");

    let server = shared_instance_server_on(port).await;
    server.await_accepted_connections(1).await;
    assert_eq!(
        shared_instance_state_next_cycle_event(&mut events).await,
        Some(SharedInstanceEvent::Connected)
    );

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
        SHARED_INSTANCE_STATE_REDIAL_CAPACITY,
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
    assert_eq!(
        shared_instance_state_next_cycle_event(&mut events).await,
        Some(SharedInstanceEvent::Connected)
    );

    assert!(client.transport.detach_shared_instance().await, "the attach is in place");
    assert_eq!(
        shared_instance_state_next_cycle_event(&mut events).await,
        None,
        "the detach ends the stream"
    );

    drop(stream);
    timeout(STOP_TIMEOUT, client.transport.stop()).await.expect("client stop completes");
}

/// A state channel of capacity one that is never drained: the attach's `Connecting` fills
/// it and every later event is dropped at debug, while the consumer keeps serving the
/// restart, so the re-announce lands and the tunnel is re-synthesized. The one event the
/// channel retains is that `Connecting`, nothing behind it.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_state_events_full_channel_drops_at_debug_without_stalling() {
    install_test_logger();
    let name = "shared-instance-state-full";
    let server = shared_instance_server().await;
    let mut server_announces = server.transport.recv_announces().await;
    let (client, mut events) = shared_instance_state_client(name, server.endpoint(), 1).await;
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
                && !record.contains("DialFailed")
        })
        .map(|(_, record)| record)
        .collect();
    assert_eq!(
        dropped.len(),
        4,
        "the first `Connected`, the drop's `Disconnected` and `Connecting`, and the restart's `Connected` are dropped at debug: {:?}",
        shared_instance_records_for(name)
    );

    assert_eq!(
        shared_instance_state_next(&mut events).await,
        Some(SharedInstanceEvent::Connecting),
        "the retained event is the attach's own `Connecting`"
    );
    assert!(events.try_recv().is_err(), "nothing was queued behind the retained event");
    drop(events);
    timeout(STOP_TIMEOUT, client.transport.stop()).await.expect("client stop completes");
    timeout(STOP_TIMEOUT, restarted.transport.stop())
        .await
        .expect("restarted shared instance stop completes");
}
