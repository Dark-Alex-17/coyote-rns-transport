// Third loopback probe of the resource-form request path, written from the contract alone.
// The response form follows the response alone (`RNS/Link.py:849`): a request that arrived
// as a request Resource is still answered by one `Response` packet when its envelope fits
// the link MDU, and no response Resource travels beside it. `allow_mode == ALLOW_NONE`
// (`RNS/Link.py:818-824`) holds on the resource path as it does for a packet: the handler
// never runs, the listener still posts `Complete(ResourceComplete { is_request: true, .. })`,
// and re-registering the path lifts the denial on the same link. One handler serves both
// request forms interleaved on one link, each answered under its own id. Helpers come from
// the sibling `tests_parts` files.

/// Collects the `Response` packets for `ids` from the connector's link events, in whatever
/// order they arrive, and returns them keyed by the request id each one names.
#[deny(clippy::await_holding_invalid_type)]
async fn await_response_packets_for(
    pair: &mut LoopbackRequestPair,
    ids: &[RequestId],
) -> std::collections::HashMap<[u8; 16], rmpv::Value> {
    let link_id = pair.handles.link_id;
    let mut answers = std::collections::HashMap::new();
    while answers.len() < ids.len() {
        let event = timeout(
            RESPONSE_TIMEOUT,
            await_event(&mut pair.out_events, |event| {
                event.id == link_id
                    && matches!(&event.event, LinkEvent::Data(payload)
                        if payload.context() == PacketContext::Response)
            }),
        )
        .await
        .expect("response packet arrives");
        let LinkEvent::Data(payload) = event.event else { unreachable!("matched above") };
        let (answered, body) =
            unpack_response_envelope(payload.as_slice()).expect("response envelope");
        assert!(
            ids.iter().any(|id| id.to_bytes() == answered),
            "the response names one of the requests sent"
        );
        assert!(answers.insert(answered, body).is_none(), "each request is answered once");
    }
    answers
}

/// `RNS/Link.py:849`: the response form follows the response, not the request. A request
/// Resource whose handler answers a small `Value` is answered by exactly one `Response`
/// packet, and no response Resource is advertised for it.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_small_response_is_one_packet() {
    let mut pair = loopback_request_pair("request-resource-form-small-response").await;
    let mut connector_events = pair.handles.connector.transport.resource_events();
    let calls = Arc::new(AtomicUsize::new(0));
    let counted = calls.clone();
    register_handler(&pair, "/ack", RequestAllow::All, move |request: InboundRequest| {
        counted.fetch_add(1, Ordering::SeqCst);
        let len = match request.into_data() {
            rmpv::Value::Binary(body) => body.len(),
            other => panic!("the request body is binary, got {other:?}"),
        };
        std::future::ready(Some(RequestResponse::Value(rmpv::Value::from(len as u64))))
    })
    .await;

    let body = over_mdu_body(&pair).await;
    let expected_len = body.len() as u64;
    let (id, hash) = send_request_as_resource(&pair, "/ack", rmpv::Value::Binary(body)).await;
    let answer = await_response(&mut pair, id).await;
    assert_eq!(answer, rmpv::Value::from(expected_len), "the handler saw the whole body");
    assert_eq!(calls.load(Ordering::SeqCst), 1, "the handler ran once");

    // Bound the window with a data round trip in both directions, then look at the wire.
    exchange_data_both_ways(&mut pair.handles).await;
    let summary = drain_connector_tap(&mut pair).await;
    assert_eq!(summary.response_packets, 1, "exactly one response packet answered the request");
    assert!(
        response_advertisements_for(&summary, id).is_empty(),
        "no response Resource is advertised when the envelope fits the MDU"
    );
    // The request Resource itself was accepted, whatever form its answer took.
    let outcome = await_resource_event(&mut connector_events, |event| {
        event.hash == hash
            && matches!(
                event.kind,
                ResourceEventKind::OutboundRejected
                    | ResourceEventKind::OutboundComplete
                    | ResourceEventKind::OutboundFailed
            )
    })
    .await;
    assert!(
        matches!(outcome.kind, ResourceEventKind::OutboundComplete),
        "the request Resource completed, got {:?}",
        outcome.kind
    );
}

