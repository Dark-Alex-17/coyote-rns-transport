// Coverage for the attach's dial-failure reporting and the opt-in first-connect timeout: a
// client attached while nothing listens reads `Connecting` then one `DialFailed` per failed
// attempt, `Refused` over TCP and `NotFound` over a unix path nothing has bound, with the
// posture and the slot set as for any attach, and `Connected` once the socket exists with
// no `DialFailed` after it; with `with_first_connect_timeout` the attach that sees no
// connect in time detaches and returns `SharedInstanceConnectTimeout`, the stream ending
// after `Connecting` and the failures, while a listening server keeps the attach `Ok`.
// The hand-driven consumer pins the dial-failure arm live and in the drain, and the
// first-connect notifier whose receiver stopped waiting. Helpers come from the sibling
// `tests_parts` files included into this module.

/// Reads `DialFailed { reason }` events until `count` have arrived; any other event fails.
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_expect_dial_failures(
    events: &mut tokio::sync::mpsc::Receiver<SharedInstanceEvent>,
    reason: DialFailureKind,
    count: usize,
) {
    for attempt in 1..=count {
        assert_eq!(
            shared_instance_state_next(events).await,
            Some(SharedInstanceEvent::DialFailed { reason }),
            "attempt {attempt}"
        );
    }
}

/// Reads until `Connected` arrives, letting the `DialFailed` queued before the bind landed
/// through. The dial-failure and connect channels are separate, so a refusal queued before
/// the connect may still be served within one `SHARED_INSTANCE_RECONNECT_WAIT` after
/// `Connected`; those are let through too, and the quiet window after them must hold no
/// `DialFailed`.
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_expect_connected_then_quiet(
    events: &mut tokio::sync::mpsc::Receiver<SharedInstanceEvent>,
) {
    loop {
        match shared_instance_state_next(events).await {
            Some(SharedInstanceEvent::Connected) => break,
            Some(SharedInstanceEvent::DialFailed { .. }) => continue,
            other => panic!("expected DialFailed or Connected before the bind, got {other:?}"),
        }
    }
    let stale_until = tokio::time::Instant::now() + SHARED_INSTANCE_RECONNECT_WAIT;
    while let Ok(event) = tokio::time::timeout_at(stale_until, events.recv()).await {
        assert!(
            matches!(event, Some(SharedInstanceEvent::DialFailed { .. })),
            "only a stale DialFailed may trail Connected, got {event:?}"
        );
    }
    match timeout(SHARED_INSTANCE_STATE_QUIET, events.recv()).await {
        Err(_) => {}
        Ok(Some(SharedInstanceEvent::DialFailed { reason })) => {
            panic!("a DialFailed {reason:?} after Connected")
        }
        Ok(other) => panic!("unexpected event after Connected: {other:?}"),
    }
}

/// TCP, nothing listening: `Connecting`, then `DialFailed { Refused }` per attempt with the
/// posture and the slot set; a server on the port turns the stream to `Connected`.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_dial_failures_read_refused_until_the_tcp_listener_binds() {
    let port = free_loopback_port().await;
    let (client, mut events) = shared_instance_state_client(
        "shared-instance-dial-refused",
        SharedInstanceEndpoint::Tcp(SocketAddr::from(([127, 0, 0, 1], port))),
        SHARED_INSTANCE_STATE_REDIAL_CAPACITY,
    )
    .await;
    assert_eq!(
        shared_instance_state_next(&mut events).await,
        Some(SharedInstanceEvent::Connecting)
    );
    shared_instance_expect_dial_failures(&mut events, DialFailureKind::Refused, 2).await;
    assert!(
        client.transport.connected_to_shared_instance().await,
        "the connected posture is set by the attach"
    );
    assert_eq!(client.transport.shared_instance(), Some(client.iface), "the slot keeps the iface");

    let server = shared_instance_server_on(port).await;
    server.await_accepted_connections(1).await;
    shared_instance_expect_connected_then_quiet(&mut events).await;

    timeout(STOP_TIMEOUT, client.transport.stop()).await.expect("client stop completes");
    timeout(STOP_TIMEOUT, server.transport.stop()).await.expect("shared instance stop completes");
}

