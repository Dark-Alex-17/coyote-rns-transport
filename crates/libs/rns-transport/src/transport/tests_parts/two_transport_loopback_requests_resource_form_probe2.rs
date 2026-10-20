// Second loopback probe of the resource-form request path, written from the contract
// alone. The response-size rule is `len(packed_response) <= self.mdu` (`RNS/Link.py:849`):
// the sibling probe pins that the largest envelope that fits goes out as one packet, this
// file pins that the very next byte tips it into a response Resource and that no response
// packet travels beside it. Two request Resources in flight on one link are each answered
// under their own id, in whatever order they assemble. A handler that answers `None` for a
// request Resource sends nothing at all while the listener still posts
// `Complete(ResourceComplete { is_request: true, .. })` and the link keeps answering.
// Helpers come from the sibling `tests_parts` files.

/// The largest `Bytes` body whose `[request_id, body]` envelope still fits one link packet
/// (`RNS/Link.py:849`). The request id is always a 16-byte bin, so the envelope length
/// depends on the body length alone.
#[deny(clippy::await_holding_invalid_type)]
async fn largest_packet_body_len(pair: &LoopbackRequestPair) -> usize {
    let mdu = pair.handles.link.lock().await.link_mdu();
    let envelope_len = |body_len: usize| {
        ResponseFrame::new(RequestId::new([0u8; 16]), rmpv::Value::Binary(vec![0u8; body_len]))
            .encode()
            .len()
    };
    let fit = (0..=mdu).rev().find(|&len| envelope_len(len) <= mdu).expect("some body fits");
    assert!(envelope_len(fit + 1) > mdu, "fit is the boundary");
    fit
}

