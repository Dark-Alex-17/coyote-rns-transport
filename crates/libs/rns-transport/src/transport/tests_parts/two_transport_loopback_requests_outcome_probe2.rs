// Usage probes for `InboundRequest::track_outcome` in the shapes a consumer reaches for
// that the verdict table and the first probe file leave untried: the README's own pattern,
// a task the handler spawns to await the receiver while the handler returns at once; two
// requests in flight on one link whose handler answers one and stays silent on the other,
// each receiver resolving for its own request; and a request that arrived as a request
// Resource rather than one packet, tracked like any other; and a handler that panics after
// tracking, whose receiver resolves `NotSent` because the sender went down with the task.
// Helpers come from the sibling `tests_parts` files.

/// The handler keeps nothing of the receiver: it spawns a task that awaits the verdict and
/// forwards it, then returns its response without waiting. This is the shape the README's
/// "awaited outside the handler" sentence describes.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_outcome_awaited_in_a_task_the_handler_spawned() {
    let mut pair = loopback_request_pair("request-outcome-probe2-spawned").await;
    let (verdict_tx, mut verdicts) = tokio::sync::mpsc::unbounded_channel::<ResponseOutcome>();
    register_handler(&pair, "/echo", RequestAllow::All, move |mut request: InboundRequest| {
        let receiver = request.track_outcome();
        let verdict_tx = verdict_tx.clone();
        tokio::spawn(async move {
            let verdict = receiver.await;
            verdict_tx.send(verdict).expect("test receiver is alive");
        });
        std::future::ready(Some(RequestResponse::Value(request.into_data())))
    })
    .await;

    let id = send_request(&pair, "/echo", rmpv::Value::from("spawned")).await;
    assert_eq!(await_response(&mut pair, id).await, rmpv::Value::from("spawned"));
    let verdict = timeout(RESPONSE_TIMEOUT, verdicts.recv())
        .await
        .expect("the spawned task reports within the packet budget")
        .expect("the spawned task reported");
    assert_eq!(verdict, ResponseOutcome::Packet);
}

/// Two requests reach the same handler and are both tracked before either is answered: the
/// handler parks on a zero-permit `gate` until the test has both receivers in hand and
/// adds the permits, then answers the request that asked to be answered and stays silent
/// on the other. Each receiver carries its own request's verdict, so the silent one
/// resolves `NotSent` while the answered one resolves `Packet` and its response names the
/// right request id.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_outcomes_stay_with_their_own_request_when_two_are_in_flight(
) {
    let mut pair = loopback_request_pair("request-outcome-probe2-two-in-flight").await;
    let gate = Arc::new(tokio::sync::Semaphore::new(0));
    let (tracked_tx, mut tracked) =
        tokio::sync::mpsc::unbounded_channel::<(String, ResponseOutcomeReceiver)>();
    let handler_gate = gate.clone();
    register_handler(&pair, "/mixed", RequestAllow::All, move |mut request: InboundRequest| {
        let tag = request.data().as_str().expect("the probe sends strings").to_owned();
        tracked_tx.send((tag, request.track_outcome())).expect("test receiver is alive");
        let gate = handler_gate.clone();
        async move {
            let _permit = gate.acquire().await.expect("the gate is never closed");
            if request.data().as_str() == Some("answer") {
                Some(RequestResponse::Value(request.into_data()))
            } else {
                None
            }
        }
    })
    .await;

    let answer_id = send_request(&pair, "/mixed", rmpv::Value::from("answer")).await;
    let quiet_id = send_request(&pair, "/mixed", rmpv::Value::from("quiet")).await;
    assert_ne!(quiet_id, answer_id, "two distinct requests are in flight");

    let mut receivers = std::collections::HashMap::new();
    for _ in 0..2 {
        let (data, receiver) = timeout(RESPONSE_TIMEOUT, tracked.recv())
            .await
            .expect("both handlers run concurrently and track before the gate")
            .expect("handler tracked the outcome");
        receivers.insert(data, receiver);
    }
    gate.add_permits(2);

    let answered = receivers.remove("answer").expect("answered request tracked");
    let silenced = receivers.remove("quiet").expect("quiet request tracked");
    assert_eq!(
        timeout(RESPONSE_TIMEOUT, silenced).await.expect("quiet verdict resolves"),
        ResponseOutcome::NotSent,
        "the silent request's receiver is the one that reads NotSent"
    );
    assert_eq!(
        timeout(RESPONSE_TIMEOUT, answered).await.expect("answered verdict resolves"),
        ResponseOutcome::Packet,
        "the answered request's receiver is the one that reads Packet"
    );
    assert_eq!(await_response(&mut pair, answer_id).await, rmpv::Value::from("answer"));
}

/// A request over the link MDU travels as a request Resource and is handed to the same
/// handler once assembled; `track_outcome` on it resolves exactly as for a packet request,
/// `Packet` for a short reply that names the recomputed request id.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_request_outcome_is_packet_for_a_short_reply()
{
    let mut pair = loopback_request_pair("request-outcome-probe2-resource-form").await;
    let (outcome_tx, mut outcomes) = tokio::sync::mpsc::unbounded_channel();
    register_handler(
        &pair,
        "/len",
        RequestAllow::All,
        tracking_handler(outcome_tx, |request| {
            let len = match request.into_data() {
                rmpv::Value::Binary(body) => body.len(),
                other => panic!("the resource-form request carried {other:?}"),
            };
            Some(RequestResponse::Value(rmpv::Value::from(len as u64)))
        }),
    )
    .await;

    let body = over_mdu_body(&pair).await;
    let expected_len = body.len() as u64;
    let (id, _) = send_request_as_resource(&pair, "/len", rmpv::Value::Binary(body)).await;
    assert_eq!(
        await_outcome_within(&mut outcomes, RESOURCE_TIMEOUT).await,
        ResponseOutcome::Packet
    );
    assert_eq!(await_response(&mut pair, id).await, rmpv::Value::from(expected_len));
}

/// The panic takes the response task down and the sender with it; the dispatcher never
/// reaches its resolution step, so the receiver resolves through the dropped-sender rule
/// and a sibling request on the same link is still answered.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_handler_panicking_after_tracking_resolves_not_sent() {
    let mut pair = loopback_request_pair("request-outcome-probe2-panic").await;
    let (outcome_tx, mut outcomes) = tokio::sync::mpsc::unbounded_channel();
    register_handler(&pair, "/panic", RequestAllow::All, move |mut request: InboundRequest| {
        outcome_tx.send(request.track_outcome()).expect("test receiver is alive");
        async { panic!("handler panics on purpose after tracking") }
    })
    .await;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;

    send_request(&pair, "/panic", rmpv::Value::Nil).await;
    assert_eq!(await_outcome(&mut outcomes).await, ResponseOutcome::NotSent);

    let id = send_request(&pair, "/echo", rmpv::Value::from("after panic")).await;
    assert_eq!(await_response(&mut pair, id).await, rmpv::Value::from("after panic"));
}
