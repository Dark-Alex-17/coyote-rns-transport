// Loopback probes for the corners of the response cap and `into_parts` that the sibling
// files reach only through a packet-form request: a request that arrives as a
// request Resource runs "the same handler" (README request-handlers section), so the
// response it earns is measured against the destination's `max_response_size` like any
// other and `into_parts` hands the handler the assembled request data; a cap of zero
// refuses every response, the smallest included, with `TooLarge { len, max: 0 }`; a handler
// that answers `None` under a cap has nothing to measure, so the verdict stays `NotSent` and
// the refusal warn store stays empty; and `Transport::send_response`, addressed to a link,
// goes out untouched by a destination cap that would refuse the same bytes from a handler,
// which is the reach the README and rustdoc disclose. Helpers come from the sibling
// `tests_parts` files.

/// A request Resource arrives, the handler answers with a small packet-form body one byte
/// over the cap: refused with `TooLarge`, nothing on the wire, one warn naming the id the
/// responder recomputed from the assembled bytes.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_request_response_over_the_cap_is_too_large() {
    install_test_logger();
    let mut pair = loopback_request_pair("request-response-cap-probe3-resource-request").await;
    let link_id = pair.handles.link_id;
    let body = rmpv::Value::from("short answer to a long request");
    let len = packet_response_len(body.clone());
    let max = len - 1;
    set_max_response_size(&pair, max).await;
    let (outcome_tx, mut outcomes) = tokio::sync::mpsc::unbounded_channel();
    register_handler(
        &pair,
        "/capped",
        RequestAllow::All,
        tracking_handler(outcome_tx, move |_| Some(RequestResponse::Value(body.clone()))),
    )
    .await;

    let request = over_mdu_body(&pair).await;
    let (id, _) = send_request_as_resource(&pair, "/capped", rmpv::Value::Binary(request)).await;
    assert_eq!(
        await_outcome_within(&mut outcomes, RESOURCE_TIMEOUT).await,
        ResponseOutcome::TooLarge { len, max }
    );
    assert_no_response(&mut pair).await;
    assert_one_refusal_warn(link_id, id, len, max);
}

/// The same request Resource, with the cap exactly at the response length: the response
/// goes out as a packet and the tracker reads `Packet`, so the resource-form request path
/// keeps the inclusive edge the packet-form path has.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_request_response_at_the_cap_goes_out() {
    let mut pair = loopback_request_pair("request-response-cap-probe3-resource-request-at").await;
    let body = rmpv::Value::from("exactly at the cap");
    set_max_response_size(&pair, packet_response_len(body.clone())).await;
    let (outcome_tx, mut outcomes) = tokio::sync::mpsc::unbounded_channel();
    register_handler(
        &pair,
        "/capped",
        RequestAllow::All,
        tracking_handler(outcome_tx, move |_| Some(RequestResponse::Value(body.clone()))),
    )
    .await;

    let request = over_mdu_body(&pair).await;
    let (id, _) = send_request_as_resource(&pair, "/capped", rmpv::Value::Binary(request)).await;
    assert_eq!(
        await_outcome_within(&mut outcomes, RESOURCE_TIMEOUT).await,
        ResponseOutcome::Packet
    );
    assert_eq!(await_response(&mut pair, id).await, rmpv::Value::from("exactly at the cap"));
}

/// `into_parts` on a request that arrived as a Resource yields the assembled request data,
/// byte for byte, and the tracker armed afterwards still reads the packet verdict.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_request_into_parts_yields_the_assembled_data()
{
    let mut pair = loopback_request_pair("request-response-cap-probe3-into-parts-resource").await;
    let request = over_mdu_body(&pair).await;
    let expected = request.clone();
    let (outcome_tx, mut outcomes) = tokio::sync::mpsc::unbounded_channel();
    register_handler(
        &pair,
        "/compare",
        RequestAllow::All,
        into_parts_tracking_handler(outcome_tx, move |data| {
            let same = data.as_slice() == Some(expected.as_slice());
            Some(RequestResponse::Value(rmpv::Value::Boolean(same)))
        }),
    )
    .await;

    let (id, _) = send_request_as_resource(&pair, "/compare", rmpv::Value::Binary(request)).await;
    assert_eq!(
        await_response(&mut pair, id).await,
        rmpv::Value::Boolean(true),
        "the handler saw the assembled request bytes"
    );
    assert_eq!(
        await_outcome_within(&mut outcomes, RESOURCE_TIMEOUT).await,
        ResponseOutcome::Packet
    );
}

