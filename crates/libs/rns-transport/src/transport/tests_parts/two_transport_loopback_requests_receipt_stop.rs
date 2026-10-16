// Loopback coverage for `Transport::stop` against a link whose lock is held by someone else
// for the whole shutdown: the receipt sweep shares the stop budget, so `stop` still returns
// within it and leaves the held link's receipts as they were. Once the lock is released and
// the transport is dropped, the link goes with it and the receipt concludes `LinkClosed` on
// its own. The sweep skips only the held link: a second link the same connector holds is
// still swept, and its receipt concludes `LinkClosed` inside `stop`. Helpers come from the
// sibling `tests_parts` files.

use super::lifecycle::STOP_BUDGET;

/// Scheduling slack on top of the stop budget `stop` may return late by.
const STOP_SLACK: Duration = Duration::from_secs(3);

/// Bounds the wait for a receipt that concludes on the link drop.
const LINK_DROP_TIMEOUT: Duration = LOOPBACK_TIMEOUT;

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
        // Err means the test dropped its half; either way the guard goes below.
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
    assert!(
        elapsed >= STOP_BUDGET,
        "stop returned before the held link's wait ran out: {elapsed:?}"
    );
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

/// A second link from the connector to a sibling destination on the listener, with the
/// silent handler registered there, so a request on it stays `Sent` like one on the pair's
/// link. `Transport::link` reuses the out-link per destination, hence the sibling.
#[deny(clippy::await_holding_invalid_type)]
async fn stop_open_sibling_link(pair: &mut LoopbackRequestPair) -> (Arc<Mutex<Link>>, AddressHash) {
    // See `two_transport_loopback_request_handler_is_scoped_to_its_destination` for why the
    // sibling's announce waits.
    tokio::time::sleep(SIBLING_ANNOUNCE_SPACING).await;
    let (sibling, sibling_desc) = register_in_destination(
        &pair.handles.listener.transport,
        "loopback",
        "receipt-stop-sibling",
    )
    .await;
    sibling
        .lock()
        .await
        .register_request_handler("/quiet", Arc::new(silent_handler()), RequestAllow::All, true)
        .expect("handler registers");
    let learned =
        learn_announce(&mut pair.handles.connector.announces, &sibling_desc.address_hash).await;
    let (link, link_id) = open_link(&pair.handles.connector.transport, learned).await;
    assert_ne!(link_id, pair.handles.link_id, "two distinct links");
    (link, link_id)
}

/// The sweep waits out the budget on the held link and still takes the other link's lock
/// afterwards: its receipt concludes `LinkClosed` inside `stop`, the held link's stays as it
/// was until the link is dropped.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_stop_sweeps_the_links_it_can_lock_beside_a_held_one() {
    let mut pair = loopback_request_pair("receipt-stop-two-links").await;
    register_handler(&pair, "/quiet", RequestAllow::All, silent_handler()).await;
    let (_sibling_link, sibling_link_id) = stop_open_sibling_link(&mut pair).await;
    let held_receipt = request(&pair, "/quiet", rmpv::Value::Nil).await;
    let swept_receipt = pair
        .handles
        .connector
        .transport
        .request(&sibling_link_id, "/quiet", rmpv::Value::Nil, RequestOptions::default())
        .await
        .expect("request is sent");
    assert_eq!(held_receipt.status(), RequestStatus::Sent);
    assert_eq!(swept_receipt.status(), RequestStatus::Sent);

    let held = hold_link(pair.handles.link.clone());
    held.locked.await.expect("the holder takes the lock");

    let started = std::time::Instant::now();
    pair.handles.connector.transport.stop().await;
    let elapsed = started.elapsed();
    assert!(elapsed < STOP_BUDGET + STOP_SLACK, "stop overran its budget: {elapsed:?}");
    assert_eq!(held_receipt.status(), RequestStatus::Sent, "the sweep skipped the held link");
    assert_eq!(swept_receipt.status(), RequestStatus::Failed, "the other link was swept");
    assert_eq!(swept_receipt.response().await, Err(RequestError::LinkClosed));
    let concluded = swept_receipt.concluded_at().expect("the sweep concluded it");
    assert!(concluded <= started + STOP_BUDGET + STOP_SLACK, "concluded inside stop");

    held.release.send(()).expect("the holder waits for the release");
    held.thread.join().expect("the holder releases the lock");
    drop(pair);

    let outcome = timeout(LINK_DROP_TIMEOUT, held_receipt.response())
        .await
        .expect("the receipt concludes once the link is dropped");
    assert_eq!(outcome, Err(RequestError::LinkClosed));
}
