// Loopback coverage for the packet-form half of the `set_max_response_size` rustdoc: the
// responder's cap measures the whole packed `[request_id, body]` frame while the requester's
// `RequestOptions::with_max_response_size` measures the packed body alone, so the two caps
// do not compose at one number for a packet response the way they do for a Resource (the
// sibling probe2). With both caps at the packed body length the responder refuses first and
// nothing reaches the wire; with the responder's cap at the frame length a requester cap at
// the body length admits the response and one byte under it rejects the packet. Helpers come
// from the sibling `tests_parts` files.

/// The length the requester measures a packet-form response at: the packed body alone,
/// `unpacked_response[1]` re-encoded.
fn packet_body_len(body: &rmpv::Value) -> usize {
    let mut packed = Vec::new();
    rmpv::encode::write_value(&mut packed, body).expect("body packs");
    packed.len()
}

/// Sends `/frame` under a requester cap of `requester_max` and returns the receipt verdict
/// and the responder's tracked outcome for that one request.
#[deny(clippy::await_holding_invalid_type)]
async fn frame_response_under_requester_cap(
    pair: &LoopbackRequestPair,
    outcomes: &mut tokio::sync::mpsc::UnboundedReceiver<ResponseOutcomeReceiver>,
    requester_max: usize,
) -> (Result<RequestResponseData, RequestError>, ResponseOutcome) {
    let receipt = request_with(
        pair,
        "/frame",
        rmpv::Value::Nil,
        RequestOptions::default().with_max_response_size(requester_max),
    )
    .await;
    let verdict = await_receipt(&receipt).await;
    let outcome = await_outcome_within(outcomes, RESOURCE_TIMEOUT).await;
    (verdict, outcome)
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_packet_form_caps_measure_the_frame_and_the_body() {
    install_test_logger();
    let mut pair = loopback_request_pair("request-response-cap-probe4-packet-both-ends").await;
    let link_id = pair.handles.link_id;
    let body = rmpv::Value::from("the body the requester measures");
    let body_len = packet_body_len(&body);
    let frame_len = packet_response_len(body.clone());
    assert!(frame_len > body_len, "the frame carries the id on top of the body");
    let (outcome_tx, mut outcomes) = tokio::sync::mpsc::unbounded_channel();
    let expected = body.clone();
    register_handler(
        &pair,
        "/frame",
        RequestAllow::All,
        tracking_handler(outcome_tx, move |_| Some(RequestResponse::Value(body.clone()))),
    )
    .await;

    // Responder cap at the frame, requester cap at the body: both admit the response.
    set_max_response_size(&pair, frame_len).await;
    let (verdict, outcome) =
        frame_response_under_requester_cap(&pair, &mut outcomes, body_len).await;
    let response = verdict.expect("a requester cap at the packed body length admits the packet");
    assert_eq!(response.value(), Some(&expected));
    assert_eq!(outcome, ResponseOutcome::Packet);

    // One byte under the body: the responder still sends, the requester refuses the packet.
    let (verdict, outcome) =
        frame_response_under_requester_cap(&pair, &mut outcomes, body_len - 1).await;
    assert_eq!(
        verdict,
        Err(RequestError::Oversize),
        "a requester cap one under the packed body length {body_len} rejects the packet"
    );
    assert_eq!(outcome, ResponseOutcome::Packet, "the responder's cap at the frame let it out");
    assert!(
        refused_response_records_for(link_id).is_empty(),
        "the responder's own cap refused nothing so far: {:?}",
        refused_response_records_for(link_id)
    );

    // Equal caps at the body length: the responder measures the frame and refuses first,
    // so the requester, whose cap would have admitted the body, sees nothing arrive.
    drain_connector_tap(&mut pair).await;
    let mut resource_events = pair.handles.connector.transport.resource_events();
    set_max_response_size(&pair, body_len).await;
    let receipt = request_with(
        &pair,
        "/frame",
        rmpv::Value::Nil,
        RequestOptions::default().with_max_response_size(body_len),
    )
    .await;
    let id = receipt.request_id();
    assert_eq!(
        await_outcome_within(&mut outcomes, RESOURCE_TIMEOUT).await,
        ResponseOutcome::TooLarge { len: frame_len, max: body_len }
    );
    assert_nothing_sent_for(&mut pair, &mut resource_events, id).await;
    assert_one_refusal_warn(link_id, id, frame_len, body_len);
}