/// Waits until `calls` reads `expected`, or fails after `RESPONSE_TIMEOUT`.
#[deny(clippy::await_holding_invalid_type)]
async fn await_handler_calls(calls: &AtomicUsize, expected: usize) {
    timeout(RESPONSE_TIMEOUT, async {
        while calls.load(Ordering::SeqCst) < expected {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the handler ran");
}

/// `RNS/Link.py:849-852`: an envelope one byte over the link MDU is a response Resource,
/// never a packet, and it is the only thing that answers the request.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_response_one_byte_over_the_mdu_is_a_resource_only() {
    let mut pair = loopback_request_pair("request-mdu-boundary-over").await;
    let mut resource_events = pair.handles.connector.transport.resource_events();
    let link_id = pair.handles.link_id;
    let body = random_payload(largest_packet_body_len(&pair).await + 1);
    let expected = body.clone();
    register_handler(&pair, "/edge-over", RequestAllow::All, move |_: InboundRequest| {
        std::future::ready(Some(RequestResponse::Bytes(body.clone())))
    })
    .await;

    let id = send_request(&pair, "/edge-over", rmpv::Value::Nil).await;
    let complete = await_response_resource(&mut resource_events, link_id, id).await;
    let (answered, value) = unpack_response_envelope(&complete.data).expect("response envelope");
    assert_eq!(answered, id.to_bytes(), "the response names the request");
    assert_eq!(value, rmpv::Value::Binary(expected));

    let summary = drain_connector_tap(&mut pair).await;
    assert_eq!(summary.response_packets, 0, "nothing travelled as a response packet");
    let advertisements = response_advertisements_for(&summary, id);
    assert_eq!(advertisements.len(), 1, "exactly one response advertisement names the request");
    assert!(advertisements[0].is_response(), "p is set");
}

/// Two request Resources in flight on one link are each answered under their own id, and
/// the handler sees exactly those two ids.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_two_in_flight_requests_are_each_answered() {
    let pair = loopback_request_pair("request-resource-form-two-in-flight").await;
    let mut resource_events = pair.handles.connector.transport.resource_events();
    let link_id = pair.handles.link_id;
    let (seen_tx, mut seen_rx) = tokio::sync::mpsc::unbounded_channel();
    register_handler(&pair, "/echo", RequestAllow::All, recording_echo_handler(seen_tx)).await;

    let body_a = over_mdu_body(&pair).await;
    let body_b = random_payload(body_a.len() + 1);
    let (id_a, _) =
        send_request_as_resource(&pair, "/echo", rmpv::Value::Binary(body_a.clone())).await;
    let (id_b, _) =
        send_request_as_resource(&pair, "/echo", rmpv::Value::Binary(body_b.clone())).await;
    assert_ne!(id_a, id_b, "distinct bodies hash to distinct request ids");

    let mut answers = std::collections::HashMap::new();
    while answers.len() < 2 {
        let complete = await_resource_event(&mut resource_events, |event| {
            event.link_id == link_id
                && matches!(&event.kind, ResourceEventKind::Complete(complete)
                if complete.is_response
                    && [id_a, id_b].iter().any(|id| {
                        complete.request_id.as_deref() == Some(id.as_bytes().as_slice())
                    }))
        })
        .await;
        let complete = complete_payload(complete);
        let (answered, value) =
            unpack_response_envelope(&complete.data).expect("response envelope");
        assert_eq!(
            complete.request_id.as_deref(),
            Some(answered.as_slice()),
            "the advertised q and the envelope name the same request"
        );
        assert!(answers.insert(answered, value).is_none(), "each request is answered once");
    }
    assert_eq!(answers.remove(&id_a.to_bytes()), Some(rmpv::Value::Binary(body_a)));
    assert_eq!(answers.remove(&id_b.to_bytes()), Some(rmpv::Value::Binary(body_b)));

    let first = seen_rx.recv().await.expect("handler ran");
    let second = seen_rx.recv().await.expect("handler ran twice");
    let mut seen = [first, second];
    seen.sort_by_key(|id| id.to_bytes());
    let mut sent = [id_a, id_b];
    sent.sort_by_key(|id| id.to_bytes());
    assert_eq!(seen, sent, "the handler saw exactly the two ids");
    assert!(
        matches!(seen_rx.try_recv(), Err(tokio::sync::mpsc::error::TryRecvError::Empty)),
        "the handler ran exactly twice"
    );
}

/// `RNS/Link.py:844`: a handler that answers `None` for a request Resource sends nothing,
/// neither a response packet nor a response advertisement, while the listener still posts
/// `Complete(ResourceComplete { is_request: true, .. })` and the link keeps answering.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_handler_none_sends_nothing() {
    let mut pair = loopback_request_pair("request-resource-form-handler-none").await;
    let mut listener_events = pair.handles.listener.transport.resource_events();
    let mut connector_events = pair.handles.connector.transport.resource_events();
    let link_id = pair.handles.link_id;
    let calls = Arc::new(AtomicUsize::new(0));
    let counted = calls.clone();
    register_handler(&pair, "/quiet", RequestAllow::All, move |_: InboundRequest| {
        counted.fetch_add(1, Ordering::SeqCst);
        std::future::ready(None::<RequestResponse>)
    })
    .await;

    let body = over_mdu_body(&pair).await;
    let (quiet, hash) = send_request_as_resource(&pair, "/quiet", rmpv::Value::Binary(body)).await;
    let complete = await_request_resource_complete(&mut listener_events, link_id).await;
    assert_eq!(complete.request_id.as_deref(), Some(quiet.as_bytes().as_slice()));
    let outcome = await_outbound_terminal(&mut connector_events, hash).await;
    assert!(matches!(outcome, ResourceEventKind::OutboundComplete), "accepted, got {outcome:?}");
    await_handler_calls(&calls, 1).await;

    // Bound the silence window with a data round trip in both directions, then look.
    exchange_data_both_ways(&mut pair.handles).await;
    let summary = drain_connector_tap(&mut pair).await;
    assert_eq!(summary.response_packets, 0, "no response packet for /quiet");
    assert!(response_advertisements_for(&summary, quiet).is_empty(), "nothing answered /quiet");

    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;
    let id = send_request(&pair, "/echo", rmpv::Value::from("still here")).await;
    assert_eq!(await_response(&mut pair, id).await, rmpv::Value::from("still here"));
    assert_eq!(calls.load(Ordering::SeqCst), 1, "the quiet handler ran once");
}
