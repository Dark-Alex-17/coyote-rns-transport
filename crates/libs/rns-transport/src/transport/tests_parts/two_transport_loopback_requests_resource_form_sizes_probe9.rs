// Usage-pattern coverage of where the cancel owed to a displaced response Resource leaves
// the link, written from the public contract: a record failed or replaced while its
// response Resource is inbound owes that Resource one receiver cancel (`RNS/Link.py:1435`,
// `RNS/Resource.py:1111-1116`), sent on the link's next inbound packet, Resource packets
// included. An over-limit packet response is itself that packet, so its cancel leaves at
// once. `Transport::send_request_resource*` under an id already receiving replaces the
// record locally and cancels the displaced transfer in the call, before the request Resource
// is advertised, so no keepalive or response reaches the link first; the record that
// replaced it keeps the limit `None` leaves alone and takes the one `Some` sets.
// `Link::expect_response` owes its displaced transfer the cancel until the
// next inbound packet, here the responder's own receiver cancel refusing a request Resource
// over its `max_request_size` (`RNS/Link.py:1036-1046`). Helpers come from the sibling
// `tests_parts` files.

/// A `/quiet` receipt under `SIZES_TIGHT_LIMIT` moved to `Receiving` by an unbacked
/// response advertisement of `d = SIZES_TIGHT_LIMIT / 2`, with the advertised hash.
#[deny(clippy::await_holding_invalid_type)]
async fn sizes_probe9_receiving_receipt(
    pair: &LoopbackRequestPair,
) -> (RequestReceipt, crate::hash::Hash) {
    let receipt = request_with(
        pair,
        "/quiet",
        rmpv::Value::Nil,
        RequestOptions::default().with_max_response_size(SIZES_TIGHT_LIMIT),
    )
    .await;
    let mut status_rx = receipt.subscribe();
    let advertisement =
        sizes_cancel_unbacked_response_advertisement(receipt.request_id(), SIZES_CANCEL_ADMITTED_D);
    inject_advertisement_from_listener(pair, &advertisement).await;
    await_status(&mut status_rx, RequestStatus::Receiving, RECEIPT_TIMEOUT).await;
    (receipt, advertisement.hash)
}

/// Waits for the connector to drop the inbound Resource `hash` on the pair's link as
/// `cancelled`, then checks that no keepalive and no response packet reached the
/// connector's link since `connector_tap` was taken, and that one receiver cancel for it
/// reached the responder.
#[deny(clippy::await_holding_invalid_type)]
async fn sizes_probe9_assert_displaced_transfer_cancelled(
    pair: &LoopbackRequestPair,
    events: &mut broadcast::Receiver<ResourceEvent>,
    connector_tap: &mut broadcast::Receiver<RxMessage>,
    listener_tap: &mut broadcast::Receiver<RxMessage>,
    hash: crate::hash::Hash,
) {
    let link_id = pair.handles.link_id;
    let failed = timeout(LOOPBACK_TIMEOUT, async {
        loop {
            match events.recv().await {
                Ok(event)
                    if event.link_id == link_id
                        && matches!(event.kind, ResourceEventKind::InboundFailed(_)) =>
                {
                    return event
                }
                Ok(_) => {}
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => panic!("resource event channel closed"),
            }
        }
    })
    .await
    .expect("the displaced response Resource is dropped on the link's next inbound packet");
    sizes_probe8_assert_no_non_resource_packet_arrived(connector_tap, link_id);
    assert_eq!(failed.hash, hash, "the transfer the displaced record was receiving is dropped");
    match &failed.kind {
        ResourceEventKind::InboundFailed(failure) => {
            assert_eq!(failure.reason, "cancelled", "a local cancel, not a retry exhaustion")
        }
        kind => panic!("not an inbound failure: {kind:?}"),
    }
    await_link_packets(listener_tap, link_id, PacketContext::ResourceReceiverCancel, 1).await;
}

/// Sends `/quiet` with an over-MDU body as a request Resource under `id`, which the link
/// is already receiving a response for, with `limit` for the response. The responder
/// answers the advertisement with a part request, the first packet back on the link.
#[deny(clippy::await_holding_invalid_type)]
async fn sizes_probe9_send_request_resource_under(
    pair: &LoopbackRequestPair,
    id: RequestId,
    limit: Option<usize>,
) {
    let body = over_mdu_body(pair).await;
    let packed =
        Link::request_payload("/quiet", rmpv::Value::Binary(body)).expect("request packs").packed;
    pair.handles
        .connector
        .transport
        .send_request_resource_with_max_response_size(
            &pair.handles.link_id,
            id.to_bytes().to_vec(),
            packed,
            None,
            limit,
        )
        .await
        .expect("request resource dispatched");
}