/// Unix, a path nothing has bound: `Connecting`, then `DialFailed { NotFound }` per attempt;
/// a unix shared instance bound at the path turns the stream to `Connected`.
#[cfg(unix)]
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_dial_failures_read_not_found_until_the_unix_socket_is_bound() {
    let temp = tempfile::tempdir().expect("tempdir");
    let socket = temp.path().join("shared.sock");
    let (client, mut events) = shared_instance_state_client(
        "shared-instance-dial-not-found",
        SharedInstanceEndpoint::Unix(LocalUnixEndpoint::filesystem(socket.clone())),
        SHARED_INSTANCE_STATE_REDIAL_CAPACITY,
    )
    .await;
    assert_eq!(
        shared_instance_state_next(&mut events).await,
        Some(SharedInstanceEvent::Connecting)
    );
    shared_instance_expect_dial_failures(&mut events, DialFailureKind::NotFound, 2).await;

    let (owner, bound) = shared_instance_unix_owner(temp.path()).await;
    assert_eq!(bound, LocalUnixEndpoint::filesystem(socket), "the owner binds the dialed path");
    shared_instance_expect_connected_then_quiet(&mut events).await;

    timeout(STOP_TIMEOUT, client.transport.stop()).await.expect("client stop completes");
    timeout(STOP_TIMEOUT, owner.stop()).await.expect("unix shared instance stop completes");
}

/// `with_first_connect_timeout` against a port nobody listens on: the attach returns
/// `SharedInstanceConnectTimeout`, the slot and the posture are released, and the stream
/// reads `Connecting`, the failures of the attempts that fit, and then ends, never
/// `Connected`.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_first_connect_timeout_detaches_when_nothing_listens() {
    let port = free_loopback_port().await;
    let (events_tx, mut events) = tokio::sync::mpsc::channel(16);
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let transport = Transport::new(TransportConfig::new(
        "shared-instance-first-connect-timeout",
        &identity,
        false,
    ));
    let outcome = transport
        .attach_shared_instance(
            SharedInstanceEndpoint::Tcp(SocketAddr::from(([127, 0, 0, 1], port))),
            SharedInstanceOptions::default()
                .with_reconnect_wait(SHARED_INSTANCE_RECONNECT_WAIT)
                .with_state_events(events_tx)
                .with_first_connect_timeout(Duration::from_millis(300)),
        )
        .await;
    assert!(
        matches!(outcome, Err(RnsError::SharedInstanceConnectTimeout)),
        "the attach gives up: {outcome:?}"
    );
    assert_eq!(transport.shared_instance(), None, "the slot is released");
    assert!(!transport.connected_to_shared_instance().await, "the posture is off");

    let read = shared_instance_state_read_to_end(&mut events).await;
    assert_eq!(read.first(), Some(&SharedInstanceEvent::Connecting), "{read:?}");
    assert!(
        read.len() >= 2,
        "a 300 ms wait over 100 ms redials on loopback reads at least one refusal: {read:?}"
    );
    assert!(
        read[1..].iter().all(|event| matches!(event, SharedInstanceEvent::DialFailed { .. })),
        "only dial failures follow Connecting: {read:?}"
    );
    assert!(
        read[1..].iter().all(|event| {
            *event == SharedInstanceEvent::DialFailed { reason: DialFailureKind::Refused }
        }),
        "every failure is a refusal: {read:?}"
    );

    timeout(STOP_TIMEOUT, transport.stop()).await.expect("client stop completes");
}

/// The positive control: `with_first_connect_timeout` against a listening shared instance
/// returns `Ok` with the stream reading `Connecting`, `Connected`.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_first_connect_timeout_passes_when_the_server_listens() {
    let server = shared_instance_server().await;
    let (events_tx, mut events) = tokio::sync::mpsc::channel(8);
    let client = shared_instance_client_with_options(
        "shared-instance-first-connect-ok",
        server.endpoint(),
        SharedInstanceOptions::default()
            .with_reconnect_wait(SHARED_INSTANCE_RECONNECT_WAIT)
            .with_state_events(events_tx)
            .with_first_connect_timeout(Duration::from_secs(2)),
    )
    .await;
    shared_instance_assert_attached(&client.transport, client.iface).await;
    assert_eq!(
        shared_instance_state_next(&mut events).await,
        Some(SharedInstanceEvent::Connecting)
    );
    assert_eq!(shared_instance_state_next(&mut events).await, Some(SharedInstanceEvent::Connected));

    timeout(STOP_TIMEOUT, client.transport.stop()).await.expect("client stop completes");
    timeout(STOP_TIMEOUT, server.transport.stop()).await.expect("shared instance stop completes");
}

