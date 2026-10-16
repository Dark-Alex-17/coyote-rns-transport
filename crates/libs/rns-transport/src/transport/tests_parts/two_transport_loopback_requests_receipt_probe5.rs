// Usage-pattern coverage for how a consumer holds a `RequestReceipt` after the request is on
// its way: `response()` is a read a consumer may repeat on one clone after the receipt
// concluded, `Ok` or `Err`, and reads the same outcome every time; a subscriber taken after
// the conclusion reads the final status with `borrow_and_update` and is reported no change;
// a receipt every clone of which is dropped before the response arrives, in either form,
// still has its response reach the link (`LinkEvent::Data(Response)` keeps broadcasting)
// and leaves the link answering the next request. Helpers come from the sibling
// `tests_parts` files.

/// Long enough for a status change that is coming to be seen, short enough that waiting on
/// one that never comes does not slow the test down.
const NO_CHANGE_WINDOW: Duration = Duration::from_millis(300);

/// A late subscriber reads `wanted` as the current status, is reported no change for it, and
/// sees no change arrive afterwards.
#[deny(clippy::await_holding_invalid_type)]
async fn assert_late_subscriber_reads(receipt: &RequestReceipt, wanted: RequestStatus) {
    let mut status_rx = receipt.subscribe();
    assert!(
        !matches!(status_rx.has_changed(), Ok(true)),
        "the status current at `subscribe()` is not reported as a change"
    );
    assert_eq!(
        *status_rx.borrow_and_update(),
        wanted,
        "the late subscriber reads the final status"
    );
    let change = timeout(NO_CHANGE_WINDOW, status_rx.changed()).await;
    assert!(
        !matches!(change, Ok(Ok(()))),
        "a concluded receipt reports no further change: {change:?}"
    );
    assert_eq!(*status_rx.borrow(), wanted, "the final status stays readable");
}

/// `response()` is a read: repeated on the same clone after the receipt concluded `Ready`
/// it returns the same body every time, and a late subscriber reads `Ready`.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_receipt_response_reads_again_on_the_same_clone() {
    let pair = loopback_request_pair("receipt-probe5-reread-ready").await;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;

    let receipt = request(&pair, "/echo", rmpv::Value::from("again")).await;
    let first = await_receipt(&receipt).await.expect("the request is answered");
    assert_eq!(first.value(), Some(&rmpv::Value::from("again")));
    for _ in 0..3 {
        assert_eq!(await_receipt(&receipt).await, Ok(first.clone()), "the same clone rereads");
    }
    let clone = receipt.clone();
    assert_eq!(await_receipt(&clone).await, Ok(first.clone()), "a clone taken late rereads");
    assert_eq!(receipt.status(), RequestStatus::Ready);
    let concluded = receipt.concluded_at().expect("a ready receipt has concluded");
    assert_eq!(receipt.concluded_at(), Some(concluded), "rereading does not move the conclusion");
    assert_late_subscriber_reads(&receipt, RequestStatus::Ready).await;
}

/// `response()` repeated on a receipt that failed returns the same failure every time, and a
/// late subscriber reads `Failed`.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_receipt_response_reads_the_same_failure_again() {
    let pair = loopback_request_pair("receipt-probe5-reread-failed").await;
    register_handler(&pair, "/quiet", RequestAllow::All, silent_handler()).await;

    let receipt = request_with(
        &pair,
        "/quiet",
        rmpv::Value::Nil,
        RequestOptions::default().with_timeout(Duration::ZERO),
    )
    .await;
    let bound = INTERVAL_LINKS_CHECK + TIMEOUT_SLACK;
    let first = timeout(bound, receipt.response()).await.expect("expired on the next tick");
    assert_eq!(first, Err(RequestError::Timeout));
    for _ in 0..3 {
        assert_eq!(await_receipt(&receipt).await, Err(RequestError::Timeout));
    }
    assert_eq!(await_receipt(&receipt.clone()).await, Err(RequestError::Timeout));
    assert_eq!(receipt.status(), RequestStatus::Failed);
    assert_late_subscriber_reads(&receipt, RequestStatus::Failed).await;
}

/// A packet-form receipt dropped, every clone of it, before the response arrives: the
/// response still reaches the link's broadcast under the request's id, and the link answers
/// the next request on a receipt that resolves normally.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_receipt_dropped_packet_form_leaves_the_link_answering() {
    let mut pair = loopback_request_pair("receipt-probe5-dropped-packet").await;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;

    let dropped_id = {
        let receipt = request(&pair, "/echo", rmpv::Value::from("nobody waits")).await;
        let clone = receipt.clone();
        let id = receipt.request_id();
        drop(receipt);
        drop(clone);
        id
    };
    let body = await_response(&mut pair, dropped_id).await;
    assert_eq!(body, rmpv::Value::from("nobody waits"), "the response still reaches the link");

    let next = request(&pair, "/echo", rmpv::Value::from("somebody waits")).await;
    assert_ne!(next.request_id(), dropped_id, "a fresh request has its own id");
    let response = await_ready_on_two_clones(&next).await;
    assert_eq!(response.value(), Some(&rmpv::Value::from("somebody waits")));
}

/// A resource-form receipt dropped before its request Resource even completes: the responder
/// still serves the request (its handler runs with the id the receipt reported), the echoed
/// Resource comes back to a link with nobody waiting, and the link answers the next request
/// in both forms afterwards.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_receipt_dropped_resource_form_leaves_the_link_answering() {
    let pair = loopback_request_pair("receipt-probe5-dropped-resource").await;
    let mut seen = register_recording_echo(&pair, "/echo").await;
    let body = over_mdu_body(&pair).await;

    let dropped_id = {
        let receipt = request(&pair, "/echo", rmpv::Value::Binary(body.clone())).await;
        assert_eq!(receipt.status(), RequestStatus::Sent, "dropped while the Resource is out");
        receipt.request_id()
    };
    let served = timeout(LOOPBACK_TIMEOUT, seen.recv())
        .await
        .expect("the responder serves the request")
        .expect("the handler channel is alive");
    assert_eq!(served, dropped_id, "the responder derives the id the dropped receipt reported");

    let small = request(&pair, "/echo", rmpv::Value::from("packet after")).await;
    let response = await_ready_on_two_clones(&small).await;
    assert_eq!(response.value(), Some(&rmpv::Value::from("packet after")));
    assert_eq!(
        timeout(LOOPBACK_TIMEOUT, seen.recv()).await.expect("served").expect("alive"),
        small.request_id()
    );

    let large = request(&pair, "/echo", rmpv::Value::Binary(body.clone())).await;
    let response = await_ready_on_two_clones(&large).await;
    assert_eq!(response.into_value(), Some(rmpv::Value::Binary(body)));
    assert_eq!(
        timeout(LOOPBACK_TIMEOUT, seen.recv()).await.expect("served").expect("alive"),
        large.request_id()
    );
    assert!(large.response_size().is_some(), "the second echo came back as a Resource too");
}
