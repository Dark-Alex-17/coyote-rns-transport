// Usage probes for the shared-instance state stream beyond the single restart cycle: a
// `Transport::stop` ends the stream without a `Disconnected`, a refused attach (remote
// endpoint, or the slot already taken) sends nothing and drops the sender, a receiver the
// caller let go does not stall the consumer (the restart is still served and the
// re-announce lands), two restarts in a row read two whole cycles with nothing in between,
// and a detach taken while the client is still dialing ends the stream with no `Connected`.
// Helpers come from the sibling `tests_parts` files included into this module.

/// A client stopped while connected: the stream ends and reads no `Disconnected` first,
/// since a stop flushes nothing and the sender is dropped with the consumer.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_state_stream_ends_on_stop_without_a_disconnected() {
    let server = shared_instance_server().await;
    let (client, mut events) =
        shared_instance_state_client("shared-instance-state-stop", server.endpoint(), 8).await;
    server.await_accepted_connections(1).await;
    assert_eq!(
        shared_instance_state_next(&mut events).await,
        Some(SharedInstanceEvent::Connecting)
    );
    assert_eq!(shared_instance_state_next(&mut events).await, Some(SharedInstanceEvent::Connected));

    timeout(STOP_TIMEOUT, client.transport.stop()).await.expect("client stop completes");
    let mut after_stop = Vec::new();
    while let Some(event) = shared_instance_state_next(&mut events).await {
        after_stop.push(event);
    }
    assert!(after_stop.is_empty(), "a stop sends no event, read {after_stop:?}");
    assert!(!client.transport.connected_to_shared_instance().await, "the stop clears the posture");

    timeout(STOP_TIMEOUT, server.transport.stop()).await.expect("shared instance stop completes");
}

/// An attach refused before the client is spawned, by a remote endpoint or by a slot
/// already taken, sends nothing: the options carrying the sender are dropped with the
/// refusal, so the stream ends at once, and the attach already in place reads nothing
/// extra.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_state_refused_attach_sends_nothing_and_ends_the_stream() {
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let transport =
        Transport::new(TransportConfig::new("shared-instance-state-refused", &identity, false));

    let (remote_tx, mut remote_events) = tokio::sync::mpsc::channel(8);
    let remote = SharedInstanceEndpoint::Tcp(SocketAddr::from(([203, 0, 113, 7], 4242)));
    let refused = transport
        .attach_shared_instance(
            remote,
            SharedInstanceOptions::default().with_state_events(remote_tx),
        )
        .await;
    assert!(
        matches!(refused, Err(RnsError::SharedInstanceEndpointNotLoopback)),
        "a remote endpoint is refused: {refused:?}"
    );
    assert_eq!(
        shared_instance_state_next(&mut remote_events).await,
        None,
        "the refused attach sends nothing and drops the sender"
    );

    let server = shared_instance_server().await;
    let (first_tx, mut first_events) = tokio::sync::mpsc::channel(8);
    let iface = transport
        .attach_shared_instance(
            server.endpoint(),
            SharedInstanceOptions::default()
                .with_reconnect_wait(SHARED_INSTANCE_RECONNECT_WAIT)
                .with_state_events(first_tx),
        )
        .await
        .expect("attach to the shared instance");
    server.await_accepted_connections(1).await;
    assert_eq!(
        shared_instance_state_next(&mut first_events).await,
        Some(SharedInstanceEvent::Connecting)
    );
    assert_eq!(
        shared_instance_state_next(&mut first_events).await,
        Some(SharedInstanceEvent::Connected)
    );

    let (second_tx, mut second_events) = tokio::sync::mpsc::channel(8);
    let second = transport
        .attach_shared_instance(
            server.endpoint(),
            SharedInstanceOptions::default().with_state_events(second_tx),
        )
        .await;
    assert!(matches!(second, Err(RnsError::InvalidArgument)), "the slot refuses a second attach");
    assert_eq!(
        shared_instance_state_next(&mut second_events).await,
        None,
        "the refused second attach sends nothing and drops its sender"
    );
    assert!(
        timeout(SHARED_INSTANCE_STATE_QUIET, first_events.recv()).await.is_err(),
        "the attach in place reads nothing from the refusal"
    );
    assert_eq!(transport.shared_instance(), Some(iface), "the slot still holds the first attach");

    assert!(transport.detach_shared_instance().await, "the first attach is in place");
    assert_eq!(shared_instance_state_next(&mut first_events).await, None);
    timeout(STOP_TIMEOUT, transport.stop()).await.expect("client stop completes");
    timeout(STOP_TIMEOUT, server.transport.stop()).await.expect("shared instance stop completes");
}

