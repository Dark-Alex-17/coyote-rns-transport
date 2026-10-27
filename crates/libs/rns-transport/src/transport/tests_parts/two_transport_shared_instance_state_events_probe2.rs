// Usage probes for the three promises the shared-instance state-stream docs make beyond
// the single attach: a retained `SharedInstanceOptions` clone keeps the stream open past a
// detach and the stream ends only when that clone goes too; a drop already queued when the
// client worker's senders go away is still served, `Disconnected` then `Connecting`, before
// the stream ends; and under a storm the queued disconnects are served before the connects,
// so two `Disconnected`, each with its `Connecting`, precede both `Connected`s. Helpers come
// from the sibling `tests_parts` files included into this module.

/// The caller keeps a clone of the options it attached with: the clone carries a clone of
/// the state sender, so the detach drops only the attach's clone and the receiver stays
/// pending, reading nothing, until the retained options are dropped, when the stream ends.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_state_retained_options_clone_keeps_the_stream_open_after_detach() {
    let server = shared_instance_server().await;
    let (events_tx, mut events) = tokio::sync::mpsc::channel(8);
    let retained = SharedInstanceOptions::default()
        .with_reconnect_wait(SHARED_INSTANCE_RECONNECT_WAIT)
        .with_state_events(events_tx);
    let client = shared_instance_client_with_options(
        "shared-instance-state-retained-clone",
        server.endpoint(),
        retained.clone(),
    )
    .await;
    server.await_accepted_connections(1).await;
    assert_eq!(
        shared_instance_state_next(&mut events).await,
        Some(SharedInstanceEvent::Connecting)
    );
    assert_eq!(shared_instance_state_next(&mut events).await, Some(SharedInstanceEvent::Connected));

    assert!(client.transport.detach_shared_instance().await, "the attach is in place");
    assert!(
        !client.transport.connected_to_shared_instance().await,
        "the detach clears the posture"
    );
    let while_retained = timeout(SHARED_INSTANCE_STATE_QUIET, events.recv()).await;
    assert!(
        while_retained.is_err(),
        "the retained clone keeps the stream open and the detach sends nothing: {while_retained:?}"
    );

    drop(retained);
    assert_eq!(
        shared_instance_state_next(&mut events).await,
        None,
        "the stream ends once the last clone of the sender is gone"
    );

    timeout(STOP_TIMEOUT, client.transport.stop()).await.expect("client stop completes");
    timeout(STOP_TIMEOUT, server.transport.stop()).await.expect("shared instance stop completes");
}

/// A drop queued when every sender the client worker held is already gone, the shape a
/// detach leaves behind when the stream ended an instant before it: the consumer serves the
/// queued disconnect, `Disconnected` then `Connecting`, and only then ends the stream.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_state_consumer_serves_a_queued_drop_before_the_stream_ends() {
    install_test_logger();
    let name = "shared-instance-state-consumer-queued-drop";
    let iface = AddressHash::new_from_rand(OsRng);
    let (reconnects_tx, reconnects) = tokio::sync::mpsc::channel(1);
    let (connects_tx, connects) = tokio::sync::mpsc::channel(1);
    let (disconnects_tx, disconnects) = tokio::sync::mpsc::channel(1);
    let (state_tx, mut state) = tokio::sync::mpsc::channel(8);
    disconnects_tx.send(iface).await.expect("queue the disconnect");
    drop(reconnects_tx);
    drop(connects_tx);
    drop(disconnects_tx);

    let (_transport, consumer) = shared_instance_reconnect_consumer(name, iface);
    let cancel = consumer.cancel.clone();
    timeout(STOP_TIMEOUT, consumer.run(reconnects, connects, disconnects, Some(state_tx)))
        .await
        .expect("the consumer exits once the queued drop is served and the channels are closed");
    cancel.cancel();

    let read = timeout(STOP_TIMEOUT, async {
        let mut read = Vec::new();
        while let Some(event) = state.recv().await {
            read.push(event);
        }
        read
    })
    .await
    .expect("the consumer's return ends the stream");
    assert_eq!(
        read,
        [SharedInstanceEvent::Disconnected, SharedInstanceEvent::Connecting],
        "the queued drop is served before the stream ends"
    );
}

/// Two drops, two connects and two reconnects all waiting when the consumer polls, the
/// storm the docs describe: the biased select serves every queued disconnect before any
/// connect, so the stream reads `Disconnected`, `Connecting`, `Disconnected`, `Connecting`,
/// `Connected`, `Connected`; the per-cycle order holds while the two cycles nest, and both
/// reconnects are still served.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_state_consumer_nests_the_cycles_under_a_storm() {
    install_test_logger();
    let name = "shared-instance-state-consumer-storm";
    let iface = AddressHash::new_from_rand(OsRng);
    let (reconnects_tx, reconnects) = tokio::sync::mpsc::channel(2);
    let (connects_tx, connects) = tokio::sync::mpsc::channel(2);
    let (disconnects_tx, disconnects) = tokio::sync::mpsc::channel(2);
    let (state_tx, mut state) = tokio::sync::mpsc::channel(8);
    for _ in 0..2 {
        disconnects_tx.send(iface).await.expect("queue the disconnect");
        connects_tx.send(iface).await.expect("queue the connect");
        reconnects_tx.send(iface).await.expect("queue the reconnect");
    }
    drop(reconnects_tx);

    let (_transport, consumer) = shared_instance_reconnect_consumer(name, iface);
    let cancel = consumer.cancel.clone();
    timeout(STOP_TIMEOUT, consumer.run(reconnects, connects, disconnects, Some(state_tx)))
        .await
        .expect("the consumer exits once the reconnect channel is drained and closed");
    cancel.cancel();
    drop(connects_tx);
    drop(disconnects_tx);

    let read = timeout(STOP_TIMEOUT, async {
        let mut read = Vec::new();
        while let Some(event) = state.recv().await {
            read.push(event);
        }
        read
    })
    .await
    .expect("the consumer's return ends the stream");
    assert_eq!(
        read,
        [
            SharedInstanceEvent::Disconnected,
            SharedInstanceEvent::Connecting,
            SharedInstanceEvent::Disconnected,
            SharedInstanceEvent::Connecting,
            SharedInstanceEvent::Connected,
            SharedInstanceEvent::Connected,
        ],
        "queued disconnects are served before connects, the cycles nest"
    );
    let records: Vec<String> =
        shared_instance_records_for(name).into_iter().map(|(_, record)| record).collect();
    assert_eq!(
        records.iter().filter(|record| record.contains("reconnected")).count(),
        2,
        "both reconnects are still served: {records:?}"
    );
}