/// An over-limit packet response for a receipt already `Receiving` is itself the inbound
/// packet its cancel leaves on: the receiver cancel reaches the responder and the
/// transfer is dropped before anything but that response packet reached the connector's
/// link.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_sizes_probe9_oversize_packet_cancel_leaves_on_that_packet(
) {
    let pair = loopback_request_pair("sizes-probe9-oversize-cancel-at-once").await;
    let mut listener_tap = pair.handles.listener.transport.iface_rx();
    let link_id = pair.handles.link_id;
    register_handler(&pair, "/quiet", RequestAllow::All, silent_handler()).await;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;
    let mut events = pair.handles.connector.transport.resource_events();

    let (receipt, hash) = sizes_probe9_receiving_receipt(&pair).await;
    let mut connector_tap = pair.handles.connector.transport.iface_rx();
    sizes_cancel_send_response_packet_from_listener(
        &pair,
        &sizes_cancel_probe_oversize_envelope(receipt.request_id()),
    )
    .await;
    assert_eq!(await_receipt(&receipt).await, Err(RequestError::Oversize));

    let failed = await_resource_event(&mut events, |event| {
        event.link_id == link_id && matches!(event.kind, ResourceEventKind::InboundFailed(_))
    })
    .await;
    assert_eq!(failed.hash, hash, "the transfer the failed receipt was receiving is dropped");
    await_link_packets(&mut listener_tap, link_id, PacketContext::ResourceReceiverCancel, 1).await;
    assert_eq!(
        count_link_packets(&mut connector_tap, link_id, PacketContext::Response),
        1,
        "the over-limit response is the only response packet that reached the link"
    );
    assert_eq!(
        count_link_packets(&mut connector_tap, link_id, PacketContext::KeepAlive),
        0,
        "the cancel did not wait for a keepalive"
    );
    sizes_assert_link_still_answers(&pair).await;
    assert_eq!(
        count_link_packets(&mut listener_tap, link_id, PacketContext::ResourceReceiverCancel),
        0,
        "no further receiver cancel leaves the connector"
    );
}

/// `send_request_resource` under an id whose receipt-less record is receiving: the record
/// is replaced, the transfer it was receiving is cancelled in the call, before the request
/// Resource is advertised and before any keepalive or response reaches the link, and the
/// replacing record keeps `SIZES_TIGHT_LIMIT`: a later advertisement under it is admitted
/// and asked for parts, and one over it then refused, which fails the record and cancels
/// that partial assembly too, with a receiver cancel for each.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_sizes_probe9_request_resource_over_a_receiving_record_cancels_it_and_keeps_the_limit(
) {
    let pair = loopback_request_pair("sizes-probe9-request-resource-keeps").await;
    let mut listener_tap = pair.handles.listener.transport.iface_rx();
    let link_id = pair.handles.link_id;
    register_handler(&pair, "/quiet", RequestAllow::All, silent_handler()).await;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;
    let mut events = pair.handles.connector.transport.resource_events();

    let (id, displaced_hash) = sizes_probe8_receiving_record(&pair, &mut listener_tap).await;
    let mut connector_tap = pair.handles.connector.transport.iface_rx();
    sizes_probe9_send_request_resource_under(&pair, id, None).await;
    sizes_probe9_assert_displaced_transfer_cancelled(
        &pair,
        &mut events,
        &mut connector_tap,
        &mut listener_tap,
        displaced_hash,
    )
    .await;

    // Part requests on the tap so far were for the displaced transfer, now cancelled.
    let _ = count_link_packets(&mut listener_tap, link_id, PacketContext::ResourceRequest);
    let under = sizes_cancel_unbacked_response_advertisement(id, SIZES_CANCEL_ADMITTED_D);
    inject_advertisement_from_listener(&pair, &under).await;
    await_link_packets(&mut listener_tap, link_id, PacketContext::ResourceRequest, 1).await;

    let over = sizes_cancel_unbacked_response_advertisement(id, SIZES_OVER_D);
    inject_advertisement_from_listener(&pair, &over).await;
    sizes_probe8_await_inbound_failed(&mut events, link_id, id, under.hash).await;
    await_link_packets(&mut listener_tap, link_id, PacketContext::ResourceReceiverCancel, 2).await;
    sizes_assert_link_still_answers(&pair).await;
    assert_eq!(
        count_link_packets(&mut listener_tap, link_id, PacketContext::ResourceReceiverCancel),
        0,
        "no further receiver cancel leaves the connector"
    );
}

