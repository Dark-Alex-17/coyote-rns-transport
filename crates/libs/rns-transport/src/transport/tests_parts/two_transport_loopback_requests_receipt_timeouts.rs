// Loopback coverage for the request timeout. The default is `rtt * 6 + 11.25 s`
// (`RNS/Link.py:491`), `RequestOptions::with_timeout` overrides it, and the link watchdog
// fails an unanswered receipt with `RequestError::Timeout` once the deadline passes, in every
// state: Python (`RNS/Link.py:1398`, `:1408`) fails only a receipt that reached `DELIVERED`.
// Helpers come from the sibling `tests_parts` files.

/// Short enough to wait for, long enough that the response path cannot race it.
const OVERRIDE_TIMEOUT: Duration = Duration::from_millis(1500);

/// Scheduling slack on top of the link check tick a timeout may land late by.
const TIMEOUT_SLACK: Duration = Duration::from_millis(500);

/// `response()` must read `Timeout`, no earlier than the deadline and no later than one link
/// check interval plus slack after it.
#[deny(clippy::await_holding_invalid_type)]
async fn assert_times_out(receipt: &RequestReceipt, started: Instant) {
    let outcome = timeout(OVERRIDE_TIMEOUT + INTERVAL_LINKS_CHECK + TIMEOUT_SLACK, receipt.response())
        .await
        .expect("the receipt times out");
    let elapsed = started.elapsed();
    assert_eq!(outcome, Err(RequestError::Timeout));
    assert_eq!(receipt.status(), RequestStatus::Failed);
    assert!(elapsed >= OVERRIDE_TIMEOUT, "the receipt failed before its deadline: {elapsed:?}");
    assert!(receipt.concluded_at().is_some(), "a failed receipt has concluded");
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_receipt_default_timeout_follows_the_link_rtt() {
    let pair = loopback_request_pair("receipt-default-timeout").await;
    register_handler(&pair, "/quiet", RequestAllow::All, silent_handler()).await;

    let rtt = pair.handles.link.lock().await.rtt();
    assert!(rtt > Duration::ZERO, "the initiator measured its rtt on activation");
    let receipt = request(&pair, "/quiet", rmpv::Value::Nil).await;

    assert_eq!(receipt.timeout(), rtt * 6 + Duration::from_millis(11_250));
    assert_eq!(receipt.status(), RequestStatus::Sent, "the default timeout is far off");
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_receipt_packet_form_times_out_without_a_response() {
    let pair = loopback_request_pair("receipt-timeout-packet").await;
    register_handler(&pair, "/quiet", RequestAllow::All, silent_handler()).await;

    let started = Instant::now();
    let receipt = request_with(
        &pair,
        "/quiet",
        rmpv::Value::Nil,
        RequestOptions::default().with_timeout(OVERRIDE_TIMEOUT),
    )
    .await;
    assert_eq!(receipt.timeout(), OVERRIDE_TIMEOUT);

    assert_times_out(&receipt, started).await;
}

/// An unregistered path answers nothing either; the receipt still times out.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_receipt_unhandled_path_times_out() {
    let pair = loopback_request_pair("receipt-timeout-no-handler").await;

    let started = Instant::now();
    let receipt = request_with(
        &pair,
        "/nobody-home",
        rmpv::Value::Nil,
        RequestOptions::default().with_timeout(OVERRIDE_TIMEOUT),
    )
    .await;

    assert_times_out(&receipt, started).await;
}

/// The resource form reaches `Delivered` when its request Resource completes
/// (`RNS/Link.py:1372`) and still times out when nothing answers.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_receipt_resource_form_is_delivered_then_times_out() {
    let pair = loopback_request_pair("receipt-timeout-resource-form").await;
    register_handler(&pair, "/quiet", RequestAllow::All, silent_handler()).await;
    let body = over_mdu_body(&pair).await;

    let started = Instant::now();
    let receipt = request_with(
        &pair,
        "/quiet",
        rmpv::Value::Binary(body),
        RequestOptions::default().with_timeout(OVERRIDE_TIMEOUT),
    )
    .await;

    let mut status_rx = receipt.subscribe();
    let first = *status_rx.borrow_and_update();
    assert!(
        matches!(first, RequestStatus::Sent | RequestStatus::Delivered),
        "the request Resource may already have completed: {first:?}"
    );
    if first == RequestStatus::Sent {
        timeout(OVERRIDE_TIMEOUT, status_rx.changed())
            .await
            .expect("the request Resource completes before the timeout")
            .expect("the pending entry is alive");
        assert_eq!(*status_rx.borrow_and_update(), RequestStatus::Delivered);
    }

    assert_times_out(&receipt, started).await;
}