/// `set_max_response_size(0)` is accepted (`tests/response_cap_tracker_api.rs`); here it
/// refuses the smallest possible response, a `Nil` body, with `len` the frame the dispatcher
/// measured and `max` zero.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_zero_cap_refuses_even_a_nil_response() {
    install_test_logger();
    let mut pair = loopback_request_pair("request-response-cap-probe3-zero").await;
    let link_id = pair.handles.link_id;
    let len = packet_response_len(rmpv::Value::Nil);
    set_max_response_size(&pair, 0).await;
    let (outcome_tx, mut outcomes) = tokio::sync::mpsc::unbounded_channel();
    register_handler(
        &pair,
        "/nil",
        RequestAllow::All,
        tracking_handler(outcome_tx, |_| Some(RequestResponse::Value(rmpv::Value::Nil))),
    )
    .await;

    let id = send_request(&pair, "/nil", rmpv::Value::Nil).await;
    assert_eq!(await_outcome(&mut outcomes).await, ResponseOutcome::TooLarge { len, max: 0 });
    assert_no_response(&mut pair).await;
    assert_one_refusal_warn(link_id, id, len, 0);
}

/// A handler that answers `None` under the tightest cap: there is no response to measure,
/// so the verdict is `NotSent` as without a cap, and the refusal warn store stays empty.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_none_under_a_cap_is_not_sent_without_a_refusal_warn() {
    install_test_logger();
    let mut pair = loopback_request_pair("request-response-cap-probe3-none").await;
    let link_id = pair.handles.link_id;
    set_max_response_size(&pair, 0).await;
    let (outcome_tx, mut outcomes) = tokio::sync::mpsc::unbounded_channel();
    register_handler(&pair, "/silent", RequestAllow::All, tracking_handler(outcome_tx, |_| None))
        .await;

    send_request(&pair, "/silent", rmpv::Value::Nil).await;
    assert_eq!(await_outcome(&mut outcomes).await, ResponseOutcome::NotSent);
    assert_no_response(&mut pair).await;
    assert!(
        refused_response_records_for(link_id).is_empty(),
        "`None` is not a response the cap can refuse: {:?}",
        refused_response_records_for(link_id)
    );
}

/// The reach the docs disclose: `Transport::send_response` is addressed to a link, not a
/// destination, so a destination cap of zero that would refuse the same frame from a
/// handler does not touch it. The handler answers `None` and reports the id; the test then
/// sends the very frame the dispatcher would have refused, and the requester receives it.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_transport_send_response_is_not_measured_by_the_cap() {
    install_test_logger();
    let mut pair = loopback_request_pair("request-response-cap-probe3-reach").await;
    let link_id = pair.handles.link_id;
    set_max_response_size(&pair, 0).await;
    let (seen_tx, mut seen_rx) = tokio::sync::mpsc::unbounded_channel();
    register_handler(&pair, "/manual", RequestAllow::All, move |request: InboundRequest| {
        seen_tx.send(request.request_id()).expect("test receiver is alive");
        std::future::ready(None)
    })
    .await;

    let id = send_request(&pair, "/manual", rmpv::Value::Nil).await;
    assert_eq!(seen_rx.recv().await, Some(id), "the handler saw the request");
    let envelope = ResponseFrame::new(id, rmpv::Value::from("by hand")).encode();
    let sent = pair
        .handles
        .listener
        .transport
        .send_response(&link_id, id.to_bytes().to_vec(), envelope, None)
        .await
        .expect("a link-addressed response is not gated by the destination cap");
    assert_eq!(sent, None, "a packet-sized envelope goes out as one packet");
    assert_eq!(await_response(&mut pair, id).await, rmpv::Value::from("by hand"));
    assert!(
        refused_response_records_for(link_id).is_empty(),
        "nothing was measured, so nothing was refused: {:?}",
        refused_response_records_for(link_id)
    );
}
