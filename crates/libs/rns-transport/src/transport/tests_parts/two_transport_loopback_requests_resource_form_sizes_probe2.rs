// Usage-pattern coverage of the solicited-response marker from the consumer's side, written
// from the CHANGELOG contract: a request packet sent through `Transport::send_packet*` or the
// bound-iface link send solicits its response (`send_prepared_packet_broadcast_with_trace` does
// not: it hands the packet to the interface manager and never passes `TransportHandler::send`,
// where the record is made), and `Link::request_packet_with_max_response_size` keeps its own
// gate (`RNS/Link.py:1046-1049`, `<=` admits a response exactly at the limit); a receipt and a
// bare request packet interleaved on one link each get their own Resource response. Helpers
// come from the sibling `tests_parts` files.

/// A request packet for `path` built with the bare `request_packet` and the id the
/// responder derives from it.
#[deny(clippy::await_holding_invalid_type)]
async fn sizes_probe2_bare_request_packet(
    pair: &LoopbackRequestPair,
    path: &str,
) -> (Packet, RequestId) {
    let packed = Link::request_payload(path, rmpv::Value::Nil).expect("request packs").packed;
    let packet =
        pair.handles.link.lock().await.request_packet(&packed).expect("request fits a packet");
    let id = RequestId::from_packet_hash(&packet.hash());
    (packet, id)
}

/// A request packet for `path` built with `request_packet_with_max_response_size(limit)`,
/// sent over the bound interface, and the id the responder derives from it.
#[deny(clippy::await_holding_invalid_type)]
async fn sizes_probe2_send_limited_request_packet(
    pair: &LoopbackRequestPair,
    path: &str,
    limit: Option<usize>,
) -> RequestId {
    let packed = Link::request_payload(path, rmpv::Value::Nil).expect("request packs").packed;
    let packet = pair
        .handles
        .link
        .lock()
        .await
        .request_packet_with_max_response_size(&packed, limit)
        .expect("request fits a packet");
    let id = RequestId::from_packet_hash(&packet.hash());
    let outcome = pair
        .handles
        .connector
        .transport
        .send_link_packet_on_bound_iface(&pair.handles.link, packet)
        .await;
    assert!(outcome.is_sent(), "request reaches the wire: {outcome:?}");
    id
}

/// A bare request packet put on the wire through the broadcast send seam
/// (`Transport::send_packet_broadcast_with_trace`), not the bound-iface helper the in-repo
/// clients use, still solicits its response: the marker is recorded where the transport
/// sends the packet, so the response Resource is admitted and assembled
/// (`RNS/Link.py:473-510`, `:1046-1047`). The plain `send_packet` seam is not used here:
/// the loopback pair runs with `broadcast: false`, under which an initiator-side link
/// packet has no route through it (`DroppedNoRoute`, unchanged since before this work).
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_sizes_probe2_bare_request_packet_via_broadcast_send_solicits_its_response(
) {
    let pair = loopback_request_pair_with_mtu("sizes-probe2-broadcast-send", LEGACY_MTU).await;
    let body = random_payload(MANY_PARTS_LEN);
    let expected = body.clone();
    register_handler(&pair, "/big", RequestAllow::All, move |_: InboundRequest| {
        std::future::ready(Some(RequestResponse::Bytes(body.clone())))
    })
    .await;
    let mut events = pair.handles.connector.transport.resource_events();

    let (packet, id) = sizes_probe2_bare_request_packet(&pair, "/big").await;
    let trace = pair.handles.connector.transport.send_packet_broadcast_with_trace(packet).await;
    assert!(trace.outcome.is_sent(), "request reaches the wire: {:?}", trace.outcome);

    let complete = await_response_resource(&mut events, pair.handles.link_id, id).await;
    let (answered, value) = unpack_response_envelope(&complete.data).expect("response envelope");
    assert_eq!(answered, id.to_bytes(), "the response names the request");
    assert_eq!(value, rmpv::Value::Binary(expected));
}

