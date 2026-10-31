// Usage-pattern pins over the responder's `max_response_size` and `into_parts`, written
// from the README and rustdoc before the dispatcher was read: clearing the cap lifts the
// gate for the next request; a `Resource` response is measured on its uncompressed bytes,
// so a body that bz2 would shrink below the cap is still refused when `auto_compress` is
// on; a `Resource` with no metadata measures exactly its data; a handler that never tracked
// still costs one warn and leaves the requester unanswered; a tracker from `into_parts`
// resolves `TooLarge` like `track_outcome` does; and a tracker armed after the handler
// returned resolves `NotSent` at once instead of hanging. Helpers come from the sibling
// `tests_parts` files.

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_clearing_max_response_size_lifts_the_gate() {
    install_test_logger();
    let mut pair = loopback_request_pair("request-response-cap-probe-clear").await;
    let link_id = pair.handles.link_id;
    let body = rmpv::Value::from("gated then free");
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

    let refused = send_request(&pair, "/capped", rmpv::Value::Nil).await;
    assert_eq!(await_outcome(&mut outcomes).await, ResponseOutcome::TooLarge { len, max });
    assert_no_response(&mut pair).await;

    pair.handles.destination.lock().await.clear_max_response_size();
    assert_eq!(pair.handles.destination.lock().await.max_response_size(), None);

    let answered = send_request(&pair, "/capped", rmpv::Value::Nil).await;
    assert_ne!(answered, refused, "a fresh request id for the second send");
    assert_eq!(await_response(&mut pair, answered).await, rmpv::Value::from("gated then free"));
    assert_eq!(await_outcome(&mut outcomes).await, ResponseOutcome::Packet);
    assert_one_refusal_warn(link_id, refused, len, max);
}

/// Zeros compress to a few bytes under bz2, so a cap sitting one byte under the raw payload
/// tells whether the dispatcher measured before or after `auto_compress` ran.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_compressible_resource_over_the_cap_is_measured_uncompressed(
) {
    install_test_logger();
    let mut pair = loopback_request_pair("request-response-cap-probe-compressible").await;
    let mut resource_events = pair.handles.connector.transport.resource_events();
    let link_id = pair.handles.link_id;
    let data = vec![0u8; 4096];
    let len = data.len();
    let max = len - 1;
    set_max_response_size(&pair, max).await;
    let (outcome_tx, mut outcomes) = tokio::sync::mpsc::unbounded_channel();
    register_handler_with_compression(
        &pair,
        "/zeros",
        true,
        tracking_handler(outcome_tx, move |_| {
            Some(RequestResponse::Resource { data: data.clone(), metadata: None })
        }),
    )
    .await;

    let id = send_request(&pair, "/zeros", rmpv::Value::Nil).await;
    assert_eq!(await_outcome(&mut outcomes).await, ResponseOutcome::TooLarge { len, max });
    assert_nothing_sent_for(&mut pair, &mut resource_events, id).await;
    assert_one_refusal_warn(link_id, id, len, max);
}

/// With no metadata there is nothing packed beside the data, so the measured size is the
/// data length and a cap exactly there lets the transfer start.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_without_metadata_measures_its_data_at_the_cap() {
    install_test_logger();
    let pair = loopback_request_pair("request-response-cap-probe-no-metadata").await;
    let mut resource_events = pair.handles.connector.transport.resource_events();
    let link_id = pair.handles.link_id;
    let data = random_payload(96);
    set_max_response_size(&pair, resource_response_len(&data, None)).await;
    let (outcome_tx, mut outcomes) = tokio::sync::mpsc::unbounded_channel();
    register_handler(
        &pair,
        "/raw",
        RequestAllow::All,
        tracking_handler(outcome_tx, move |_| {
            Some(RequestResponse::Resource { data: data.clone(), metadata: None })
        }),
    )
    .await;

    let id = send_request(&pair, "/raw", rmpv::Value::Nil).await;
    let complete = await_resource_event(&mut resource_events, |event| {
        event.link_id == link_id && matches!(event.kind, ResourceEventKind::Complete(_))
    })
    .await;
    let complete = complete_payload(complete);
    assert_eq!(complete.request_id.as_deref(), Some(id.as_bytes().as_slice()));
    assert_eq!(
        await_outcome_within(&mut outcomes, RESOURCE_TIMEOUT).await,
        ResponseOutcome::Resource(OutboundResult::Complete)
    );
    assert!(refused_response_records_for(link_id).is_empty(), "nothing was refused");
}

