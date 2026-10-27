// Loopback coverage for `InboundRequest::track_outcome`: a packet response resolves
// `Packet` once it has left on an interface, a response that travels as a resource (the
// `Resource` form or a `Bytes` or `Value` body over the link MDU) resolves `Resource` with
// the transfer's own verdict, including the verdict of a transfer the requester tears the
// link out from under; `None` from the handler, a packet that reaches no interface and a
// responder that stops with the transfer in flight all resolve `NotSent`; a second
// `track_outcome` call retires the first receiver with `NotSent`, and a receiver whose
// sender went away without the dispatcher running resolves `NotSent` instead of hanging.
// Helpers come from the sibling `tests_parts` files.

use crate::destination::request::{
    empty_outcome_slot, RequestFrame, ResponseOutcome, ResponseOutcomeReceiver,
};

/// A handler that tracks the outcome of every request before `respond` decides the
/// answer, and hands each receiver to the test over `outcomes`.
fn tracking_handler(
    outcomes: tokio::sync::mpsc::UnboundedSender<ResponseOutcomeReceiver>,
    respond: impl Fn(InboundRequest) -> Option<RequestResponse> + Send + Sync,
) -> impl Fn(InboundRequest) -> std::future::Ready<Option<RequestResponse>> + Send + Sync {
    move |mut request: InboundRequest| {
        outcomes.send(request.track_outcome()).expect("test receiver is alive");
        std::future::ready(respond(request))
    }
}

/// Takes the next receiver the handler handed out and waits for its verdict within the
/// packet response budget.
#[deny(clippy::await_holding_invalid_type)]
async fn await_outcome(
    outcomes: &mut tokio::sync::mpsc::UnboundedReceiver<ResponseOutcomeReceiver>,
) -> ResponseOutcome {
    await_outcome_within(outcomes, RESPONSE_TIMEOUT).await
}

