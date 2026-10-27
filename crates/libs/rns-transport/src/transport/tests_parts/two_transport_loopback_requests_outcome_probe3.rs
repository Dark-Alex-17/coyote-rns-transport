// Usage probes for the "every path that drops the sender" rule behind
// `ResponseOutcome::NotSent` in the compositions the verdict table and the earlier probe
// files leave untried: a handler that panics after tracking while a second tracked request
// on the same link is still in flight (the panic takes down its own sender and no other);
// a request that arrived as a request Resource whose handler panics after tracking (the
// Resource dispatch entry reaps the panic like the packet entry does); and the README's
// "awaited outside the handler" shape under a multi-thread runtime where the awaiting task
// and the panicking handler run on different workers. Helpers come from the sibling
// `tests_parts` files.

/// Two tracked requests are in flight on one link, each parked on its own zero-permit
/// gate. The first is released and panics; its receiver resolves `NotSent` through the
/// dropped-sender rule. The second is still parked while the dispatcher reaps that panic,
/// and only then released: its receiver resolves `Packet` and its response arrives, so the
/// panic dropped exactly one sender and the reaping touched no sibling task.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_panic_after_tracking_leaves_a_parked_tracked_sibling_alone()
{
    let mut pair = loopback_request_pair("request-outcome-probe3-panic-sibling").await;
    let boom_gate = Arc::new(tokio::sync::Semaphore::new(0));
    let answer_gate = Arc::new(tokio::sync::Semaphore::new(0));
    let (tracked_tx, mut tracked) =
        tokio::sync::mpsc::unbounded_channel::<(String, ResponseOutcomeReceiver)>();
    let handler_boom_gate = boom_gate.clone();
    let handler_answer_gate = answer_gate.clone();
    register_handler(&pair, "/mixed", RequestAllow::All, move |mut request: InboundRequest| {
        let tag = request.data().as_str().expect("the probe sends strings").to_owned();
        tracked_tx.send((tag.clone(), request.track_outcome())).expect("test receiver is alive");
        let boom_gate = handler_boom_gate.clone();
        let answer_gate = handler_answer_gate.clone();
        async move {
            if tag == "boom" {
                let _permit = boom_gate.acquire().await.expect("the gate is never closed");
                panic!("handler panics on purpose after tracking");
            }
            let _permit = answer_gate.acquire().await.expect("the gate is never closed");
            Some(RequestResponse::Value(request.into_data()))
        }
    })
    .await;

    let answer_id = send_request(&pair, "/mixed", rmpv::Value::from("answer")).await;
    let boom_id = send_request(&pair, "/mixed", rmpv::Value::from("boom")).await;
    assert_ne!(boom_id, answer_id, "two distinct requests are in flight");

    let mut receivers = std::collections::HashMap::new();
    for _ in 0..2 {
        let (tag, receiver) = timeout(RESPONSE_TIMEOUT, tracked.recv())
            .await
            .expect("both handlers run concurrently and track before their gates")
            .expect("handler tracked the outcome");
        receivers.insert(tag, receiver);
    }
    let boomed = receivers.remove("boom").expect("panicking request tracked");
    let answered = receivers.remove("answer").expect("answered request tracked");

    boom_gate.add_permits(1);
    assert_eq!(
        timeout(RESPONSE_TIMEOUT, boomed).await.expect("panicked verdict resolves"),
        ResponseOutcome::NotSent,
        "the panicking request's receiver reads NotSent"
    );
    // Give the dispatcher loop a turn to reap the panicked task before the sibling moves.
    for _ in 0..8 {
        tokio::task::yield_now().await;
    }

    answer_gate.add_permits(1);
    assert_eq!(
        timeout(RESPONSE_TIMEOUT, answered).await.expect("answered verdict resolves"),
        ResponseOutcome::Packet,
        "the sibling parked through the panic still reads Packet"
    );
    assert_eq!(await_response(&mut pair, answer_id).await, rmpv::Value::from("answer"));
}

/// A request over the link MDU arrives as a request Resource and is dispatched through
/// the Resource entry; a handler that tracks it and then panics drops the sender the same
/// way, so the receiver resolves `NotSent` and a packet request on the link is still
/// answered afterwards.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_request_panic_after_tracking_resolves_not_sent(
) {
    let mut pair = loopback_request_pair("request-outcome-probe3-resource-panic").await;
    let (outcome_tx, mut outcomes) = tokio::sync::mpsc::unbounded_channel();
    register_handler(&pair, "/panic", RequestAllow::All, move |mut request: InboundRequest| {
        outcome_tx.send(request.track_outcome()).expect("test receiver is alive");
        assert!(
            matches!(request.data(), rmpv::Value::Binary(_)),
            "the resource-form request carried its binary body"
        );
        async { panic!("handler panics on purpose after tracking a resource-form request") }
    })
    .await;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;

    let body = over_mdu_body(&pair).await;
    let _ = send_request_as_resource(&pair, "/panic", rmpv::Value::Binary(body)).await;
    assert_eq!(
        await_outcome_within(&mut outcomes, RESOURCE_TIMEOUT).await,
        ResponseOutcome::NotSent
    );

    let id = send_request(&pair, "/echo", rmpv::Value::from("after resource panic")).await;
    assert_eq!(await_response(&mut pair, id).await, rmpv::Value::from("after resource panic"));
}

/// The README's shape on a multi-thread runtime: the handler spawns a task that awaits the
/// receiver and forwards the verdict, then panics on whichever worker it runs on. The
/// awaiting task, possibly on the other worker, reads `NotSent`, and a sibling request on
/// the link is still answered.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_panic_after_tracking_reaches_a_spawned_awaiter_multi_thread(
) {
    let mut pair = loopback_request_pair("request-outcome-probe3-panic-mt").await;
    let (verdict_tx, mut verdicts) = tokio::sync::mpsc::unbounded_channel::<ResponseOutcome>();
    register_handler(&pair, "/panic", RequestAllow::All, move |mut request: InboundRequest| {
        let receiver = request.track_outcome();
        let verdict_tx = verdict_tx.clone();
        tokio::spawn(async move {
            let verdict = receiver.await;
            verdict_tx.send(verdict).expect("test receiver is alive");
        });
        async { panic!("handler panics on purpose after tracking on a worker thread") }
    })
    .await;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;

    send_request(&pair, "/panic", rmpv::Value::Nil).await;
    let verdict = timeout(RESPONSE_TIMEOUT, verdicts.recv())
        .await
        .expect("the spawned task reports within the packet budget")
        .expect("the spawned task reported");
    assert_eq!(verdict, ResponseOutcome::NotSent);

    let id = send_request(&pair, "/echo", rmpv::Value::from("after panic mt")).await;
    assert_eq!(await_response(&mut pair, id).await, rmpv::Value::from("after panic mt"));
}