/// `send_request_resource_with_max_response_size(.., Some(limit))` under an id whose
/// receipt-less record is receiving: the displaced transfer is cancelled in the call as
/// above, and the replacing record carries the new limit, so an advertisement the kept
/// limit admitted is now refused with a receiver cancel and no part request.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_sizes_probe9_request_resource_over_a_receiving_record_cancels_it_and_sets_the_limit(
) {
    let pair = loopback_request_pair("sizes-probe9-request-resource-sets").await;
    let mut listener_tap = pair.handles.listener.transport.iface_rx();
    let link_id = pair.handles.link_id;
    register_handler(&pair, "/quiet", RequestAllow::All, silent_handler()).await;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;
    let mut events = pair.handles.connector.transport.resource_events();

    let (id, displaced_hash) = sizes_probe8_receiving_record(&pair, &mut listener_tap).await;
    let mut connector_tap = pair.handles.connector.transport.iface_rx();
    sizes_probe9_send_request_resource_under(&pair, id, Some(SIZES_TIGHT_LIMIT / 4)).await;
    sizes_probe9_assert_displaced_transfer_cancelled(
        &pair,
        &mut events,
        &mut connector_tap,
        &mut listener_tap,
        displaced_hash,
    )
    .await;

    let _ = count_link_packets(&mut listener_tap, link_id, PacketContext::ResourceRequest);
    let refused = sizes_cancel_unbacked_response_advertisement(id, SIZES_CANCEL_ADMITTED_D);
    inject_advertisement_from_listener(&pair, &refused).await;
    await_link_packets(&mut listener_tap, link_id, PacketContext::ResourceReceiverCancel, 1).await;
    sizes_assert_no_response_activity(&mut events, link_id, id).await;
    assert_eq!(
        count_link_packets(&mut listener_tap, link_id, PacketContext::ResourceRequest),
        0,
        "the refused advertisement is not asked for parts"
    );
    sizes_assert_link_still_answers(&pair).await;
    assert_eq!(
        count_link_packets(&mut listener_tap, link_id, PacketContext::ResourceReceiverCancel),
        0,
        "no further receiver cancel leaves the connector"
    );
}

/// `expect_response(id, None)` over a receiving receipt-less record, then a request
/// Resource over the responder's `max_request_size`: the first packet back is the
/// responder's receiver cancel refusing it, and the displaced transfer's own cancel leaves
/// on that packet, before any keepalive or response reaches the link. The refused request
/// is reported `OutboundRejected` and the link still answers.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_sizes_probe9_expect_response_cancel_leaves_on_the_responders_refusal(
) {
    let pair = loopback_request_pair("sizes-probe9-expect-cancel-on-refusal").await;
    let mut listener_tap = pair.handles.listener.transport.iface_rx();
    pair.handles
        .destination
        .lock()
        .await
        .set_max_request_size(RESOURCE_FORM_REQUEST_LIMIT)
        .expect("limit is set");
    register_handler(&pair, "/quiet", RequestAllow::All, silent_handler()).await;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;
    let mut events = pair.handles.connector.transport.resource_events();

    let (id, displaced_hash) = sizes_probe8_receiving_record(&pair, &mut listener_tap).await;
    let mut connector_tap = pair.handles.connector.transport.iface_rx();
    pair.handles.link.lock().await.expect_response(id.as_bytes(), None);
    // Read off its own tap so the order against the drained cancel's failure does not matter.
    let mut outbound_events = pair.handles.connector.transport.resource_events();

    let body = vec![0u8; OVERSIZE_COMPRESSIBLE_LEN];
    let (refused_id, refused_hash) =
        send_request_as_resource(&pair, "/quiet", rmpv::Value::Binary(body)).await;
    assert_ne!(refused_id, id, "the refused request is not the displaced one");
    sizes_probe9_assert_displaced_transfer_cancelled(
        &pair,
        &mut events,
        &mut connector_tap,
        &mut listener_tap,
        displaced_hash,
    )
    .await;
    let outcome = await_resource_event(&mut outbound_events, |event| {
        event.hash == refused_hash
            && matches!(
                event.kind,
                ResourceEventKind::OutboundRejected { .. }
                    | ResourceEventKind::OutboundComplete
                    | ResourceEventKind::OutboundFailed
            )
    })
    .await;
    assert!(
        matches!(outcome.kind, ResourceEventKind::OutboundRejected { .. }),
        "the oversize request is rejected, got {:?}",
        outcome.kind
    );
    sizes_assert_link_still_answers(&pair).await;
    assert_eq!(
        count_link_packets(
            &mut listener_tap,
            pair.handles.link_id,
            PacketContext::ResourceReceiverCancel
        ),
        0,
        "no further receiver cancel leaves the connector"
    );
}
