// Usage-pattern coverage of the public `Link::expect_response` composed with
// `Link::set_response_size_limit`, written from their rustdoc contracts: a limit already
// recorded under the id stays when `expect_response` is given none, and a limit it is given
// replaces the one recorded. Each case is told apart on the wire through a bare
// `request_packet` whose id is known before the send: a kept or replaced limit under the
// response refuses it at its advertisement and the responder sees `OutboundRejected`
// (`RNS/Link.py:1046-1049`); a limit that covers the envelope lets it assemble; a lost record
// would drop the response without a reject. Helpers come from the sibling `tests_parts` files.

/// Builds a bare request packet for `/big`, runs `arrange` on the link with the id the
/// responder derives from it, puts the packet on the wire and returns the id.
#[deny(clippy::await_holding_invalid_type)]
async fn sizes_probe7_send_arranged_request_packet(
    pair: &LoopbackRequestPair,
    arrange: impl FnOnce(&mut Link, &RequestId),
) -> RequestId {
    let (packet, id) = sizes_probe2_bare_request_packet(pair, "/big").await;
    arrange(&mut *pair.handles.link.lock().await, &id);
    let outcome = pair
        .handles
        .connector
        .transport
        .send_link_packet_on_bound_iface(&pair.handles.link, packet)
        .await;
    assert!(outcome.is_sent(), "request reaches the wire: {outcome:?}");
    id
}

/// Asserts the response under `id` was refused at its advertisement: the responder sees a
/// reject on the pair's link (an `OutboundComplete` still arriving for an earlier response
/// on the link is passed over; a failure is not), and the connector assembles nothing
/// under `id`.
#[deny(clippy::await_holding_invalid_type)]
async fn sizes_probe7_assert_refused(
    pair: &LoopbackRequestPair,
    events: &mut broadcast::Receiver<ResourceEvent>,
    responder_events: &mut broadcast::Receiver<ResourceEvent>,
    id: RequestId,
) {
    let link_id = pair.handles.link_id;
    let ended = await_resource_event(responder_events, |event| {
        event.link_id == link_id
            && matches!(
                event.kind,
                ResourceEventKind::OutboundRejected { .. } | ResourceEventKind::OutboundFailed
            )
    })
    .await;
    assert!(
        matches!(ended.kind, ResourceEventKind::OutboundRejected { .. }),
        "the response over the limit is refused at its advertisement, not failed: {:?}",
        ended.kind
    );
    sizes_assert_no_response_activity(events, link_id, id).await;
}

/// Waits for the response under `id` to assemble and asserts it is the `/big` body.
#[deny(clippy::await_holding_invalid_type)]
async fn sizes_probe7_assert_assembled(
    pair: &LoopbackRequestPair,
    events: &mut broadcast::Receiver<ResourceEvent>,
    id: RequestId,
    body: &[u8],
) {
    let complete = await_response_resource(events, pair.handles.link_id, id).await;
    assert_eq!(complete.data.len(), sizes_envelope_len(body), "the whole envelope is delivered");
    let (answered, value) = unpack_response_envelope(&complete.data).expect("response envelope");
    assert_eq!(answered, id.to_bytes(), "the response names the request");
    assert_eq!(value, rmpv::Value::Binary(body.to_vec()));
}

/// `expect_response(id, None)` after `set_response_size_limit(id, limit)` keeps the limit:
/// a response one byte over it is refused with a reject, one exactly at it is assembled;
/// the link still answers.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_sizes_probe7_expect_response_without_a_limit_keeps_the_one_set(
) {
    let pair = loopback_request_pair("sizes-probe7-expect-keeps-limit").await;
    let body = over_mdu_body(&pair).await;
    let envelope_len = sizes_envelope_len(&body);
    let served = body.clone();
    register_handler(&pair, "/big", RequestAllow::All, move |_: InboundRequest| {
        std::future::ready(Some(RequestResponse::Bytes(served.clone())))
    })
    .await;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;
    let mut events = pair.handles.connector.transport.resource_events();

    let mut responder_events = pair.handles.listener.transport.resource_events();
    let refused_id = sizes_probe7_send_arranged_request_packet(&pair, |link, id| {
        link.set_response_size_limit(id.as_bytes(), envelope_len - 1);
        link.expect_response(id.as_bytes(), None);
    })
    .await;
    sizes_probe7_assert_refused(&pair, &mut events, &mut responder_events, refused_id).await;

    let served_id = sizes_probe7_send_arranged_request_packet(&pair, |link, id| {
        link.set_response_size_limit(id.as_bytes(), envelope_len);
        link.expect_response(id.as_bytes(), None);
    })
    .await;
    sizes_probe7_assert_assembled(&pair, &mut events, served_id, &body).await;

    sizes_assert_link_still_answers(&pair).await;
}

/// `expect_response(id, Some(limit))` replaces a limit set before it under the same id, in
/// both directions: a limit raised to the envelope length admits the response a tighter
/// `set_response_size_limit` would have refused, and a limit lowered under it refuses the
/// response a wider one would have admitted; the link still answers.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_sizes_probe7_expect_response_with_a_limit_replaces_the_one_set(
) {
    let pair = loopback_request_pair("sizes-probe7-expect-replaces-limit").await;
    let body = over_mdu_body(&pair).await;
    let envelope_len = sizes_envelope_len(&body);
    let served = body.clone();
    register_handler(&pair, "/big", RequestAllow::All, move |_: InboundRequest| {
        std::future::ready(Some(RequestResponse::Bytes(served.clone())))
    })
    .await;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;
    let mut events = pair.handles.connector.transport.resource_events();

    let raised_id = sizes_probe7_send_arranged_request_packet(&pair, |link, id| {
        link.set_response_size_limit(id.as_bytes(), envelope_len - 1);
        link.expect_response(id.as_bytes(), Some(envelope_len));
    })
    .await;
    sizes_probe7_assert_assembled(&pair, &mut events, raised_id, &body).await;

    let mut responder_events = pair.handles.listener.transport.resource_events();
    let lowered_id = sizes_probe7_send_arranged_request_packet(&pair, |link, id| {
        link.set_response_size_limit(id.as_bytes(), envelope_len);
        link.expect_response(id.as_bytes(), Some(envelope_len - 1));
    })
    .await;
    sizes_probe7_assert_refused(&pair, &mut events, &mut responder_events, lowered_id).await;

    sizes_assert_link_still_answers(&pair).await;
}
