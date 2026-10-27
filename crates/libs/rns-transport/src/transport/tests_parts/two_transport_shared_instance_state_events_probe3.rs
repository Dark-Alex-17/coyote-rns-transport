// Coverage for the connect half of the state-stream promise (README, CHANGELOG,
// `SharedInstanceEvent::Disconnected` and `with_state_events` rustdoc): "a connect already
// queued is read as `Connected`" before the stream ends, the twin of the pinned queued-drop
// case. The two tests take the same hand-driven shape as
// `shared_instance_state_consumer_serves_a_queued_drop_before_the_stream_ends`: one event
// queued, then the senders go away as they do when the client worker exits. The first test
// is the doc's claim verbatim, with every sender gone so the disconnect channel closes
// before the connect is polled; the second keeps the disconnect sender alive so the live
// connect arm is reached, and reads the same. Helpers come from the sibling `tests_parts`
// files included into this module.

/// A connect queued when every sender the client worker held is already gone, exactly the
/// shape the queued-drop test uses for `Disconnected`. The docs read this as
/// `Connected` before the stream ends.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_state_consumer_serves_a_queued_connect_before_the_stream_ends() {
    install_test_logger();
    let name = "shared-instance-state-consumer-queued-connect";
    let iface = AddressHash::new_from_rand(OsRng);
    let (reconnects_tx, reconnects) = tokio::sync::mpsc::channel(1);
    let (connects_tx, connects) = tokio::sync::mpsc::channel(1);
    let (disconnects_tx, disconnects) = tokio::sync::mpsc::channel(1);
    let (state_tx, mut state) = tokio::sync::mpsc::channel(8);
    connects_tx.send(iface).await.expect("queue the connect");
    drop(reconnects_tx);
    drop(connects_tx);
    drop(disconnects_tx);

    let (_transport, consumer) = shared_instance_reconnect_consumer(name, iface);
    let cancel = consumer.cancel.clone();
    timeout(STOP_TIMEOUT, consumer.run(reconnects, connects, disconnects, Some(state_tx)))
        .await
        .expect("the consumer exits once the channels are closed");
    cancel.cancel();

    let read = shared_instance_state_read_to_end(&mut state).await;
    assert_eq!(
        read,
        [SharedInstanceEvent::Connected],
        "the docs promise a connect already queued is read as `Connected` before the stream ends"
    );
}

/// The same queued connect with the disconnect sender still alive while the consumer polls:
/// the biased select finds the disconnect channel open and empty, moves on, and reads the
/// connect from its live arm. Together with the test above this pins that both shapes read
/// the connect, this one being the open-channel positive control.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_state_consumer_reads_a_queued_connect_while_disconnects_stay_open() {
    install_test_logger();
    let name = "shared-instance-state-consumer-queued-connect-open";
    let iface = AddressHash::new_from_rand(OsRng);
    let (reconnects_tx, reconnects) = tokio::sync::mpsc::channel(1);
    let (connects_tx, connects) = tokio::sync::mpsc::channel(1);
    let (disconnects_tx, disconnects) = tokio::sync::mpsc::channel(1);
    let (state_tx, mut state) = tokio::sync::mpsc::channel(8);
    connects_tx.send(iface).await.expect("queue the connect");
    drop(reconnects_tx);
    drop(connects_tx);

    let (_transport, consumer) = shared_instance_reconnect_consumer(name, iface);
    let cancel = consumer.cancel.clone();
    timeout(STOP_TIMEOUT, consumer.run(reconnects, connects, disconnects, Some(state_tx)))
        .await
        .expect("the consumer exits once the connect channel is drained and closed");
    cancel.cancel();
    drop(disconnects_tx);

    let read = shared_instance_state_read_to_end(&mut state).await;
    assert_eq!(
        read,
        [SharedInstanceEvent::Connected],
        "the queued connect is read while the disconnect channel is still open"
    );
}