/// The hand-driven consumer turns the queued dial failures into `DialFailed` with their
/// kinds in the drain, behind the disconnect served live and the connect drained ahead of
/// them; the live arm is pinned by `shared_instance_state_consumer_serves_a_dial_failure_live`.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_state_consumer_serves_dial_failures_last() {
    install_test_logger();
    let read = shared_instance_state_drive_consumer_with(
        "shared-instance-state-consumer-dial-failures",
        DriveSpec {
            queued: &[
                Queued::DialFailure(DialFailureKind::Refused),
                Queued::Connect,
                Queued::Disconnect,
                Queued::DialFailure(DialFailureKind::Timeout),
            ],
            close_before_run: &[
                Queued::Reconnect,
                Queued::Connect,
                Queued::Disconnect,
                Queued::DialFailure(DialFailureKind::Refused),
            ],
            cancel_before_run: false,
            first_connect_abandoned: false,
            disconnect_reasons: false,
            exits: "the consumer exits once the channels are closed",
        },
    )
    .await;
    assert_eq!(
        read,
        [
            SharedInstanceEvent::Disconnected,
            SharedInstanceEvent::Connecting,
            SharedInstanceEvent::Connected,
            SharedInstanceEvent::DialFailed { reason: DialFailureKind::Refused },
            SharedInstanceEvent::DialFailed { reason: DialFailureKind::Timeout },
        ]
    );
}

/// The live dial-failure arm: with the other three senders still open, only the
/// dial-failure sender is gone, so the biased select reaches its arm with the queued
/// failure, serves it, and leaves on the `None` behind it with nothing left to drain. The
/// absent drain record is what tells the live arm from the drain.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_state_consumer_serves_a_dial_failure_live() {
    install_test_logger();
    let name = "shared-instance-state-consumer-dial-failure-live";
    let read = shared_instance_state_drive_consumer_with(
        name,
        DriveSpec {
            queued: &[Queued::DialFailure(DialFailureKind::Refused)],
            close_before_run: &[Queued::DialFailure(DialFailureKind::Refused)],
            cancel_before_run: false,
            first_connect_abandoned: false,
            disconnect_reasons: false,
            exits: "the consumer exits once the dial-failure channel is drained and closed",
        },
    )
    .await;
    assert_eq!(read, [SharedInstanceEvent::DialFailed { reason: DialFailureKind::Refused }]);
    let records: Vec<String> =
        shared_instance_records_for(name).into_iter().map(|(_, record)| record).collect();
    assert!(
        !records.iter().any(|record| record.contains("drained")),
        "the failure was served live, not drained: {records:?}"
    );
}

/// A first-connect notifier whose receiver is already gone when the connect is served: the
/// attach stopped waiting (its timeout ran out, or a stop, detach or `stop_interface` ended
/// it), so the consumer records that at debug, still sends `Connected`, and keeps serving.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_state_consumer_notes_a_connect_after_the_attach_stopped_waiting() {
    install_test_logger();
    let name = "shared-instance-state-consumer-late-first-connect";
    let read = shared_instance_state_drive_consumer_with(
        name,
        DriveSpec {
            queued: &[Queued::Connect],
            close_before_run: &[Queued::Reconnect],
            cancel_before_run: false,
            first_connect_abandoned: true,
            disconnect_reasons: false,
            exits: "the consumer exits once the reconnect channel is closed",
        },
    )
    .await;
    assert_eq!(read, [SharedInstanceEvent::Connected]);
    let records: Vec<String> =
        shared_instance_records_for(name).into_iter().map(|(_, record)| record).collect();
    assert!(
        records.iter().any(|record| record.contains("connected after the attach stopped waiting")),
        "{records:?}"
    );
}