/// Takes the next receiver the handler handed out and waits up to `budget` for its verdict.
#[deny(clippy::await_holding_invalid_type)]
async fn await_outcome_within(
    outcomes: &mut tokio::sync::mpsc::UnboundedReceiver<ResponseOutcomeReceiver>,
    budget: Duration,
) -> ResponseOutcome {
    let receiver = outcomes.recv().await.expect("handler tracked the outcome");
    timeout(budget, receiver).await.expect("outcome resolves")
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_packet_response_outcome_is_packet() {
    let mut pair = loopback_request_pair("request-outcome-packet").await;
    let (outcome_tx, mut outcomes) = tokio::sync::mpsc::unbounded_channel();
    register_handler(
        &pair,
        "/echo",
        RequestAllow::All,
        tracking_handler(outcome_tx, |request| {
            Some(RequestResponse::Value(request.into_data()))
        }),
    )
    .await;

    let id = send_request(&pair, "/echo", rmpv::Value::from("tracked")).await;
    assert_eq!(await_response(&mut pair, id).await, rmpv::Value::from("tracked"));
    assert_eq!(await_outcome(&mut outcomes).await, ResponseOutcome::Packet);
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_outcome_is_the_transfer_verdict() {
    let pair = loopback_request_pair("request-outcome-resource-form").await;
    let mut resource_events = pair.handles.connector.transport.resource_events();
    let link_id = pair.handles.link_id;
    let (outcome_tx, mut outcomes) = tokio::sync::mpsc::unbounded_channel();
    register_handler(
        &pair,
        "/file",
        RequestAllow::All,
        tracking_handler(outcome_tx, |_| {
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
        await_outcome(&mut outcomes).await,
        ResponseOutcome::Resource(OutboundResult::Complete)
    );
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_oversize_bytes_outcome_is_the_transfer_verdict() {
    let pair = loopback_request_pair("request-outcome-oversize").await;
    let body = over_mdu_zeros(&pair).await;
    let (outcome_tx, mut outcomes) = tokio::sync::mpsc::unbounded_channel();
    register_handler(
        &pair,
        "/big",
        RequestAllow::All,
        tracking_handler(outcome_tx, move |_| Some(RequestResponse::Bytes(body.clone()))),
    )
    .await;

    send_request(&pair, "/big", rmpv::Value::Nil).await;
    assert_eq!(
        await_outcome_within(&mut outcomes, RESOURCE_TIMEOUT).await,
        ResponseOutcome::Resource(OutboundResult::Complete)
    );
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_oversize_value_outcome_is_the_transfer_verdict() {
    let pair = loopback_request_pair("request-outcome-oversize-value").await;
    let text = "v".repeat(pair.handles.link.lock().await.link_mdu() + 1);
    let (outcome_tx, mut outcomes) = tokio::sync::mpsc::unbounded_channel();
    register_handler(
        &pair,
        "/big-text",
        RequestAllow::All,
        tracking_handler(outcome_tx, move |_| {
            Some(RequestResponse::Value(rmpv::Value::String(text.clone().into())))
        }),
    )
    .await;

    send_request(&pair, "/big-text", rmpv::Value::Nil).await;
    assert_eq!(
        await_outcome_within(&mut outcomes, RESOURCE_TIMEOUT).await,
        ResponseOutcome::Resource(OutboundResult::Complete)
    );
}

/// `reset_out_link` is local to the requester: nothing goes over the wire, so the responder
/// learns of the loss through the acks that stop arriving and its transfer runs out of
/// retries. If the responder's own link sweep gets there first the verdict is `LinkClosed`
/// instead; either way the tracked outcome carries the transfer's failure.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_response_outcome_follows_a_requester_link_reset()
{
    let pair = loopback_request_pair("request-outcome-link-reset").await;
    let destination = outcome_out_link_destination(&pair.handles.connector.transport).await;
    let mut resource_events = pair.handles.connector.transport.resource_events();
    let link_id = pair.handles.link_id;
    let payload = random_payload(4 * MAX_EFFICIENT_SIZE + 1);
    let (outcome_tx, mut outcomes) = tokio::sync::mpsc::unbounded_channel();
    register_handler(
        &pair,
        "/long",
        RequestAllow::All,
        tracking_handler(outcome_tx, move |_| {
            Some(RequestResponse::Resource { data: payload.clone(), metadata: None })
        }),
    )
    .await;

    send_request(&pair, "/long", rmpv::Value::Nil).await;
    await_resource_event(&mut resource_events, |event| {
        event.link_id == link_id && matches!(event.kind, ResourceEventKind::Progress(_))
    })
    .await;
    pair.handles.connector.transport.reset_out_link(&destination).await;

    let verdict = await_outcome_within(&mut outcomes, RESOURCE_TIMEOUT).await;
    assert!(
        matches!(
            verdict,
            ResponseOutcome::Resource(OutboundResult::Failed(
                OutboundFailure::RetriesExhausted | OutboundFailure::LinkClosed
            ))
        ),
        "a requester link reset under the transfer fails it, got {verdict:?}"
    );
}

/// The teardown goes over the wire, so the responder's in-link closes and its link cleanup
/// fails the in-flight transfer as `LinkClosed`.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_response_outcome_follows_a_requester_link_teardown(
) {
    let pair = loopback_request_pair("request-outcome-link-teardown").await;
    let mut resource_events = pair.handles.connector.transport.resource_events();
    let link_id = pair.handles.link_id;
    let payload = random_payload(4 * MAX_EFFICIENT_SIZE + 1);
    let (outcome_tx, mut outcomes) = tokio::sync::mpsc::unbounded_channel();
    register_handler(
        &pair,
        "/long-teardown",
        RequestAllow::All,
        tracking_handler(outcome_tx, move |_| {
            Some(RequestResponse::Resource { data: payload.clone(), metadata: None })
        }),
    )
    .await;

    send_request(&pair, "/long-teardown", rmpv::Value::Nil).await;
    await_resource_event(&mut resource_events, |event| {
        event.link_id == link_id && matches!(event.kind, ResourceEventKind::Progress(_))
    })
    .await;
    let packet = pair.handles.link.lock().await.teardown().expect("an active link closes");
    let outcome = pair
        .handles
        .connector
        .transport
        .send_link_packet_on_bound_iface(&pair.handles.link, packet)
        .await;
    assert_eq!(outcome, SendPacketOutcome::SentDirect, "link close reaches the wire");

    let verdict = await_outcome_within(&mut outcomes, RESOURCE_TIMEOUT).await;
    assert_eq!(
        verdict,
        ResponseOutcome::Resource(OutboundResult::Failed(OutboundFailure::LinkClosed)),
        "a requester teardown under the transfer closes it, got {verdict:?}"
    );
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_silent_handler_outcome_is_not_sent() {
    let mut pair = loopback_request_pair("request-outcome-silent").await;
    let (outcome_tx, mut outcomes) = tokio::sync::mpsc::unbounded_channel();
    register_handler(&pair, "/quiet", RequestAllow::All, tracking_handler(outcome_tx, |_| None))
        .await;

    send_request(&pair, "/quiet", rmpv::Value::Nil).await;
    assert_eq!(await_outcome(&mut outcomes).await, ResponseOutcome::NotSent);
    assert_no_response(&mut pair).await;
}

/// The handler repoints the in-link at an interface the manager does not know, so the
/// response packet's direct send matches no interface. The original interface is put back
/// once the verdict is in, so the silence check can still bound its window with a data
/// round trip on the link.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_packet_response_reaching_no_interface_is_not_sent() {
    let mut pair = loopback_request_pair("request-outcome-no-interface").await;
    let in_link = pair
        .handles
        .listener
        .transport
        .find_in_link(&pair.handles.link_id)
        .await
        .expect("responder tracks the inbound link");
    let ingress_iface = in_link.lock().await.ingress_iface().expect("the in-link has an ingress");
    let (outcome_tx, mut outcomes) = tokio::sync::mpsc::unbounded_channel();
    let stranded_link = in_link.clone();
    register_handler(&pair, "/stranded", RequestAllow::All, move |mut request: InboundRequest| {
        outcome_tx.send(request.track_outcome()).expect("test receiver is alive");
        let link = stranded_link.clone();
        async move {
            link.lock().await.set_ingress_iface(AddressHash::new_from_rand(OsRng));
            Some(RequestResponse::Value(rmpv::Value::from("stranded")))
        }
    })
    .await;

    send_request(&pair, "/stranded", rmpv::Value::Nil).await;
    assert_eq!(await_outcome(&mut outcomes).await, ResponseOutcome::NotSent);
    in_link.lock().await.set_ingress_iface(ingress_iface);
    assert_no_response(&mut pair).await;
}

/// The connector's handler lock is held from right after the request leaves until the
/// verdict is in, so the response Resource stalls at whichever step next needs that lock
/// and is still in flight when the responder stops.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_response_in_flight_at_stop_is_not_sent() {
    let pair = loopback_request_pair("request-outcome-stop").await;
    let payload = random_payload(2 * MAX_EFFICIENT_SIZE);
    let (outcome_tx, mut outcomes) = tokio::sync::mpsc::unbounded_channel();
    register_handler(
        &pair,
        "/stalled",
        RequestAllow::All,
        tracking_handler(outcome_tx, move |_| {
            Some(RequestResponse::Resource { data: payload.clone(), metadata: None })
        }),
    )
    .await;

    send_request(&pair, "/stalled", rmpv::Value::Nil).await;
    let (stall, release) =
        stall_transport_until_released(&pair.handles.connector.transport).await;
    let receiver = outcomes.recv().await.expect("handler tracked the outcome");
    pair.handles.listener.transport.stop().await;

    let verdict = timeout(RESOURCE_TIMEOUT, receiver).await.expect("outcome resolves");
    assert_eq!(verdict, ResponseOutcome::NotSent);
    release.send(()).expect("stall task is waiting for the release");
    stall.await.expect("stall task finished");
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_second_track_outcome_call_retires_the_first_receiver() {
    let mut pair = loopback_request_pair("request-outcome-retracked").await;
    let (outcome_tx, mut outcomes) = tokio::sync::mpsc::unbounded_channel();
    register_handler(&pair, "/echo", RequestAllow::All, move |mut request: InboundRequest| {
        outcome_tx.send(request.track_outcome()).expect("test receiver is alive");
        outcome_tx.send(request.track_outcome()).expect("test receiver is alive");
        std::future::ready(Some(RequestResponse::Value(request.into_data())))
    })
    .await;

    let id = send_request(&pair, "/echo", rmpv::Value::from("twice")).await;
    assert_eq!(await_response(&mut pair, id).await, rmpv::Value::from("twice"));
    assert_eq!(await_outcome(&mut outcomes).await, ResponseOutcome::NotSent, "replaced");
    assert_eq!(await_outcome(&mut outcomes).await, ResponseOutcome::Packet, "current");
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn request_outcome_receiver_resolves_not_sent_when_its_request_is_dropped_undispatched() {
    let mut request = InboundRequest::new(
        "/echo".to_owned(),
        RequestFrame::new(0.0, PathHash::of("/echo"), rmpv::Value::Nil),
        RequestId::new([7u8; 16]),
        AddressHash::new_from_rand(OsRng),
        None,
        empty_outcome_slot(),
    );
    let receiver = request.track_outcome();
    drop(request);
    assert_eq!(receiver.await, ResponseOutcome::NotSent);
}
