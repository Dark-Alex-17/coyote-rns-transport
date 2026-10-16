// Usage-pattern coverage for the requester side of the request layer: `Transport::request`
// and `RequestReceipt` as a consumer drives them. A response Resource over
// `max_response_size` fails the receipt `Oversize` at its advertisement
// (`RNS/Link.py:874-876`) and leaves the link usable; the id a receipt reports
// is the id the responder derives, in both forms (`RNS/Link.py:496-510`); several requests in
// flight on one link resolve independently; `concluded_at()` is empty until the receipt
// concludes; the default timeout really fires at `rtt * 6 + 11.25 s` (`RNS/Link.py:491`)
// and a zero override fires at the next link check; a receipt outlives the transport that
// issued it without hanging `response()`. Helpers come from the sibling `tests_parts` files.

/// Bound for a receipt that must conclude on the next link check tick.
fn one_tick_bound() -> Duration {
    INTERVAL_LINKS_CHECK + Duration::from_millis(500)
}

/// Registers an echo handler at `path` on the responder and returns the channel on which it
/// reports the id of every request it serves.
#[deny(clippy::await_holding_invalid_type)]
async fn register_recording_echo(
    pair: &LoopbackRequestPair,
    path: &str,
) -> tokio::sync::mpsc::UnboundedReceiver<RequestId> {
    let (seen_tx, seen_rx) = tokio::sync::mpsc::unbounded_channel();
    register_handler(pair, path, RequestAllow::All, recording_echo_handler(seen_tx)).await;
    seen_rx
}

/// `RNS/Link.py:874-876`: a response Resource whose advertised size exceeds
/// `max_response_size` is refused at its advertisement; the receipt fails `Oversize` without
/// ever reporting `Receiving`, and the link keeps answering afterwards.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_receipt_oversize_resource_response_fails_and_link_stays_usable(
) {
    let pair = loopback_request_pair("receipt-probe-oversize-resource").await;
    let body = over_mdu_body(&pair).await;
    register_handler(&pair, "/big", RequestAllow::All, move |_: InboundRequest| {
        std::future::ready(Some(RequestResponse::Bytes(body.clone())))
    })
    .await;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;

    let receipt = request_with(
        &pair,
        "/big",
        rmpv::Value::Nil,
        RequestOptions::default().with_max_response_size(1024),
    )
    .await;
    let seen = observe_statuses(&receipt).await;
    assert_eq!(await_receipt(&receipt).await, Err(RequestError::Oversize));
    assert_eq!(receipt.status(), RequestStatus::Failed);
    assert!(
        !seen.contains(&RequestStatus::Receiving),
        "a refused advertisement is not received: {seen:?}"
    );
    assert!(receipt.concluded_at().is_some());

    let after = request(&pair, "/echo", rmpv::Value::from("still here")).await;
    let answer = await_receipt(&after).await.expect("the link still answers after a refusal");
    assert_eq!(answer.value(), Some(&rmpv::Value::from("still here")));
}

/// A response Resource that fits `max_response_size` is admitted: the gate is an upper bound,
/// not a switch that refuses every Resource response.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_receipt_resource_response_within_limit_is_admitted() {
    let pair = loopback_request_pair("receipt-probe-within-limit").await;
    let body = over_mdu_body(&pair).await;
    let expected = body.clone();
    register_handler(&pair, "/big", RequestAllow::All, move |_: InboundRequest| {
        std::future::ready(Some(RequestResponse::Bytes(body.clone())))
    })
    .await;

    let receipt = request_with(
        &pair,
        "/big",
        rmpv::Value::Nil,
        RequestOptions::default().with_max_response_size(expected.len() * 4),
    )
    .await;
    let response = await_ready_on_two_clones(&receipt).await;
    assert_eq!(response.into_value(), Some(rmpv::Value::Binary(expected)));
}

/// The id on the receipt is the id the responder derives from what arrived: the packet hash
/// prefix for a request packet, the Resource hash for a request Resource (`RNS/Link.py:496-510`).
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_receipt_id_matches_the_responder_in_both_forms() {
    let pair = loopback_request_pair("receipt-probe-id-parity").await;
    let mut seen = register_recording_echo(&pair, "/echo").await;

    let packet_form = request(&pair, "/echo", rmpv::Value::from("short")).await;
    await_ready_on_two_clones(&packet_form).await;
    let responder_saw = timeout(LOOPBACK_TIMEOUT, seen.recv())
        .await
        .expect("the handler ran")
        .expect("the handler channel is alive");
    assert_eq!(packet_form.request_id(), responder_saw, "packet-form id parity");

    let body = over_mdu_body(&pair).await;
    let resource_form = request(&pair, "/echo", rmpv::Value::Binary(body)).await;
    await_ready_on_two_clones(&resource_form).await;
    let responder_saw = timeout(LOOPBACK_TIMEOUT, seen.recv())
        .await
        .expect("the handler ran")
        .expect("the handler channel is alive");
    assert_eq!(resource_form.request_id(), responder_saw, "resource-form id parity");
    assert_ne!(packet_form.request_id(), resource_form.request_id());
}

