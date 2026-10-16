// Loopback coverage for `Transport::stop` against a link whose lock is held by someone else
// for the whole shutdown: the receipt sweep shares the stop budget, so `stop` still returns
// within it and leaves the held link's receipts as they were. Once the lock is released and
// the transport is dropped, the link goes with it and the receipt concludes `LinkClosed` on
// its own. Helpers come from the sibling `tests_parts` files.

use super::lifecycle::STOP_BUDGET;

/// Scheduling slack on top of the stop budget `stop` may return late by.
const STOP_SLACK: Duration = Duration::from_secs(3);

/// Bounds the wait for a receipt that concludes on the link drop.
const LINK_DROP_TIMEOUT: Duration = Duration::from_secs(15);

/// A thread that holds `link`'s lock until `release` fires. The guard lives on the thread, not
/// across an await in the test, and `locked` tells the test when the lock is taken.
struct HeldLink {
    locked: tokio::sync::oneshot::Receiver<()>,
    release: tokio::sync::oneshot::Sender<()>,
    thread: std::thread::JoinHandle<()>,
}

fn hold_link(link: Arc<Mutex<Link>>) -> HeldLink {
    let (locked_tx, locked) = tokio::sync::oneshot::channel();
    let (release, release_rx) = tokio::sync::oneshot::channel::<()>();
    let thread = std::thread::spawn(move || {
        let guard = link.blocking_lock();
        locked_tx.send(()).expect("the test waits for the lock");
        let _ = release_rx.blocking_recv();
        drop(guard);
    });
    HeldLink { locked, release, thread }
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_stop_skips_a_held_link_and_its_receipt_concludes_on_the_drop() {
    let pair = loopback_request_pair("receipt-stop-held-link").await;
    register_handler(&pair, "/quiet", RequestAllow::All, silent_handler()).await;
    let receipt = request(&pair, "/quiet", rmpv::Value::Nil).await;
    assert_eq!(receipt.status(), RequestStatus::Sent);

    let held = hold_link(pair.handles.link.clone());
    held.locked.await.expect("the holder takes the lock");

    let started = std::time::Instant::now();
    pair.handles.connector.transport.stop().await;
    let elapsed = started.elapsed();
    assert!(elapsed < STOP_BUDGET + STOP_SLACK, "stop overran its budget: {elapsed:?}");
    assert_eq!(receipt.status(), RequestStatus::Sent, "the sweep skipped the held link");

    held.release.send(()).expect("the holder waits for the release");
    held.thread.join().expect("the holder releases the lock");
    drop(pair);

    let outcome = timeout(LINK_DROP_TIMEOUT, receipt.response())
        .await
        .expect("the receipt concludes once the link is dropped");
    assert_eq!(outcome, Err(RequestError::LinkClosed));
    assert_eq!(receipt.status(), RequestStatus::Failed);
}
