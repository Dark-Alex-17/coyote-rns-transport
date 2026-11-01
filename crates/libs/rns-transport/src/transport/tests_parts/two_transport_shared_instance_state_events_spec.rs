// The hand-driven shared-instance consumer: what can be queued on its channels, how one
// drive is specified, and the driver that runs it and reads the state stream to its end.
// Used by the state-event, dial-failure and disconnect-reason pins included into this
// module.

/// One event queued on a hand-built consumer's channels.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Queued {
    Disconnect,
    /// A disconnect carrying this reason; `Disconnect` is `PeerClosed`.
    DisconnectWith(DisconnectReason),
    Connect,
    Reconnect,
    DialFailure(DialFailureKind),
}

/// How a hand-built consumer is driven: the events queued, in order, before it first polls;
/// the senders already gone by then (the rest go after it returns); whether the consumer's
/// cancel token is cancelled before the poll; whether it is handed a first-connect notifier
/// whose receiver is already gone; whether it sends `DisconnectedWithReason` in place of
/// `Disconnected`; and what the consumer's exit within `STOP_TIMEOUT` pins.
struct DriveSpec<'a> {
    queued: &'a [Queued],
    close_before_run: &'a [Queued],
    cancel_before_run: bool,
    first_connect_abandoned: bool,
    disconnect_reasons: bool,
    exits: &'a str,
}

/// Drives a hand-built consumer named `name` as `spec` says and returns what the state
/// stream read until it ended; the consumer's return drops the sender. Each event channel
/// holds what is queued on it, so the queueing never blocks. A queued disconnect carries
/// `PeerClosed` unless it names its reason.
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_state_drive_consumer_with(
    name: &str,
    spec: DriveSpec<'_>,
) -> Vec<SharedInstanceEvent> {
    let iface = AddressHash::new_from_rand(OsRng);
    let capacity = |kind| spec.queued.iter().filter(|queued| **queued == kind).count().max(1);
    let (reconnects_tx, reconnects) = tokio::sync::mpsc::channel(capacity(Queued::Reconnect));
    let (connects_tx, connects) = tokio::sync::mpsc::channel(capacity(Queued::Connect));
    let disconnect_count = spec
        .queued
        .iter()
        .filter(|queued| matches!(queued, Queued::Disconnect | Queued::DisconnectWith(_)))
        .count();
    let (disconnects_tx, disconnects) = tokio::sync::mpsc::channel(disconnect_count.max(1));
    let dial_failure_count =
        spec.queued.iter().filter(|queued| matches!(queued, Queued::DialFailure(_))).count();
    let (dial_failures_tx, dial_failures) = tokio::sync::mpsc::channel(dial_failure_count.max(1));
    let (state_tx, mut state) = tokio::sync::mpsc::channel(8);
    for kind in spec.queued {
        match kind {
            Queued::Disconnect => {
                disconnects_tx
                    .send((iface, DisconnectReason::PeerClosed))
                    .await
                    .expect("queue the disconnect");
            }
            Queued::DisconnectWith(reason) => {
                disconnects_tx.send((iface, *reason)).await.expect("queue the disconnect");
            }
            Queued::Connect => connects_tx.send(iface).await.expect("queue the connect"),
            Queued::Reconnect => reconnects_tx.send(iface).await.expect("queue the reconnect"),
            Queued::DialFailure(reason) => {
                dial_failures_tx
                    .send(DialFailure::new(iface, *reason))
                    .await
                    .expect("queue the dial failure");
            }
        }
    }
    let mut open = vec![(Queued::Reconnect, reconnects_tx), (Queued::Connect, connects_tx)];
    open.retain(|(kind, _)| !spec.close_before_run.contains(kind));
    let disconnects_tx = (!spec
        .close_before_run
        .iter()
        .any(|queued| matches!(queued, Queued::Disconnect | Queued::DisconnectWith(_))))
    .then_some(disconnects_tx);
    let dial_failures_tx =
        (!spec.close_before_run.iter().any(|queued| matches!(queued, Queued::DialFailure(_))))
            .then_some(dial_failures_tx);

    let (_transport, mut consumer) = shared_instance_reconnect_consumer(name, iface);
    consumer.disconnect_reasons = spec.disconnect_reasons;
    let cancel = consumer.cancel.clone();
    if spec.cancel_before_run {
        cancel.cancel();
    }
    let first_connect = spec.first_connect_abandoned.then(|| {
        let (notify, waiting) = tokio::sync::oneshot::channel();
        drop(waiting);
        notify
    });
    timeout(
        STOP_TIMEOUT,
        consumer.run(
            reconnects,
            connects,
            disconnects,
            dial_failures,
            Some(state_tx),
            first_connect,
        ),
    )
    .await
    .expect(spec.exits);
    if !spec.cancel_before_run {
        cancel.cancel();
    }
    drop(open);
    drop(disconnects_tx);
    drop(dial_failures_tx);

    shared_instance_state_read_to_end(&mut state).await
}