/// `request_packet_with_max_response_size(Some(limit))` keeps its gate: a response
/// Resource whose advertised `d` is over the limit is refused at the advertisement
/// (`RNS/Link.py:1049-1052`), the responder sees `OutboundRejected`, nothing of it is
/// assembled on the requester, and the link goes on answering.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_sizes_probe2_limited_request_packet_refuses_an_oversize_resource_response(
) {
    let pair = loopback_request_pair_with_mtu("sizes-probe2-limited-oversize", LEGACY_MTU).await;
    let mut connector_events = pair.handles.connector.transport.resource_events();
    let mut listener_events = pair.handles.listener.transport.resource_events();
    let link_id = pair.handles.link_id;
    let body = random_payload(MANY_PARTS_LEN);
    register_handler(&pair, "/big", RequestAllow::All, move |_: InboundRequest| {
        std::future::ready(Some(RequestResponse::Bytes(body.clone())))
    })
    .await;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;

    let id = sizes_probe2_send_limited_request_packet(&pair, "/big", Some(SIZES_TIGHT_LIMIT)).await;

    let rejected = await_resource_event(&mut listener_events, |event| {
        event.link_id == link_id && matches!(event.kind, ResourceEventKind::OutboundRejected)
    })
    .await;
    assert!(matches!(rejected.kind, ResourceEventKind::OutboundRejected));
    sizes_assert_no_response_activity(&mut connector_events, link_id, id).await;
    sizes_assert_link_still_answers(&pair).await;
}

/// `request_packet_with_max_response_size(Some(limit))` admits a response Resource whose
/// advertised `d` is exactly the limit (`<=`, `RNS/Link.py:1049`), and `None` admits any.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_sizes_probe2_limited_request_packet_admits_a_resource_response_at_the_limit(
) {
    let pair = loopback_request_pair_with_mtu("sizes-probe2-limited-at-limit", LEGACY_MTU).await;
    let mut events = pair.handles.connector.transport.resource_events();
    let link_id = pair.handles.link_id;
    let body = random_payload(MANY_PARTS_LEN);
    let expected = body.clone();
    let exact = sizes_envelope_len(&body);
    register_handler(&pair, "/big", RequestAllow::All, move |_: InboundRequest| {
        std::future::ready(Some(RequestResponse::Bytes(body.clone())))
    })
    .await;

    for limit in [Some(exact), None] {
        let id = sizes_probe2_send_limited_request_packet(&pair, "/big", limit).await;
        let complete = await_response_resource(&mut events, link_id, id).await;
        let (answered, value) =
            unpack_response_envelope(&complete.data).expect("response envelope");
        assert_eq!(answered, id.to_bytes(), "the response names the request ({limit:?})");
        assert_eq!(value, rmpv::Value::Binary(expected.clone()), "whole body ({limit:?})");
    }
}

/// A `Transport::request` receipt and a bare request packet interleaved on one link are
/// each answered by their own response Resource: the receipt resolves `Ready` with
/// `response_size` equal to its envelope and the bare request's response is assembled
/// under its own id; neither is dropped as unsolicited nor resolved by the other's body.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_sizes_probe2_receipt_and_bare_request_packet_interleave_on_one_link(
) {
    let pair = loopback_request_pair("sizes-probe2-interleave").await;
    let mut events = pair.handles.connector.transport.resource_events();
    let link_id = pair.handles.link_id;
    let receipt_body = over_mdu_body(&pair).await;
    let bare_body = random_payload(receipt_body.len() + 1);
    let receipt_expected = receipt_body.clone();
    let bare_expected = bare_body.clone();
    register_handler(&pair, "/receipt", RequestAllow::All, move |_: InboundRequest| {
        std::future::ready(Some(RequestResponse::Bytes(receipt_body.clone())))
    })
    .await;
    register_handler(&pair, "/bare", RequestAllow::All, move |_: InboundRequest| {
        std::future::ready(Some(RequestResponse::Bytes(bare_body.clone())))
    })
    .await;

    let receipt = request(&pair, "/receipt", rmpv::Value::Nil).await;
    let (bare_packet, bare_id) = sizes_probe2_bare_request_packet(&pair, "/bare").await;
    let outcome = pair
        .handles
        .connector
        .transport
        .send_link_packet_on_bound_iface(&pair.handles.link, bare_packet)
        .await;
    assert!(outcome.is_sent(), "bare request reaches the wire: {outcome:?}");
    assert_ne!(receipt.request_id(), bare_id);

    let bare = await_response_resource(&mut events, link_id, bare_id).await;
    let (answered, value) = unpack_response_envelope(&bare.data).expect("response envelope");
    assert_eq!(answered, bare_id.to_bytes(), "the bare response names its request");
    assert_eq!(value, rmpv::Value::Binary(bare_expected));

    let response = await_receipt(&receipt).await.expect("the receipt resolves");
    assert_eq!(receipt.status(), RequestStatus::Ready);
    assert_eq!(response.into_value(), Some(rmpv::Value::Binary(receipt_expected.clone())));
    assert_eq!(receipt.response_size(), Some(sizes_envelope_len(&receipt_expected)));
    assert_eq!(receipt.progress(), 1.0);
}
