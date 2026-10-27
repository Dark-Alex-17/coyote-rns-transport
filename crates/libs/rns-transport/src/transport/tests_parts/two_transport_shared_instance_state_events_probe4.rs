// Coverage for the drain the consumer runs when the client worker is gone and for the
// posture-before-`Connecting` order of the attach, both promised by the CHANGELOG and the
// `run` rustdoc: with every sender dropped, the disconnects still queued are served first,
// each as `Disconnected`, `Connecting`, then the connects as `Connected`; a queued
// reconnect is left (the worker is gone, nothing to re-synthesize over); a cancel seen by
// the select serves nothing queued; and the attach turns `connected_to_shared_instance()`
// on before it sends the first `Connecting` while the slot still holds the interface, so a
// reader of that event finds the posture on (in the attach/stop race the attach's rustdoc
// describes, `Connecting` goes out with the posture off and the stream ends at once). The
// first three take the hand-driven consumer shape of the sibling probe files; the
// fourth holds the handler lock the posture write needs and watches the stream. Helpers
// come from the sibling `tests_parts` files included into this module.

/// A disconnect and a connect both queued when every sender is gone: the drain serves the
/// disconnect's pair first, then the connect, the order the live arms keep.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_state_consumer_drains_a_queued_drop_then_the_queued_connect() {
    install_test_logger();
    let name = "shared-instance-state-consumer-drain-both";
    let read = shared_instance_state_drive_consumer_with(
        name,
        DriveSpec {
            queued: &[Queued::Disconnect, Queued::Connect],
            close_before_run: &[Queued::Reconnect, Queued::Connect, Queued::Disconnect],
            cancel_before_run: false,
            exits: "the consumer exits once the channels are closed",
        },
    )
    .await;
    assert_eq!(
        read,
        [
            SharedInstanceEvent::Disconnected,
            SharedInstanceEvent::Connecting,
            SharedInstanceEvent::Connected
        ],
        "the drain serves every queued disconnect before any queued connect"
    );
    let records: Vec<String> =
        shared_instance_records_for(name).into_iter().map(|(_, record)| record).collect();
    assert!(
        records.iter().any(|record| record.contains("stream ended after detach, tables kept")),
        "the drained disconnect is served through the same arm as a live one: {records:?}"
    );
}

/// A connect and the reconnect behind it both queued when every sender is gone: the drain
/// reads the connect as `Connected` and leaves the reconnect, so no reconnect record is
/// written for this consumer.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_state_consumer_leaves_a_queued_reconnect_when_the_worker_is_gone() {
    install_test_logger();
    let name = "shared-instance-state-consumer-drain-leaves-reconnect";
    let read = shared_instance_state_drive_consumer_with(
        name,
        DriveSpec {
            queued: &[Queued::Connect, Queued::Reconnect],
            close_before_run: &[Queued::Reconnect, Queued::Connect, Queued::Disconnect],
            cancel_before_run: false,
            exits: "the consumer exits once the channels are closed",
        },
    )
    .await;
    assert_eq!(read, [SharedInstanceEvent::Connected], "the queued connect is still read");
    let records: Vec<String> =
        shared_instance_records_for(name).into_iter().map(|(_, record)| record).collect();
    assert!(
        !records.iter().any(|record| record.contains("reconnected")),
        "a reconnect queued when the worker is gone is not served: {records:?}"
    );
}

/// The transport is cancelled before the consumer polls, with a disconnect and a connect
/// queued and their senders still alive: the cancel wins the biased select and the
/// consumer returns past the drain, so the stream ends with nothing on it and no event
/// leaves a record.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_state_consumer_cancelled_before_it_polls_serves_nothing_queued() {
    install_test_logger();
    let name = "shared-instance-state-consumer-cancelled-first";
    let read = shared_instance_state_drive_consumer_with(
        name,
        DriveSpec {
            queued: &[Queued::Disconnect, Queued::Connect],
            close_before_run: &[],
            cancel_before_run: true,
            exits: "the consumer returns on the cancel",
        },
    )
    .await;
    assert_eq!(read, [], "a cancel seen by the select serves nothing queued");
    let records = shared_instance_records_for(name);
    assert!(records.is_empty(), "no queued event is served after the cancel: {records:?}");
}

/// The attach writes the connected posture under the handler lock and only then sends
/// `Connecting`. Holding that lock from outside keeps the write pending: no event may
/// arrive while it is held; once released, `Connecting` arrives with the posture already
/// on. The port has no listener so the client itself contributes nothing to the stream.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_state_connecting_is_not_sent_before_the_connected_posture_is_set() {
    install_test_logger();
    let port = free_loopback_port().await;
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let transport = Transport::new(TransportConfig::new(
        "shared-instance-state-posture-first",
        &identity,
        false,
    ));
    let (events_tx, mut events) = tokio::sync::mpsc::channel(8);
    let options = SharedInstanceOptions::default()
        .with_reconnect_wait(SHARED_INSTANCE_RECONNECT_WAIT)
        .with_state_events(events_tx);
    let handler = transport.get_handler();
    let guard = handler.lock().await;

    let attach = transport.attach_shared_instance(
        SharedInstanceEndpoint::Tcp(SocketAddr::from(([127, 0, 0, 1], port))),
        options,
    );
    let watch = async {
        let early = timeout(SHARED_INSTANCE_STATE_QUIET, events.recv()).await;
        assert!(
            early.is_err(),
            "no event may arrive while the posture write is still pending: {early:?}"
        );
        assert!(
            !guard.config.connected_to_shared_instance,
            "the posture is still off while the handler lock is held from outside"
        );
        drop(guard);
        assert_eq!(
            shared_instance_state_next(&mut events).await,
            Some(SharedInstanceEvent::Connecting)
        );
        assert!(
            transport.connected_to_shared_instance().await,
            "a reader of `Connecting` finds the connected posture on"
        );
    };
    let (attached, ()) = tokio::join!(attach, watch);
    let iface = attached.expect("attach to the shared instance");
    assert_eq!(transport.shared_instance(), Some(iface), "the slot keeps the iface");
    assert!(
        timeout(SHARED_INSTANCE_STATE_QUIET, events.recv()).await.is_err(),
        "nothing more while nothing listens"
    );

    timeout(STOP_TIMEOUT, transport.stop()).await.expect("client stop completes");
}
