// Usage-pattern coverage for what a `subscribe()` receiver taken BEFORE the request
// concludes sees once it does, as `RequestReceipt::subscribe` documents it for a normal
// conclusion: the terminal status is reported as the last change, then the sender is
// dropped with the pending entry, so the next `changed()` returns an error rather than a
// further change, `borrow()` keeps the terminal status, and reading the receipt afterwards
// (`status()`, `response()`, `concluded_at()`) moves nothing the receiver sees. Covered for a
// packet-form `Ready`, a resource-form `Ready` that passed through `Delivered`, and a
// `Timeout` failure. Helpers come from the sibling `tests_parts` files.

/// A receiver that already read `terminal` as a change finds the sender gone: `changed()`
/// errors out, `borrow()` keeps `terminal`, and `has_changed()` keeps erroring.
#[deny(clippy::await_holding_invalid_type)]
async fn assert_early_subscriber_outlives_the_sender(
    status_rx: &mut tokio::sync::watch::Receiver<RequestStatus>,
    terminal: RequestStatus,
) {
    assert_eq!(*status_rx.borrow(), terminal, "the terminal status was the last change");
    let change = timeout(RECEIPT_TIMEOUT, status_rx.changed())
        .await
        .expect("the sender goes with the concluded entry");
    assert!(change.is_err(), "no change follows the conclusion: {change:?}");
    assert_eq!(*status_rx.borrow(), terminal, "the receiver keeps the terminal status");
    assert!(status_rx.has_changed().is_err(), "the sender stays gone");
}

/// Packet form: a subscriber taken at `Sent` is reported `Ready` as a change, then finds the
/// sender gone; reading the receipt afterwards on two clones changes nothing it sees.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_receipt_early_subscriber_outlives_a_ready_conclusion() {
    let pair = loopback_request_pair("receipt-probe7-early-ready").await;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;

    let receipt = request(&pair, "/echo", rmpv::Value::from("early")).await;
    let clone = receipt.clone();
    let mut early = receipt.subscribe();
    assert_eq!(*early.borrow_and_update(), RequestStatus::Sent);

    await_status(&mut early, RequestStatus::Ready, RECEIPT_TIMEOUT).await;
    assert_early_subscriber_outlives_the_sender(&mut early, RequestStatus::Ready).await;

    assert_eq!(receipt.status(), RequestStatus::Ready);
    let concluded = receipt.concluded_at().expect("a ready receipt has concluded");
    let body = await_ready_on_two_clones(&clone).await;
    assert_eq!(body.value(), Some(&rmpv::Value::from("early")));
    assert_eq!(receipt.concluded_at(), Some(concluded), "reading does not move the conclusion");
    assert_eq!(*early.borrow(), RequestStatus::Ready, "reading moves no subscriber");
    assert!(early.has_changed().is_err(), "the sender stays gone after the reads");
}

/// Resource form: a subscriber taken at `Sent` is reported `Ready` as the last change (the
/// `Delivered` and `Receiving` steps on the way may coalesce into it over a fast loopback,
/// as `subscribe()` documents), then finds the sender gone with `Ready` kept.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_receipt_early_subscriber_outlives_a_resource_form_ready() {
    let pair = loopback_request_pair("receipt-probe7-early-resource-ready").await;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;
    let body = over_mdu_body(&pair).await;

    let receipt = request(&pair, "/echo", rmpv::Value::Binary(body.clone())).await;
    let mut early = receipt.subscribe();
    assert_eq!(*early.borrow_and_update(), RequestStatus::Sent);

    await_status(&mut early, RequestStatus::Ready, RECEIPT_TIMEOUT).await;
    assert_early_subscriber_outlives_the_sender(&mut early, RequestStatus::Ready).await;

    let response = await_ready_on_two_clones(&receipt).await;
    assert_eq!(response.into_value(), Some(rmpv::Value::Binary(body)));
    assert!(receipt.response_size().is_some(), "the echo came back as a Resource");
    assert_eq!(*early.borrow(), RequestStatus::Ready, "reading moves no subscriber");
}

/// A `Timeout` failure: the subscriber is reported `Failed` as the last change, then finds
/// the sender gone with `Failed` kept; `response()` reads `Timeout` on two clones without
/// moving it, and a subscriber taken after the failure reads `Failed` and no change.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_receipt_early_subscriber_outlives_a_timeout() {
    let pair = loopback_request_pair("receipt-probe7-early-timeout").await;
    register_handler(&pair, "/quiet", RequestAllow::All, silent_handler()).await;

    let receipt = request_with(
        &pair,
        "/quiet",
        rmpv::Value::Nil,
        RequestOptions::default().with_timeout(Duration::ZERO),
    )
    .await;
    let clone = receipt.clone();
    let mut early = receipt.subscribe();
    assert_eq!(*early.borrow_and_update(), RequestStatus::Sent);

    await_status(&mut early, RequestStatus::Failed, one_tick_bound()).await;
    assert_early_subscriber_outlives_the_sender(&mut early, RequestStatus::Failed).await;

    let concluded = receipt.concluded_at().expect("a failed receipt has concluded");
    assert_eq!(await_receipt(&receipt).await, Err(RequestError::Timeout));
    assert_eq!(await_receipt(&clone).await, Err(RequestError::Timeout));
    assert_eq!(receipt.concluded_at(), Some(concluded), "reading does not move the conclusion");
    assert_eq!(*early.borrow(), RequestStatus::Failed, "reading moves no subscriber");

    let mut late = receipt.subscribe();
    assert!(late.has_changed().is_err(), "the sender is already gone");
    assert_eq!(*late.borrow_and_update(), RequestStatus::Failed);
}
