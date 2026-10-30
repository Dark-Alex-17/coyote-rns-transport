// Loopback pins over the responder's `max_response_size` and the `into_parts` tracker:
// the cap in force is the one read when the request was dispatched, so a cap tightened
// while the handler runs lets that response out and refuses the next one; a tracker from
// `into_parts` armed after an earlier `track_outcome` retires that first receiver with
// `NotSent`; and the responder's Resource measure sits 3 bytes under the advertised size
// the requester's cap reads, so a requester cap equal to the responder's, or 2 bytes above
// it, refuses the metadata-bearing response the responder let through and one 3 bytes above
// accepts it. Helpers come from the sibling `tests_parts` files.

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
async fn two_transport_loopback_request_requester_cap_needs_three_bytes_over_the_responder_resource_measure(
) {
    install_test_logger();
    let pair = loopback_request_pair("request-response-cap-probe2-both-ends").await;
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

    for requester_max in [measure, measure + 2] {
        let (verdict, outcome) =
            meta_response_under_requester_cap(&pair, &mut outcomes, requester_max).await;
        assert_eq!(
            verdict,
            Err(RequestError::Oversize),
            "a requester cap of {requester_max} refuses what the responder measured at {measure}"
        );
        assert_eq!(
            outcome,
            ResponseOutcome::Resource(OutboundResult::Rejected(RejectReason::PeerCancelled)),
            "the responder let the response through and saw the requester's reject"
        );
    }

    let (verdict, outcome) =
        meta_response_under_requester_cap(&pair, &mut outcomes, measure + 3).await;
    let response = verdict.expect("3 bytes of headroom admits the advertisement");
    assert_eq!(response.data(), Some(expected.as_slice()));
    assert_eq!(outcome, ResponseOutcome::Resource(OutboundResult::Complete));
    assert!(
        refused_response_records_for(link_id).is_empty(),
        "the responder's own cap refused nothing: {:?}",
        refused_response_records_for(link_id)
    );
}
