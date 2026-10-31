// Loopback pins over the responder's `max_response_size` and the `into_parts` tracker:
// the cap in force is the one read when the request was dispatched, so a cap tightened
// while the handler runs lets that response out and refuses the next one; a tracker from
// `into_parts` armed after an earlier `track_outcome` retires that first receiver with
// `NotSent`; and the responder's Resource measure is the advertised size the requester's
// cap reads, so equal caps admit a metadata-bearing response at that size, a requester cap
// one under it rejects the advertisement, and a responder cap one under it refuses before
// anything is sent. Helpers come from the sibling `tests_parts` files.

/// A handler that tracks the outcome, then holds its answer until `release` is notified.
/// The receiver arriving on `outcomes` proves the dispatcher already read the cap.
fn gated_tracking_handler(
    outcomes: tokio::sync::mpsc::UnboundedSender<ResponseOutcomeReceiver>,
    release: Arc<tokio::sync::Notify>,
    body: rmpv::Value,
) -> impl Fn(
    InboundRequest,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Option<RequestResponse>> + Send>>
       + Send
       + Sync {
    move |mut request: InboundRequest| {
        outcomes.send(request.track_outcome()).expect("test receiver is alive");
        let release = release.clone();
        let body = body.clone();
        Box::pin(async move {
            release.notified().await;
            Some(RequestResponse::Value(body))
        })
    }
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_cap_tightened_while_the_handler_runs_applies_to_the_next_request(
) {
    install_test_logger();
    let mut pair = loopback_request_pair("request-response-cap-probe2-mid-flight").await;
    let link_id = pair.handles.link_id;
    let body = rmpv::Value::from("dispatched before the cap");
    let len = packet_response_len(body.clone());
    let max = len - 1;
    let release = Arc::new(tokio::sync::Notify::new());
    let (outcome_tx, mut outcomes) = tokio::sync::mpsc::unbounded_channel();
    register_handler(
        &pair,
        "/gated",
        RequestAllow::All,
        gated_tracking_handler(outcome_tx, release.clone(), body),
    )
    .await;

    let first = send_request(&pair, "/gated", rmpv::Value::Nil).await;
    let first_receiver = timeout(RESPONSE_TIMEOUT, outcomes.recv())
        .await
        .expect("handler entered")
        .expect("handler tracked the outcome");
    set_max_response_size(&pair, max).await;
    release.notify_one();
    assert_eq!(
        await_response(&mut pair, first).await,
        rmpv::Value::from("dispatched before the cap"),
        "the cap read at dispatch was unbounded"
    );
    assert_eq!(
        timeout(RESPONSE_TIMEOUT, first_receiver).await.expect("outcome resolves"),
        ResponseOutcome::Packet
    );
    assert!(refused_response_records_for(link_id).is_empty(), "nothing refused yet");
    // The first response packet is still in the connector tap; the silence check below
    // counts response packets, so start it from an empty tap.
    drain_connector_tap(&mut pair).await;

    let second = send_request(&pair, "/gated", rmpv::Value::Nil).await;
    release.notify_one();
    assert_eq!(await_outcome(&mut outcomes).await, ResponseOutcome::TooLarge { len, max });
    assert_no_response(&mut pair).await;
    assert_one_refusal_warn(link_id, second, len, max);
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_into_parts_tracker_retires_an_earlier_track_outcome_receiver(
) {
    install_test_logger();
    let mut pair = loopback_request_pair("request-response-cap-probe2-retire").await;
    let link_id = pair.handles.link_id;
    let (pair_tx, mut pairs) = tokio::sync::mpsc::unbounded_channel();
    register_handler(&pair, "/twice", RequestAllow::All, move |mut request: InboundRequest| {
        let earlier = request.track_outcome();
        let (data, tracker) = request.into_parts();
        pair_tx.send((earlier, tracker.outcome())).expect("test receiver is alive");
        std::future::ready(Some(RequestResponse::Value(data)))
    })
    .await;

    let id = send_request(&pair, "/twice", rmpv::Value::from("twice tracked")).await;
    assert_eq!(await_response(&mut pair, id).await, rmpv::Value::from("twice tracked"));
    let (earlier, later) = pairs.recv().await.expect("the handler handed out both receivers");
    assert_eq!(
        timeout(RESPONSE_TIMEOUT, earlier).await.expect("retired receiver resolves"),
        ResponseOutcome::NotSent,
        "the tracker replaced the sender track_outcome installed"
    );
    assert_eq!(
        timeout(RESPONSE_TIMEOUT, later).await.expect("live receiver resolves"),
        ResponseOutcome::Packet
    );
    assert!(
        dispatch_outcome_receiver_dropped_records_for(link_id).is_empty(),
        "the live receiver was awaited, so the dispatcher dropped nothing: {:?}",
        dispatch_outcome_receiver_dropped_records_for(link_id)
    );
}

/// Sends `/meta` under a requester cap of `requester_max` and returns the receipt verdict
/// and the responder's tracked outcome for that one request.
#[deny(clippy::await_holding_invalid_type)]
async fn meta_response_under_requester_cap(
    pair: &LoopbackRequestPair,
    outcomes: &mut tokio::sync::mpsc::UnboundedReceiver<ResponseOutcomeReceiver>,
    requester_max: usize,
) -> (Result<RequestResponseData, RequestError>, ResponseOutcome) {
    let receipt = request_with(
        pair,
        "/meta",
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
async fn two_transport_loopback_request_equal_requester_and_responder_caps_compose() {
    install_test_logger();
    let mut pair = loopback_request_pair("request-response-cap-probe2-both-ends").await;
    let link_id = pair.handles.link_id;
    let data = random_payload(96);
    let metadata = rmpv::Value::from("named by the responder");
    let measure = resource_response_len(&data, &metadata);
    set_max_response_size(&pair, measure).await;
    let (outcome_tx, mut outcomes) = tokio::sync::mpsc::unbounded_channel();
    let expected = data.clone();
    register_handler(
        &pair,
        "/meta",
        RequestAllow::All,
        tracking_handler(outcome_tx, move |_| {
            Some(RequestResponse::Resource { data: data.clone(), metadata: Some(metadata.clone()) })
        }),
    )
    .await;

    let (verdict, outcome) = meta_response_under_requester_cap(&pair, &mut outcomes, measure).await;
    let response = verdict.expect("a requester cap equal to the responder's admits the response");
    assert_eq!(response.data(), Some(expected.as_slice()));
    assert_eq!(outcome, ResponseOutcome::Resource(OutboundResult::Complete));

    let (verdict, outcome) =
        meta_response_under_requester_cap(&pair, &mut outcomes, measure - 1).await;
    assert_eq!(
        verdict,
        Err(RequestError::Oversize),
        "a requester cap one under the responder's measure {measure} rejects the advertisement"
    );
    assert_eq!(
        outcome,
        ResponseOutcome::Resource(OutboundResult::Rejected(RejectReason::PeerCancelled)),
        "the responder let the response through and saw the requester's reject"
    );
    assert!(
        refused_response_records_for(link_id).is_empty(),
        "the responder's own cap refused nothing: {:?}",
        refused_response_records_for(link_id)
    );

    // The two transfers above left their packets in the connector tap; the silence check
    // below reads the tap and the resource events from empty.
    drain_connector_tap(&mut pair).await;
    let mut resource_events = pair.handles.connector.transport.resource_events();
    let max = measure - 1;
    set_max_response_size(&pair, max).await;
    let id = send_request(&pair, "/meta", rmpv::Value::Nil).await;
    assert_eq!(await_outcome(&mut outcomes).await, ResponseOutcome::TooLarge { len: measure, max });
    assert_nothing_sent_for(&mut pair, &mut resource_events, id).await;
    assert_one_refusal_warn(link_id, id, measure, max);
}
