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
    let read = shared_instance_state_drive_consumer_with(
        name,
        DriveSpec {
            queued: &[Queued::Connect],
            close_before_run: &[Queued::Reconnect, Queued::Connect, Queued::Disconnect],
            cancel_before_run: false,
            exits: "the consumer exits once the channels are closed",
        },
    )
    .await;
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
    let read = shared_instance_state_drive_consumer_with(
        name,
        DriveSpec {
            queued: &[Queued::Connect],
            close_before_run: &[Queued::Reconnect, Queued::Connect],
            cancel_before_run: false,
            exits: "the consumer exits once the connect channel is drained and closed",
        },
    )
    .await;
    assert_eq!(
        read,
        [SharedInstanceEvent::Connected],
        "the queued connect is read while the disconnect channel is still open"
    );
}