/// The caller drops its receiver after the first two events: every later `try_send` hits a
/// closed channel and the consumer keeps serving, so the restart still re-synthesizes the
/// tunnel and lands the re-announce, and the posture is untouched.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_state_dropped_receiver_keeps_the_consumer_serving() {
    let server = shared_instance_server().await;
    let mut server_announces = server.transport.recv_announces().await;
    let (client, mut events) =
        shared_instance_state_client("shared-instance-state-receiver-gone", server.endpoint(), 8)
            .await;
    server.await_accepted_connections(1).await;
    assert_eq!(
        shared_instance_state_next(&mut events).await,
        Some(SharedInstanceEvent::Connecting)
    );
    assert_eq!(shared_instance_state_next(&mut events).await, Some(SharedInstanceEvent::Connected));
    let registered =
        shared_instance_reconnect_register(&client, &mut server_announces, "receiver-gone").await;
    drop(events);

    let (restarted, mut restarted_announces, _) = shared_instance_reconnect_restart(server).await;
    shared_instance_reconnect_await_reannounces(&mut restarted_announces, vec![registered]).await;
    restarted.await_tunneled_destinations(vec![registered]).await;
    shared_instance_assert_attached(&client.transport, client.iface).await;

    assert!(client.transport.detach_shared_instance().await, "the attach is in place");
    timeout(STOP_TIMEOUT, client.transport.stop()).await.expect("client stop completes");
    timeout(STOP_TIMEOUT, restarted.transport.stop())
        .await
        .expect("restarted shared instance stop completes");
}

/// Two restarts on the same port: the stream reads two whole `Disconnected`, `Connecting`,
/// `Connected` cycles after the attach's pair, each `Connected` only once the listener is
/// back, with nothing else in between, and ends on the detach.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_state_events_follow_two_tcp_restarts() {
    let mut server = shared_instance_server().await;
    let port = server.port;
    let (client, mut events) =
        shared_instance_state_client("shared-instance-state-two-cycles", server.endpoint(), 8)
            .await;
    server.await_accepted_connections(1).await;
    assert_eq!(
        shared_instance_state_next(&mut events).await,
        Some(SharedInstanceEvent::Connecting)
    );
    assert_eq!(shared_instance_state_next(&mut events).await, Some(SharedInstanceEvent::Connected));

    for cycle in 0..2 {
        timeout(STOP_TIMEOUT, server.transport.stop())
            .await
            .expect("shared instance stop completes");
        assert_eq!(
            shared_instance_state_next(&mut events).await,
            Some(SharedInstanceEvent::Disconnected),
            "cycle {cycle}"
        );
        assert_eq!(
            shared_instance_state_next(&mut events).await,
            Some(SharedInstanceEvent::Connecting),
            "cycle {cycle}"
        );
        assert!(
            timeout(SHARED_INSTANCE_STATE_QUIET, events.recv()).await.is_err(),
            "cycle {cycle}: no Connected before the listener is back"
        );
        server = shared_instance_server_on(port).await;
        server.await_accepted_connections(1).await;
        assert_eq!(
            shared_instance_state_next(&mut events).await,
            Some(SharedInstanceEvent::Connected),
            "cycle {cycle}"
        );
    }
    assert!(client.transport.connected_to_shared_instance().await, "the posture is still set");

    assert!(client.transport.detach_shared_instance().await, "the attach is in place");
    assert_eq!(shared_instance_state_next(&mut events).await, None, "the detach ends the stream");
    timeout(STOP_TIMEOUT, client.transport.stop()).await.expect("client stop completes");
    timeout(STOP_TIMEOUT, server.transport.stop()).await.expect("shared instance stop completes");
}

/// A detach taken while nothing listens: the stream read `Connecting` alone and ends on the
/// detach, no `Connected` or `Disconnected` follows even once a shared instance binds the
/// port, and the posture and slot are released.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_state_detach_while_connecting_ends_the_stream() {
    let port = free_loopback_port().await;
    let (client, mut events) = shared_instance_state_client(
        "shared-instance-state-detach-dialing",
        SharedInstanceEndpoint::Tcp(SocketAddr::from(([127, 0, 0, 1], port))),
        8,
    )
    .await;
    assert_eq!(
        shared_instance_state_next(&mut events).await,
        Some(SharedInstanceEvent::Connecting)
    );

    assert!(client.transport.detach_shared_instance().await, "the attach is in place");
    assert_eq!(shared_instance_state_next(&mut events).await, None, "the detach ends the stream");
    shared_instance_assert_released(&client.transport, client.iface).await;

    let server = shared_instance_server_on(port).await;
    tokio::time::sleep(SHARED_INSTANCE_STATE_QUIET).await;
    assert_eq!(
        server.status.to_json()["accepted_connections"].as_u64(),
        Some(0),
        "the detached client no longer dials"
    );

    timeout(STOP_TIMEOUT, client.transport.stop()).await.expect("client stop completes");
    timeout(STOP_TIMEOUT, server.transport.stop()).await.expect("shared instance stop completes");
}
