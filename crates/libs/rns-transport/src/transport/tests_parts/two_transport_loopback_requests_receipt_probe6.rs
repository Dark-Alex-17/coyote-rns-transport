// Usage-pattern coverage for what a `subscribe()` receiver sees when the link goes away
// without concluding the request, as `RequestReceipt::subscribe` documents it: the sender
// is dropped with the link, so `changed()` returns an error rather than a `Failed` change,
// `borrow()` keeps the last status the link published, and `status()` / `response()`
// conclude the receipt `LinkClosed` on their own without moving what the receiver reads.
// The link is made to vanish unconcluded the way the stop fixture does: its lock is held
// through `Transport::stop`, so the receipt sweep skips it, and the transport is dropped
// afterwards. Helpers come from the sibling `tests_parts` files.

/// A receiver taken before the link vanished reads an error from `changed()`, not a
/// change, and still borrows the status the link last published.
#[deny(clippy::await_holding_invalid_type)]
async fn assert_early_subscriber_sees_the_sender_go(
    status_rx: &mut tokio::sync::watch::Receiver<RequestStatus>,
    last: RequestStatus,
) {
    let change = timeout(LINK_DROP_TIMEOUT, status_rx.changed())
        .await
        .expect("the sender goes with the link");
    assert!(change.is_err(), "a vanished link is not reported as a `Failed` change: {change:?}");
    assert_eq!(*status_rx.borrow(), last, "the receiver keeps the last published status");
    assert!(status_rx.has_changed().is_err(), "the sender stays gone");
}

/// A receiver taken after the link vanished reads the last published status and no change.
fn assert_late_subscriber_reads_the_last_status(receipt: &RequestReceipt, last: RequestStatus) {
    let mut status_rx = receipt.subscribe();
    assert!(status_rx.has_changed().is_err(), "the sender is already gone");
    assert_eq!(*status_rx.borrow_and_update(), last, "the late subscriber reads the last status");
}

/// A packet-form receipt in `Sent` whose link vanishes unconcluded: the early subscriber
/// sees the sender go rather than a `Failed` change, the receipt concludes `LinkClosed` on
/// `status()` and `response()` (both clones agree), and neither conclusion changes what any
/// subscriber borrows.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_receipt_subscriber_sees_the_sender_go_with_the_link() {
    let pair = loopback_request_pair("receipt-probe6-vanished-link").await;
    register_handler(&pair, "/quiet", RequestAllow::All, silent_handler()).await;
    let receipt = request(&pair, "/quiet", rmpv::Value::Nil).await;
    let clone = receipt.clone();
    assert_eq!(receipt.status(), RequestStatus::Sent);

    let mut early = receipt.subscribe();
    assert_eq!(*early.borrow_and_update(), RequestStatus::Sent);
    assert!(!early.has_changed().expect("the link is alive"), "`Sent` predates the subscriber");

    let held = hold_link(pair.handles.link.clone());
    held.locked.await.expect("the holder takes the lock");
    pair.handles.connector.transport.stop().await;
    assert_eq!(receipt.status(), RequestStatus::Sent, "the sweep skipped the held link");
    assert!(!early.has_changed().expect("the link is still there"), "stop left it `Sent`");
    held.release.send(()).expect("the holder waits for the release");
    held.thread.join().expect("the holder releases the lock");
    drop(pair);

    assert_early_subscriber_sees_the_sender_go(&mut early, RequestStatus::Sent).await;

    assert_eq!(receipt.status(), RequestStatus::Failed, "`status()` concludes on its own");
    let concluded = receipt.concluded_at().expect("a vanished link concludes the receipt");
    assert_eq!(clone.status(), RequestStatus::Failed, "the clone agrees");
    assert_eq!(clone.concluded_at(), Some(concluded), "the clone shares the conclusion");
    assert_eq!(*early.borrow(), RequestStatus::Sent, "`status()` moves no subscriber");
    assert_late_subscriber_reads_the_last_status(&receipt, RequestStatus::Sent);

    assert_eq!(receipt.response().await, Err(RequestError::LinkClosed));
    assert_eq!(clone.response().await, Err(RequestError::LinkClosed));
    assert_eq!(receipt.concluded_at(), Some(concluded), "concluded once");
    assert_eq!(*early.borrow(), RequestStatus::Sent, "`response()` moves no subscriber");
    assert_late_subscriber_reads_the_last_status(&clone, RequestStatus::Sent);
}

/// A resource-form receipt parked in `Delivered` whose link vanishes unconcluded: the
/// subscriber keeps `Delivered`, a status the link published after the subscription, so
/// "keeps the last status" is told apart from a reset to the initial `Sent`. The receipt's
/// clock is unbounded so the sweep cannot race it; `concluded_at()` is read before `status()`
/// because its doc says a vanished link concludes the receipt there too.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_receipt_subscriber_keeps_delivered_when_the_link_goes() {
    let pair = loopback_request_pair("receipt-probe6-vanished-link-delivered").await;
    register_handler(&pair, "/quiet", RequestAllow::All, silent_handler()).await;
    let body = over_mdu_body(&pair).await;
    let receipt = request_with(
        &pair,
        "/quiet",
        rmpv::Value::Binary(body),
        RequestOptions::default().with_timeout(Duration::MAX),
    )
    .await;
    let clone = receipt.clone();
    let mut early = receipt.subscribe();
    assert_eq!(*early.borrow_and_update(), RequestStatus::Sent);

    await_status(&mut early, RequestStatus::Delivered, LOOPBACK_TIMEOUT).await;
    assert_eq!(receipt.status(), RequestStatus::Delivered);
    assert!(receipt.started_at().is_some(), "delivery starts the clock");
    assert_eq!(receipt.concluded_at(), None, "nothing has concluded the receipt yet");

    let held = hold_link(pair.handles.link.clone());
    held.locked.await.expect("the holder takes the lock");
    pair.handles.connector.transport.stop().await;
    assert_eq!(receipt.status(), RequestStatus::Delivered, "the sweep skipped the held link");
    held.release.send(()).expect("the holder waits for the release");
    held.thread.join().expect("the holder releases the lock");
    drop(pair);

    assert_early_subscriber_sees_the_sender_go(&mut early, RequestStatus::Delivered).await;

    let concluded = clone.concluded_at().expect("`concluded_at()` concludes a vanished link");
    assert_eq!(receipt.status(), RequestStatus::Failed, "the other clone reads the conclusion");
    assert_eq!(receipt.concluded_at(), Some(concluded), "concluded once");
    assert_eq!(*early.borrow(), RequestStatus::Delivered, "concluding moves no subscriber");
    assert_late_subscriber_reads_the_last_status(&receipt, RequestStatus::Delivered);

    assert_eq!(receipt.response().await, Err(RequestError::LinkClosed));
    assert_eq!(clone.response().await, Err(RequestError::LinkClosed));
    assert_eq!(receipt.concluded_at(), Some(concluded), "`response()` does not move it");
    assert_eq!(*early.borrow(), RequestStatus::Delivered, "`response()` moves no subscriber");
}
