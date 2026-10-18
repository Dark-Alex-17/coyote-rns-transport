// Usage-pattern coverage of the public `Link::expect_response` called over a receipt-less
// record that is receiving its response Resource, written from its rustdoc contract: the
// record is replaced, the transfer it was receiving is cancelled on the link's next inbound
// packet, Resource packets included (one `ResourceReceiverCancel` to the responder,
// `RNS/Link.py:1435`), and the new record carries the limit the call keeps or sets. Both
// tests make that next packet a response advertisement, so the cancel is pinned to the
// Resource path and not to a keepalive or an echo arriving later. The same fixture reaches
// the wire half of a refused later advertisement under a receiving receipt-less record: the
// refusal cancels the partial assembly as well as the advertised transfer. Helpers come
// from the sibling `tests_parts` files.

/// Waits, within the loopback budget, for the connector to drop the inbound Resource
/// `hash` on the pair's link. A `Complete` naming `id` on the way is a response the link
/// no longer solicits and fails the test.
#[deny(clippy::await_holding_invalid_type)]
async fn sizes_probe8_await_inbound_failed(
    events: &mut broadcast::Receiver<ResourceEvent>,
    link_id: AddressHash,
    id: RequestId,
    hash: Hash,
) {
    let failed = timeout(LOOPBACK_TIMEOUT, async {
        loop {
            match events.recv().await {
                Ok(event) if event.link_id == link_id => match &event.kind {
                    ResourceEventKind::InboundFailed(_) => return event,
                    ResourceEventKind::Complete(complete) => assert!(
                        complete.request_id.as_deref() != Some(id.as_bytes().as_slice()),
                        "a response the link no longer solicits was assembled under {id}"
                    ),
                    _ => {}
                },
                Ok(_) => {}
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => panic!("resource event channel closed"),
            }
        }
    })
    .await
    .expect("the displaced transfer is dropped on the link's next inbound packet");
    assert_eq!(failed.hash, hash, "the transfer the displaced record was receiving is dropped");
}

/// A receipt-less record under `SIZES_TIGHT_LIMIT` for `/quiet`, moved to receiving by an
/// unbacked response advertisement of `d = SIZES_TIGHT_LIMIT / 2`. Returns the id and the
/// advertised hash once the connector has asked the responder for parts.
#[deny(clippy::await_holding_invalid_type)]
async fn sizes_probe8_receiving_record(
    pair: &LoopbackRequestPair,
    listener_tap: &mut broadcast::Receiver<RxMessage>,
) -> (RequestId, Hash) {
    let id =
        sizes_probe2_send_limited_request_packet(pair, "/quiet", Some(SIZES_TIGHT_LIMIT)).await;
    let hash = sizes_probe8_admit_advertisement(pair, listener_tap, id).await;
    (id, hash)
}

/// Advertises an unbacked response of `d = SIZES_TIGHT_LIMIT / 2` under `id` and waits
/// for the connector's part request, which it sends only once the advertisement is
/// admitted and the record marked receiving. Returns the advertised hash.
#[deny(clippy::await_holding_invalid_type)]
async fn sizes_probe8_admit_advertisement(
    pair: &LoopbackRequestPair,
    listener_tap: &mut broadcast::Receiver<RxMessage>,
    id: RequestId,
) -> Hash {
    let link_id = pair.handles.link_id;
    // Part requests already on the tap belong to an earlier receiver.
    let _ = count_link_packets(listener_tap, link_id, PacketContext::ResourceRequest);
    let advertisement =
        sizes_cancel_unbacked_response_advertisement(id, (SIZES_TIGHT_LIMIT / 2) as u64);
    inject_advertisement_from_listener(pair, &advertisement).await;
    await_link_packets(listener_tap, link_id, PacketContext::ResourceRequest, 1).await;
    advertisement.hash
}

/// No keepalive and no response packet reached the connector's link since `connector_tap`
/// was taken: the cancel just seen went out on a Resource packet.
fn sizes_probe8_assert_no_non_resource_packet_arrived(
    connector_tap: &mut broadcast::Receiver<RxMessage>,
    link_id: AddressHash,
) {
    assert_eq!(
        count_link_packets(connector_tap, link_id, PacketContext::KeepAlive),
        0,
        "the displaced transfer is cancelled on a Resource packet, before any keepalive"
    );
    assert_eq!(
        count_link_packets(connector_tap, link_id, PacketContext::Response),
        0,
        "the displaced transfer is cancelled on a Resource packet, before any response"
    );
}

