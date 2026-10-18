// Usage-pattern coverage of the displace-and-retry cycle repeated and re-limited, written
// from the public contract: `Link::expect_response` over a record receiving a response
// Resource replaces the record, owes that transfer one receiver cancel on the link's next
// inbound packet (`RNS/Link.py:1435`), and the record that replaced it keeps the limit
// `None` leaves alone or takes the one `Some` sets (`RNS/Link.py:1046-1049`). The next
// inbound packet is the responder's retry of the very advertisement the displaced record
// was receiving (`RNS/Resource.py:594-598`), which must be admitted under the fresh record
// on its own merits. The first test runs that cycle twice on one id: nothing from the first
// cycle leaks into the second. The second widens the limit while displacing, and the
// re-admitted transfer is then measured against the wider limit, not the one it was
// admitted under before. Helpers come from the sibling `tests_parts` files.

/// Waits, within the loopback budget, for the connector to drop the inbound Resource
/// `hash` on the pair's link as `cancelled`. Failures of other transfers on the way are
/// skipped: an orphaned receiver running out its part-request retries is not the drop
/// under test.
#[deny(clippy::await_holding_invalid_type)]
async fn sizes_probe11_await_cancelled(
    events: &mut broadcast::Receiver<ResourceEvent>,
    link_id: AddressHash,
    hash: Hash,
) {
    timeout(LOOPBACK_TIMEOUT, async {
        loop {
            match events.recv().await {
                Ok(event) if event.link_id == link_id && event.hash == hash => {
                    if let ResourceEventKind::InboundFailed(failure) = &event.kind {
                        assert_eq!(failure.reason, "cancelled", "a local cancel");
                        return;
                    }
                }
                Ok(_) => {}
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => panic!("resource event channel closed"),
            }
        }
    })
    .await
    .expect("the transfer the record was receiving is cancelled");
}

/// One displace-and-retry cycle on `id`: `expect_response(id, limit)` over the record
/// receiving `advertisement`, then the responder's retry of that same advertisement as the
/// link's next inbound packet. The displaced receiver is cancelled on that packet, once,
/// before any keepalive or response reaches the connector's link, and the retry is admitted
/// under the fresh record: the connector asks for its parts again.
#[deny(clippy::await_holding_invalid_type)]
async fn sizes_probe11_displace_and_retry(
    pair: &LoopbackRequestPair,
    events: &mut broadcast::Receiver<ResourceEvent>,
    listener_tap: &mut broadcast::Receiver<RxMessage>,
    id: RequestId,
    limit: Option<usize>,
    advertisement: &ResourceAdvertisement,
) {
    let link_id = pair.handles.link_id;
    let mut part_request_tap = pair.handles.listener.transport.iface_rx();
    let mut connector_tap = pair.handles.connector.transport.iface_rx();
    pair.handles.link.lock().await.expect_response(id.as_bytes(), limit);

    inject_advertisement_from_listener(pair, advertisement).await;
    sizes_probe9_assert_displaced_transfer_cancelled(
        pair,
        events,
        &mut connector_tap,
        listener_tap,
        advertisement.hash,
    )
    .await;
    await_link_packets(&mut part_request_tap, link_id, PacketContext::ResourceRequest, 1).await;
}