/// The README: a handler that did not track is told nothing and the requester sees an
/// unanswered request; the warn is the only trace the refusal leaves.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_untracked_response_over_the_cap_warns_once_and_answers_nothing(
) {
    install_test_logger();
    let mut pair = loopback_request_pair("request-response-cap-probe-untracked").await;
    let link_id = pair.handles.link_id;
    let body = rmpv::Value::from("nobody is watching");
    let len = packet_response_len(body.clone());
    let max = len - 1;
    set_max_response_size(&pair, max).await;
    register_handler(&pair, "/quiet", RequestAllow::All, move |_: InboundRequest| {
        std::future::ready(Some(RequestResponse::Value(body.clone())))
    })
    .await;

    let id = send_request(&pair, "/quiet", rmpv::Value::Nil).await;
    assert_no_response(&mut pair).await;
    assert_one_refusal_warn(link_id, id, len, max);
    assert!(
        dispatch_outcome_receiver_dropped_records_for(link_id).is_empty(),
        "an untracked refusal has no receiver to drop"
    );
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_into_parts_tracker_resolves_too_large() {
    install_test_logger();
    let mut pair = loopback_request_pair("request-response-cap-probe-into-parts").await;
    let link_id = pair.handles.link_id;
    let body = rmpv::Value::from("split and refused");
    let len = packet_response_len(body.clone());
    let max = len - 1;
    set_max_response_size(&pair, max).await;
    let (outcome_tx, mut outcomes) = tokio::sync::mpsc::unbounded_channel();
    register_handler(
        &pair,
        "/split",
        RequestAllow::All,
        into_parts_tracking_handler(outcome_tx, move |_| {
            Some(RequestResponse::Value(body.clone()))
        }),
    )
    .await;

    let id = send_request(&pair, "/split", rmpv::Value::Nil).await;
    assert_eq!(await_outcome(&mut outcomes).await, ResponseOutcome::TooLarge { len, max });
    assert_no_response(&mut pair).await;
    assert_one_refusal_warn(link_id, id, len, max);
}

/// The tracker docs: armed after the handler returned, the sender is never taken and the
/// receiver resolves `NotSent`. Unlike a stashed `InboundRequest`, `outcome` consumes the
/// tracker, so nothing keeps the slot alive once the dispatcher is done and the verdict
/// arrives without the test dropping anything.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_into_parts_tracker_armed_after_the_handler_returned_resolves_not_sent(
) {
    install_test_logger();
    let mut pair = loopback_request_pair("request-response-cap-probe-late-tracker").await;
    let link_id = pair.handles.link_id;
    let (tracker_tx, mut trackers) = tokio::sync::mpsc::unbounded_channel();
    register_handler(&pair, "/stash", RequestAllow::All, move |request: InboundRequest| {
        let (data, tracker) = request.into_parts();
        tracker_tx.send(tracker).expect("test receiver is alive");
        std::future::ready(Some(RequestResponse::Value(data)))
    })
    .await;

    let id = send_request(&pair, "/stash", rmpv::Value::from("kept")).await;
    assert_eq!(await_response(&mut pair, id).await, rmpv::Value::from("kept"));
    let tracker = trackers.recv().await.expect("the handler handed out its tracker");
    assert_eq!(format!("{tracker:?}"), "ResponseOutcomeTracker { .. }");

    let receiver = tracker.outcome();
    assert_eq!(
        timeout(RESPONSE_TIMEOUT, receiver).await.expect("a late tracker still resolves"),
        ResponseOutcome::NotSent
    );
    assert!(
        dispatch_outcome_receiver_dropped_records_for(link_id).is_empty(),
        "the dispatcher never took the late sender, so it logs nothing"
    );
}
