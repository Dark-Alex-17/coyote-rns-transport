// Usage probe for the "every path that drops the sender" rule behind
// `ResponseOutcome::NotSent` in the composition the verdict table and the earlier probe
// files leave untried: a handler that panics after tracking while a second tracked request
// on the same link is still in flight (the panic takes down its own sender and no other).
// Helpers come from the sibling `tests_parts` files.

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