/// Two displace-and-retry cycles in a row on one id, both with `expect_response(id, None)`:
/// each cancels exactly the receiver it displaced and each re-admits the retry, so the
/// second cycle is indistinguishable from the first. After both, the record still carries
/// `SIZES_TIGHT_LIMIT` and is receiving the twice re-admitted transfer: an advertisement
/// over the limit is refused and cancels that partial assembly. The link still answers.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_sizes_probe11_a_second_displace_and_retry_cycle_behaves_like_the_first(
) {
    let pair = loopback_request_pair("sizes-probe11-two-cycles").await;
    let mut listener_tap = pair.handles.listener.transport.iface_rx();
    let link_id = pair.handles.link_id;
    register_handler(&pair, "/quiet", RequestAllow::All, silent_handler()).await;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;
    let mut events = pair.handles.connector.transport.resource_events();

    let id =
        sizes_probe2_send_limited_request_packet(&pair, "/quiet", Some(SIZES_TIGHT_LIMIT)).await;
    let advertisement = sizes_cancel_unbacked_response_advertisement(id, SIZES_CANCEL_ADMITTED_D);
    inject_advertisement_from_listener(&pair, &advertisement).await;
    await_link_packets(&mut listener_tap, link_id, PacketContext::ResourceRequest, 1).await;

    for cycle in 1..=2 {
        sizes_probe11_displace_and_retry(
            &pair,
            &mut events,
            &mut listener_tap,
            id,
            None,
            &advertisement,
        )
        .await;
        assert_eq!(
            count_link_packets(&mut listener_tap, link_id, PacketContext::ResourceReceiverCancel),
            0,
            "cycle {cycle}: exactly one receiver cancel per displaced receiver"
        );
    }

    let over = sizes_cancel_unbacked_response_advertisement(id, SIZES_OVER_D);
    inject_advertisement_from_listener(&pair, &over).await;
    await_link_packets(&mut listener_tap, link_id, PacketContext::ResourceReceiverCancel, 1).await;
    sizes_probe11_await_cancelled(&mut events, link_id, advertisement.hash).await;

    sizes_assert_link_still_answers(&pair).await;
}

/// `expect_response(id, Some(4 * SIZES_TIGHT_LIMIT))` over a record receiving under
/// `SIZES_TIGHT_LIMIT`, then the retry of the displaced advertisement: cancelled and
/// re-admitted as before. The fresh record carries the wider limit, so an advertisement
/// under `id` over the old limit and under the new one is admitted (the connector asks for
/// its parts, no receiver cancel leaves), and only one over the new limit is refused,
/// cancelling the partial assembly the record was receiving by then. The link still answers.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_sizes_probe11_a_wider_limit_set_while_displacing_governs_the_readmitted_record(
) {
    let pair = loopback_request_pair("sizes-probe11-wider-limit").await;
    let mut listener_tap = pair.handles.listener.transport.iface_rx();
    let link_id = pair.handles.link_id;
    register_handler(&pair, "/quiet", RequestAllow::All, silent_handler()).await;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;
    let mut events = pair.handles.connector.transport.resource_events();

    let id =
        sizes_probe2_send_limited_request_packet(&pair, "/quiet", Some(SIZES_TIGHT_LIMIT)).await;
    let advertisement = sizes_cancel_unbacked_response_advertisement(id, SIZES_CANCEL_ADMITTED_D);
    inject_advertisement_from_listener(&pair, &advertisement).await;
    await_link_packets(&mut listener_tap, link_id, PacketContext::ResourceRequest, 1).await;

    let wider = 4 * SIZES_TIGHT_LIMIT;
    sizes_probe11_displace_and_retry(
        &pair,
        &mut events,
        &mut listener_tap,
        id,
        Some(wider),
        &advertisement,
    )
    .await;

    // Over the old limit, under the new one: admitted, and the record now receives it. The
    // retry's own part request is still on the shared tap; only new ones count, and a new
    // one alone could be a retry of an earlier receiver: the hash the refusal below cancels
    // is what shows this advertisement was admitted under the record.
    let _ = count_link_packets(&mut listener_tap, link_id, PacketContext::ResourceRequest);
    let between = sizes_cancel_unbacked_response_advertisement(id, SIZES_OVER_D);
    inject_advertisement_from_listener(&pair, &between).await;
    await_link_packets(&mut listener_tap, link_id, PacketContext::ResourceRequest, 1).await;
    assert_eq!(
        count_link_packets(&mut listener_tap, link_id, PacketContext::ResourceReceiverCancel),
        0,
        "an advertisement under the wider limit is not refused"
    );

    // Over the new limit: refused, and the assembly the record was receiving is cancelled.
    let over = sizes_cancel_unbacked_response_advertisement(id, (wider + 64) as u64);
    inject_advertisement_from_listener(&pair, &over).await;
    await_link_packets(&mut listener_tap, link_id, PacketContext::ResourceReceiverCancel, 1).await;
    sizes_probe11_await_cancelled(&mut events, link_id, between.hash).await;

    sizes_assert_link_still_answers(&pair).await;
    assert_eq!(
        count_link_packets(&mut listener_tap, link_id, PacketContext::ResourceReceiverCancel),
        0,
        "no further receiver cancel leaves the connector"
    );
}