/// `expect_response(id, None)` over a receiving record: the transfer it was receiving is
/// cancelled on the link's next inbound packet, here the next response advertisement, with
/// one receiver cancel and before any non-Resource packet reaches the link. The record
/// that replaced it still carries `SIZES_TIGHT_LIMIT`: that advertisement, at `d` under
/// the limit, is admitted, and a later one at `d` over it refused. The refusal, arriving
/// while the record is receiving again, cancels that partial assembly too, which is what
/// shows the first was admitted: a second receiver cancel for it and a third for the
/// refused advertisement. The link still answers.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_sizes_probe8_expect_response_over_a_receiving_record_cancels_it_and_keeps_the_limit(
) {
    let pair = loopback_request_pair("sizes-probe8-expect-over-receiving-keeps").await;
    let mut listener_tap = pair.handles.listener.transport.iface_rx();
    let link_id = pair.handles.link_id;
    register_handler(&pair, "/quiet", RequestAllow::All, silent_handler()).await;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;
    let mut events = pair.handles.connector.transport.resource_events();

    let (id, displaced_hash) = sizes_probe8_receiving_record(&pair, &mut listener_tap).await;
    let mut connector_tap = pair.handles.connector.transport.iface_rx();
    pair.handles.link.lock().await.expect_response(id.as_bytes(), None);

    // The next inbound packet on the link is this advertisement, admitted under the kept
    // limit. The displaced transfer's cancel leaves the connector ahead of the part request
    // for it, and the connector publishes the failure only once both are sent, so the
    // failure is the mark that the advertisement has been handled.
    let receiving =
        sizes_cancel_unbacked_response_advertisement(id, (SIZES_TIGHT_LIMIT / 2) as u64);
    inject_advertisement_from_listener(&pair, &receiving).await;
    sizes_probe8_await_inbound_failed(&mut events, link_id, id, displaced_hash).await;
    await_link_packets(&mut listener_tap, link_id, PacketContext::ResourceReceiverCancel, 1).await;
    sizes_probe8_assert_no_non_resource_packet_arrived(&mut connector_tap, link_id);

    let over = sizes_cancel_unbacked_response_advertisement(id, (SIZES_TIGHT_LIMIT + 64) as u64);
    inject_advertisement_from_listener(&pair, &over).await;
    sizes_probe8_await_inbound_failed(&mut events, link_id, id, receiving.hash).await;
    await_link_packets(&mut listener_tap, link_id, PacketContext::ResourceReceiverCancel, 2).await;

    sizes_assert_link_still_answers(&pair).await;
    assert_eq!(
        count_link_packets(&mut listener_tap, link_id, PacketContext::ResourceReceiverCancel),
        0,
        "no further receiver cancel leaves the connector"
    );
}

/// `expect_response(id, Some(limit))` over a receiving record: the transfer it was
/// receiving is cancelled on the link's next inbound packet, here a response advertisement
/// the kept limit would have admitted, before any non-Resource packet reaches the link. The
/// record that replaced it carries the new limit, so that advertisement is refused with a
/// receiver cancel of its own and no part request, and nothing is assembled or dropped
/// under the id afterwards. The link still answers.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_sizes_probe8_expect_response_over_a_receiving_record_cancels_it_and_sets_the_limit(
) {
    let pair = loopback_request_pair("sizes-probe8-expect-over-receiving-sets").await;
    let mut listener_tap = pair.handles.listener.transport.iface_rx();
    let link_id = pair.handles.link_id;
    register_handler(&pair, "/quiet", RequestAllow::All, silent_handler()).await;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;
    let mut events = pair.handles.connector.transport.resource_events();

    let (id, displaced_hash) = sizes_probe8_receiving_record(&pair, &mut listener_tap).await;
    let mut connector_tap = pair.handles.connector.transport.iface_rx();
    pair.handles.link.lock().await.expect_response(id.as_bytes(), Some(SIZES_TIGHT_LIMIT / 4));

    // Under the kept limit this advertisement was admitted above; under the set one it
    // is over `d` and refused at its advertisement, which is also the next inbound packet
    // the displaced transfer's cancel waits for: one receiver cancel each.
    let refused = sizes_cancel_unbacked_response_advertisement(id, (SIZES_TIGHT_LIMIT / 2) as u64);
    inject_advertisement_from_listener(&pair, &refused).await;
    sizes_probe8_await_inbound_failed(&mut events, link_id, id, displaced_hash).await;
    await_link_packets(&mut listener_tap, link_id, PacketContext::ResourceReceiverCancel, 2).await;
    sizes_probe8_assert_no_non_resource_packet_arrived(&mut connector_tap, link_id);

    // Every part request on the tap so far was for the displaced transfer, now cancelled.
    sizes_assert_no_response_activity(&mut events, link_id, id).await;
    assert_eq!(
        count_link_packets(&mut listener_tap, link_id, PacketContext::ResourceRequest),
        0,
        "the refused advertisement is not asked for parts"
    );

    sizes_assert_link_still_answers(&pair).await;
}
