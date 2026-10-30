// Loopback coverage for `InboundRequest::into_parts`: a handler that takes the data by
// value and arms the tracker afterwards, still inside the handler, gets the same verdicts
// `track_outcome` gives for a packet response and a Resource-form response; a handler that
// drops the tracker unarmed still has its response delivered and leaves the dispatcher
// nothing to resolve, so no dropped-receiver line is logged. The positive control arms the
// slot and drops the receiver inside the handler, which makes the dispatcher's send fail
// deterministically and proves the logger captures that line. Helpers come from the
// sibling `tests_parts` files.

/// A handler that splits every request with `into_parts`, lets `respond` answer from the
/// owned data, then arms the tracker and hands the receiver to the test over `outcomes`.
fn into_parts_tracking_handler(
    outcomes: tokio::sync::mpsc::UnboundedSender<ResponseOutcomeReceiver>,
    respond: impl Fn(rmpv::Value) -> Option<RequestResponse> + Send + Sync,
) -> impl Fn(InboundRequest) -> std::future::Ready<Option<RequestResponse>> + Send + Sync {
    move |request: InboundRequest| {
        let (data, tracker) = request.into_parts();
        let response = respond(data);
        outcomes.send(tracker.outcome()).expect("test receiver is alive");
        std::future::ready(response)
    }
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_into_parts_packet_response_outcome_is_packet() {
    let mut pair = loopback_request_pair("request-into-parts-packet").await;
    let (outcome_tx, mut outcomes) = tokio::sync::mpsc::unbounded_channel();
    register_handler(
        &pair,
        "/echo",
        RequestAllow::All,
        into_parts_tracking_handler(outcome_tx, |data| Some(RequestResponse::Value(data))),
    )
    .await;

    let id = send_request(&pair, "/echo", rmpv::Value::from("split")).await;
    assert_eq!(await_response(&mut pair, id).await, rmpv::Value::from("split"));
    assert_eq!(await_outcome(&mut outcomes).await, ResponseOutcome::Packet);
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_into_parts_resource_form_outcome_is_the_transfer_verdict() {
    let pair = loopback_request_pair("request-into-parts-resource-form").await;
    let mut resource_events = pair.handles.connector.transport.resource_events();
    let link_id = pair.handles.link_id;
    let (outcome_tx, mut outcomes) = tokio::sync::mpsc::unbounded_channel();
    register_handler(
        &pair,
        "/file",
        RequestAllow::All,
        into_parts_tracking_handler(outcome_tx, |_| {
            Some(RequestResponse::Resource { data: b"as a resource".to_vec(), metadata: None })
        }),
    )
    .await;

    let id = send_request(&pair, "/file", rmpv::Value::Nil).await;
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
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_into_parts_unarmed_tracker_drop_logs_nothing() {
    install_test_logger();
    let mut pair = loopback_request_pair("request-into-parts-unarmed").await;
    let link_id = pair.handles.link_id;
    register_handler(&pair, "/echo", RequestAllow::All, |request: InboundRequest| {
        let (data, tracker) = request.into_parts();
        drop(tracker);
        std::future::ready(Some(RequestResponse::Value(data)))
    })
    .await;

    let id = send_request(&pair, "/echo", rmpv::Value::from("unwatched")).await;
    assert_eq!(await_response(&mut pair, id).await, rmpv::Value::from("unwatched"));
    // The requester can hold the response before the dispatcher reaches the point where it
    // would log, so let the dispatcher finish before asserting it stayed silent.
    tokio::time::sleep(Duration::from_millis(300)).await;
    let dropped = dispatch_outcome_receiver_dropped_records_for(link_id);
    assert!(dropped.is_empty(), "an unarmed tracker leaves the slot empty: {dropped:?}");
}

/// The receiver dies before the handler returns, so the sender the dispatcher takes has
/// nobody to deliver to and the send fails on the first try.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_armed_slot_with_a_dropped_receiver_logs_once() {
    install_test_logger();
    let mut pair = loopback_request_pair("request-into-parts-dropped-receiver").await;
    let link_id = pair.handles.link_id;
    register_handler(&pair, "/echo", RequestAllow::All, |mut request: InboundRequest| {
        drop(request.track_outcome());
        std::future::ready(Some(RequestResponse::Value(request.into_data())))
    })
    .await;

    let id = send_request(&pair, "/echo", rmpv::Value::from("unheard")).await;
    assert_eq!(await_response(&mut pair, id).await, rmpv::Value::from("unheard"));
    wait_until(
        "the dispatcher's dropped-receiver record",
        || !dispatch_outcome_receiver_dropped_records_for(link_id).is_empty(),
        || format!("{:?}", dispatch_outcome_receiver_dropped_records_for(link_id)),
    )
    .await;
    let dropped = dispatch_outcome_receiver_dropped_records_for(link_id);
    assert_eq!(dropped.len(), 1, "one record per dropped receiver: {dropped:?}");
    assert!(
        dropped[0].contains(&format!("response {id} on link {link_id}")),
        "the record names the request and the link: {dropped:?}"
    );
}