/// `RNS/Link.py:818-824` on the resource path: `allow_mode == ALLOW_NONE` runs no handler and
/// answers nothing while `Complete(ResourceComplete { is_request: true, .. })` still posts;
/// re-registering the same path with `All` answers the next request Resource on the same
/// link.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_allow_none_is_silent() {
    let mut pair = loopback_request_pair("request-resource-form-allow-none").await;
    let mut listener_events = pair.handles.listener.transport.resource_events();
    let mut connector_events = pair.handles.connector.transport.resource_events();
    let link_id = pair.handles.link_id;
    let calls = Arc::new(AtomicUsize::new(0));
    register_handler(&pair, "/echo", RequestAllow::None, counting_echo_handler(calls.clone()))
        .await;

    let body = over_mdu_body(&pair).await;
    let (denied, hash) =
        send_request_as_resource(&pair, "/echo", rmpv::Value::Binary(body.clone())).await;
    let complete = await_request_resource_complete(&mut listener_events, link_id).await;
    assert_eq!(
        complete.request_id.as_deref(),
        Some(denied.as_bytes().as_slice()),
        "Complete posts for the denied request"
    );
    let outcome = await_outbound_terminal(&mut connector_events, hash).await;
    assert!(matches!(outcome, ResourceEventKind::OutboundComplete), "accepted, got {outcome:?}");

    // Bound the silence window with a data round trip in both directions, then look.
    exchange_data_both_ways(&mut pair.handles).await;
    let summary = drain_connector_tap(&mut pair).await;
    assert_eq!(summary.response_packets, 0, "no response packet for the denied request");
    assert!(
        response_advertisements_for(&summary, denied).is_empty(),
        "ALLOW_NONE: nothing answers"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0, "ALLOW_NONE: the handler never ran");

    register_handler(&pair, "/echo", RequestAllow::All, counting_echo_handler(calls.clone())).await;
    let (answered, _) =
        send_request_as_resource(&pair, "/echo", rmpv::Value::Binary(body.clone())).await;
    let response = await_response_resource(&mut connector_events, link_id, answered).await;
    let (_, echoed) = unpack_response_envelope(&response.data).expect("response envelope");
    assert_eq!(echoed, rmpv::Value::Binary(body), "re-registered with All: answered");
    assert_eq!(calls.load(Ordering::SeqCst), 1, "the handler ran only after re-registration");

    let summary = drain_connector_tap(&mut pair).await;
    assert_eq!(summary.response_packets, 0, "the echo outgrows the MDU");
    assert_eq!(response_advertisements_for(&summary, answered).len(), 1, "answered once");
    assert!(
        response_advertisements_for(&summary, denied).is_empty(),
        "still nothing for the denied id"
    );
}

/// One handler serves both request forms interleaved on one link: a request packet, a
/// request Resource and another request packet are each answered under their own id, the
/// packets by `Response` packets and the Resource by a response Resource, and the handler
/// sees exactly the three ids.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_and_packet_form_interleave_on_one_link() {
    let mut pair = loopback_request_pair("request-resource-form-interleave").await;
    let mut connector_events = pair.handles.connector.transport.resource_events();
    let link_id = pair.handles.link_id;
    let (seen_tx, mut seen_rx) = tokio::sync::mpsc::unbounded_channel();
    register_handler(&pair, "/echo", RequestAllow::All, recording_echo_handler(seen_tx)).await;

    let body = over_mdu_body(&pair).await;
    let first = send_request(&pair, "/echo", rmpv::Value::from("first")).await;
    let (middle, _) =
        send_request_as_resource(&pair, "/echo", rmpv::Value::Binary(body.clone())).await;
    let last = send_request(&pair, "/echo", rmpv::Value::from("last")).await;
    assert_ne!(first, last, "distinct packets hash to distinct request ids");

    let mut packets = await_response_packets_for(&mut pair, &[first, last]).await;
    assert_eq!(packets.remove(&first.to_bytes()), Some(rmpv::Value::from("first")));
    assert_eq!(packets.remove(&last.to_bytes()), Some(rmpv::Value::from("last")));
    let response = await_response_resource(&mut connector_events, link_id, middle).await;
    let (answered, echoed) = unpack_response_envelope(&response.data).expect("response envelope");
    assert_eq!(answered, middle.to_bytes());
    assert_eq!(echoed, rmpv::Value::Binary(body));

    let mut seen = Vec::new();
    for _ in 0..3 {
        seen.push(seen_rx.recv().await.expect("the handler ran three times"));
    }
    seen.sort_by_key(|id| id.to_bytes());
    let mut sent = vec![first, middle, last];
    sent.sort_by_key(|id| id.to_bytes());
    assert_eq!(seen, sent, "the handler saw exactly the three ids");
    assert!(
        matches!(seen_rx.try_recv(), Err(tokio::sync::mpsc::error::TryRecvError::Empty)),
        "the handler ran exactly three times"
    );

    let summary = drain_connector_tap(&mut pair).await;
    assert_eq!(
        summary.response_packets, 2,
        "the two packet-form requests were answered by packets"
    );
    assert_eq!(
        response_advertisements_for(&summary, middle).len(),
        1,
        "the Resource by a Resource"
    );
    assert!(response_advertisements_for(&summary, first).is_empty());
    assert!(response_advertisements_for(&summary, last).is_empty());
}
