// Usage-pattern coverage of what `Transport::stop` publishes on `resource_events()`, written
// from the public contract: no routed drain follows a stop, so the receipt sweep publishes
// what the ResourceManager still holds in its queue first and appends the failures of the
// transfers it cancels for the receipts it sweeps, in the order of the periodic drains; the
// queued events are published, never routed, so a receipt whose transfer was dropped before
// the stop concludes `LinkClosed` like every other receipt the sweep reaches, not `Receive`.
// Helpers come from the sibling `tests_parts` files.

/// Every event the broadcast receiver holds right now, in order; a lagged receiver reads on.
fn sizes_probe16_drain_broadcast(
    events: &mut broadcast::Receiver<ResourceEvent>,
) -> Vec<ResourceEvent> {
    let mut seen = Vec::new();
    loop {
        match events.try_recv() {
            Ok(event) => seen.push(event),
            Err(broadcast::error::TryRecvError::Lagged(_)) => continue,
            Err(_) => break seen,
        }
    }
}

/// The position of the first `InboundFailed` on `link_id` whose reason is `reason`.
fn sizes_probe16_position_of_failure(
    seen: &[ResourceEvent],
    link_id: AddressHash,
    reason: &str,
) -> Option<usize> {
    seen.iter().position(|event| {
        event.link_id == link_id
            && matches!(
                &event.kind,
                ResourceEventKind::InboundFailed(failure) if failure.reason == reason
            )
    })
}

/// A receipt `Receiving` on a sibling link of the same connector has its inbound Resource
/// dropped, which queues a `link_closed` failure for that link, and a receipt on the pair's
/// link is `Receiving` a transfer that is still inbound when `stop` runs. The stop sweep
/// fails both receipts `LinkClosed` (the dropped one is not routed to `Receive`), and on
/// `resource_events()` the sibling's queued `link_closed` failure precedes the pair link's
/// `cancelled` failure; each is reported once and the sibling, whose receiver was already
/// gone, draws no `cancelled` of its own.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_sizes_probe16_stop_publishes_the_queued_failure_before_the_cancel_it_raises(
) {
    let mut pair = loopback_request_pair("sizes-probe16-stop-order").await;
    let link_id = pair.handles.link_id;
    register_handler(&pair, "/quiet", RequestAllow::All, silent_handler()).await;
    let (_sibling_link, sibling_link_id) = stop_open_sibling_link(&mut pair).await;
    let mut events = pair.handles.connector.transport.resource_events();

    let swept = sizes_probe14_receiving_receipt_on(&pair, link_id).await;
    let dropped = sizes_probe14_receiving_receipt_on(&pair, sibling_link_id).await;
    drop_resource_state(&pair.handles.connector.transport, sibling_link_id).await;
    assert_eq!(dropped.status(), RequestStatus::Receiving, "the drop queues, it does not route");

    pair.handles.connector.transport.stop().await;

    assert_eq!(swept.status(), RequestStatus::Failed, "the sweep failed the swept receipt");
    assert_eq!(swept.response().await, Err(RequestError::LinkClosed));
    assert_eq!(dropped.status(), RequestStatus::Failed, "the sweep failed the dropped receipt");
    assert_eq!(
        dropped.response().await,
        Err(RequestError::LinkClosed),
        "the queued failure is published, not routed: the sweep's own failure stands"
    );

    let seen = sizes_probe16_drain_broadcast(&mut events);
    let queued = sizes_probe16_position_of_failure(&seen, sibling_link_id, "link_closed")
        .expect("the queued link_closed failure is published inside stop");
    let cancelled = sizes_probe16_position_of_failure(&seen, link_id, "cancelled")
        .expect("the swept receipt's transfer is reported cancelled inside stop");
    assert!(queued < cancelled, "the queue goes out before the cancels the sweep raises: {seen:?}");
    let inbound_failures = seen
        .iter()
        .filter(|event| matches!(event.kind, ResourceEventKind::InboundFailed(_)))
        .count();
    assert_eq!(inbound_failures, 2, "each transfer is reported once: {seen:?}");
    assert!(
        sizes_probe16_position_of_failure(&seen, sibling_link_id, "cancelled").is_none(),
        "a receiver already dropped has nothing left to cancel: {seen:?}"
    );
}