/// Three requests in flight on one link, in both forms, each resolve to their own answer in
/// whatever order the consumer awaits them, and `response()` re-read on a concluded receipt
/// returns the same body.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_receipt_in_flight_requests_resolve_independently() {
    let pair = loopback_request_pair("receipt-probe-in-flight").await;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;
    let body = over_mdu_body(&pair).await;

    let alpha = request(&pair, "/echo", rmpv::Value::from("alpha")).await;
    let big = request(&pair, "/echo", rmpv::Value::Binary(body.clone())).await;
    let gamma = request(&pair, "/echo", rmpv::Value::from("gamma")).await;
    assert_ne!(alpha.request_id(), gamma.request_id(), "distinct requests carry distinct ids");

    let gamma_body = await_receipt(&gamma).await.expect("gamma is answered");
    assert_eq!(gamma_body.value(), Some(&rmpv::Value::from("gamma")));
    let big_body = await_receipt(&big).await.expect("the resource-form request is answered");
    assert_eq!(big_body.into_value(), Some(rmpv::Value::Binary(body)));
    let alpha_body = await_receipt(&alpha).await.expect("alpha is answered");
    assert_eq!(alpha_body.value(), Some(&rmpv::Value::from("alpha")));

    assert_eq!(
        await_receipt(&alpha).await,
        Ok(alpha_body),
        "a concluded receipt re-reads the same body"
    );
    assert_eq!(alpha.status(), RequestStatus::Ready);
}

/// A pending receipt has no conclusion time and no progress; once answered it concludes no
/// earlier than it started.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_receipt_concludes_after_it_started() {
    let pair = loopback_request_pair("receipt-probe-timestamps").await;
    register_handler(&pair, "/quiet", RequestAllow::All, silent_handler()).await;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;

    let before = std::time::Instant::now();
    let pending = request(&pair, "/quiet", rmpv::Value::Nil).await;
    let started = pending.started_at().expect("a packet-form request starts its clock when sent");
    assert!(started >= before, "started_at is taken when the request is issued");
    assert_eq!(pending.concluded_at(), None, "a pending receipt has not concluded");
    assert_eq!(pending.progress(), 0.0, "nothing has arrived yet");
    assert_eq!(pending.response_size(), None);

    let answered = request(&pair, "/echo", rmpv::Value::from(7)).await;
    await_ready_on_two_clones(&answered).await;
    let concluded = answered.concluded_at().expect("a ready receipt concluded");
    let started = answered.started_at().expect("a packet-form request starts its clock when sent");
    assert!(concluded >= started, "conclusion follows the start");
    assert!(concluded <= std::time::Instant::now());

    assert_eq!(pending.concluded_at(), None, "another request's answer does not conclude this one");
    assert_eq!(pending.status(), RequestStatus::Sent);
}

/// A zero timeout is already expired; the link watchdog fails it at its next check.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_receipt_zero_timeout_fails_on_the_next_link_check() {
    let pair = loopback_request_pair("receipt-probe-zero-timeout").await;
    register_handler(&pair, "/quiet", RequestAllow::All, silent_handler()).await;

    let started = Instant::now();
    let receipt = request_with(
        &pair,
        "/quiet",
        rmpv::Value::Nil,
        RequestOptions::default().with_timeout(Duration::ZERO),
    )
    .await;
    let outcome =
        timeout(one_tick_bound(), receipt.response()).await.expect("expired on the next tick");
    assert_eq!(outcome, Err(RequestError::Timeout));
    assert!(started.elapsed() <= one_tick_bound());
    assert_eq!(receipt.status(), RequestStatus::Failed);
}

/// `RNS/Link.py:491`: with no override the receipt fails at `rtt * 6 + 11.25 s`, observed
/// from the outside through `response()` alone. The watchdog may land one link check late.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_receipt_default_timeout_fires_from_the_link_rtt() {
    let pair = loopback_request_pair("receipt-probe-default-timeout-fires").await;
    register_handler(&pair, "/quiet", RequestAllow::All, silent_handler()).await;
    let rtt = pair.handles.link.lock().await.rtt();
    let expected = rtt * 6 + RESPONSE_GRACE;

    let started = Instant::now();
    let receipt = request(&pair, "/quiet", rmpv::Value::Nil).await;
    let outcome = timeout(expected + one_tick_bound(), receipt.response())
        .await
        .expect("the default timeout fires");
    let elapsed = started.elapsed();
    assert_eq!(outcome, Err(RequestError::Timeout));
    assert!(elapsed >= RESPONSE_GRACE, "fired before the constant term: {elapsed:?}");
    assert!(elapsed >= expected, "fired before rtt * 6 + 11.25 s ({expected:?}): {elapsed:?}");
    assert_eq!(receipt.status(), RequestStatus::Failed);
}

/// A receipt that outlives every transport handle still answers `response()`: the stopped
/// transport failed it `LinkClosed` on the way out, so the call neither hangs nor panics, and
/// `status()` and a subscription taken before the stop agree with it.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_receipt_outlives_a_dropped_transport_without_hanging() {
    let pair = loopback_request_pair("receipt-probe-dropped-transport").await;
    register_handler(&pair, "/quiet", RequestAllow::All, silent_handler()).await;

    let receipt = request(&pair, "/quiet", rmpv::Value::Nil).await;
    let mut status_rx = receipt.subscribe();
    timeout(LOOPBACK_TIMEOUT, pair.handles.connector.transport.stop())
        .await
        .expect("connector stops");
    timeout(LOOPBACK_TIMEOUT, pair.handles.listener.transport.stop())
        .await
        .expect("listener stops");
    drop(pair);

    let outcome = timeout(LOOPBACK_TIMEOUT, receipt.response())
        .await
        .expect("response() hangs after the transport is dropped");
    assert_eq!(outcome, Err(RequestError::LinkClosed));
    assert_eq!(receipt.status(), RequestStatus::Failed);
    assert_eq!(*status_rx.borrow_and_update(), RequestStatus::Failed);
}
