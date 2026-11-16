//! Usage-pattern probe for `delivery::await_link_activation` on a link re-pended under the
//! wait and then closed a second time, written from the contract alone: the wait ends with
//! `io::ErrorKind::ConnectionAborted` and `link closed before activation: <reason>` "as soon
//! as the link reads `LinkStatus::Closed`", within the poll slice; the `Closed` event only
//! wakes the reread. `tests/link_activation_closed_wake_probe.rs` pins the sanctioned
//! exception's first half: a close and `Link::restart()` or `Link::request()` in one lock
//! scope leaves the wait running. This file pins the half a consumer meets next: the
//! re-pended link closes again under the SAME wait, by hand or by its own establishment
//! timeout, and the wait still ends at that close within the poll slice instead of running
//! to its deadline. `Link::request()` rotates the link id, so after it the waiter's event
//! filter no longer matches the link; the status read every poll slice is what has to
//! answer. The transport has broadcast on and no interface, so `Transport::link` hands back
//! a pending link that nothing can ever activate.

use std::sync::Arc;
use std::time::{Duration, Instant};

use rns_transport::delivery::await_link_activation;
use rns_transport::destination::link::{LinkCloseReason, LinkStatus};
use tokio::time::timeout;

#[path = "support/assert_closed_before_activation.rs"]
mod assert_closed_before_activation;
#[path = "support/broadcasting.rs"]
mod broadcasting;
#[path = "support/unknown_destination.rs"]
mod unknown_destination;
use assert_closed_before_activation::assert_closed_before_activation;
use broadcasting::broadcasting;
use unknown_destination::unknown_destination;

/// An establishment timeout long enough that it cannot close a link under the test.
const ESTABLISHMENT_FAR: Duration = Duration::from_secs(60);
/// The establishment timeout given to a re-pended link whose own watchdog is to close it.
const ESTABLISHMENT_SHORT: Duration = Duration::from_secs(2);
/// What the caller passes; the wait has to return long before it.
const LONG_WAIT: Duration = Duration::from_secs(60);
/// When the closer task closes and re-pends the link.
const REPEND_AT: Duration = Duration::from_millis(300);
/// How long after the re-pend the closer closes the link a second time.
const SECOND_CLOSE_AFTER: Duration = Duration::from_millis(600);
/// A latency no wait may exceed after a close and still count as ending with it: the
/// contract's poll slice (250 ms) with room for a loaded host.
const CLOSE_BOUND: Duration = Duration::from_millis(1500);
/// The maintenance job that closes a timed-out link runs once per second; a close by it
/// lands within this of the establishment timeout, plus the poll slice.
const TIMEOUT_CLOSE_BOUND: Duration = Duration::from_millis(2500);
/// How long the transport is given to stop.
const WAIT: Duration = Duration::from_secs(15);

/// A link closed and re-pended by `Link::request()` under the wait, then closed again by
/// hand: the second close ends the wait with `ConnectionAborted` naming `InitiatorClosed`
/// within the poll slice of that close, even though the link's id changed under the waiter.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn a_second_hand_close_after_a_request_repend_ends_the_wait_within_the_poll_slice() {
    let transport = broadcasting("link-activation-repend-request-then-close");
    let link = transport.link(unknown_destination()).await.expect("pending link with broadcast on");
    link.lock().await.set_establishment_timeout(ESTABLISHMENT_FAR);
    let id_before = *link.lock().await.id();

    let closer = Arc::clone(&link);
    let closer_task = tokio::spawn(async move {
        tokio::time::sleep(REPEND_AT).await;
        let id_after = {
            let mut guard = closer.lock().await;
            guard.close();
            let _ = guard.request();
            guard.set_establishment_timeout(ESTABLISHMENT_FAR);
            assert_eq!(
                guard.status(),
                LinkStatus::Pending,
                "request puts the link back to pending"
            );
            *guard.id()
        };
        tokio::time::sleep(SECOND_CLOSE_AFTER).await;
        closer.lock().await.close();
        (id_after, Instant::now())
    });

    let error = await_link_activation(&transport, &link, LONG_WAIT)
        .await
        .expect_err("a pending link closed by hand never activates");
    let returned = Instant::now();
    let (id_after, closed_at) = closer_task.await.expect("closer task");
    assert_ne!(id_after, id_before, "a fresh request rotates the link id");
    assert_closed_before_activation(&error, "InitiatorClosed");
    let latency = returned.saturating_duration_since(closed_at);
    assert!(
        latency < CLOSE_BOUND,
        "the wait ends with the second close within the poll slice, not at its deadline: \
         returned {latency:?} after the close"
    );
    assert_eq!(link.lock().await.status(), LinkStatus::Closed);
    assert_eq!(link.lock().await.close_reason(), Some(LinkCloseReason::InitiatorClosed));

    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
}

/// A link closed and re-pended by `Link::restart()` under the wait, then closed again by
/// hand: the id is unchanged, so the second `Closed` event wakes the reread and the wait
/// ends with `ConnectionAborted` naming `InitiatorClosed` within the poll slice.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn a_second_hand_close_after_a_restart_repend_ends_the_wait_within_the_poll_slice() {
    let transport = broadcasting("link-activation-repend-restart-then-close");
    let link = transport.link(unknown_destination()).await.expect("pending link with broadcast on");
    link.lock().await.set_establishment_timeout(ESTABLISHMENT_FAR);
    let id_before = *link.lock().await.id();

    let closer = Arc::clone(&link);
    let closer_task = tokio::spawn(async move {
        tokio::time::sleep(REPEND_AT).await;
        {
            let mut guard = closer.lock().await;
            guard.close();
            guard.restart();
            guard.set_establishment_timeout(ESTABLISHMENT_FAR);
            assert_eq!(
                guard.status(),
                LinkStatus::Pending,
                "restart puts the link back to pending"
            );
        }
        tokio::time::sleep(SECOND_CLOSE_AFTER).await;
        closer.lock().await.close();
        Instant::now()
    });

    let error = await_link_activation(&transport, &link, LONG_WAIT)
        .await
        .expect_err("a pending link closed by hand never activates");
    let returned = Instant::now();
    let closed_at = closer_task.await.expect("closer task");
    assert_eq!(*link.lock().await.id(), id_before, "restart keeps the link id");
    assert_closed_before_activation(&error, "InitiatorClosed");
    let latency = returned.saturating_duration_since(closed_at);
    assert!(
        latency < CLOSE_BOUND,
        "the wait ends with the second close within the poll slice, not at its deadline: \
         returned {latency:?} after the close"
    );
    assert_eq!(link.lock().await.status(), LinkStatus::Closed);

    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
}

/// A link closed and re-pended by `Link::request()` under the wait with a 2 s establishment
/// timeout: the maintenance job's `Timeout` close of the re-pended link ends the wait with
/// `ConnectionAborted` naming `Timeout`, within the maintenance tick plus the poll slice of
/// that timeout, not at the caller's 60 s deadline.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn a_repended_links_establishment_timeout_ends_the_wait_naming_timeout() {
    let transport = broadcasting("link-activation-repend-request-then-timeout");
    let link = transport.link(unknown_destination()).await.expect("pending link with broadcast on");
    link.lock().await.set_establishment_timeout(ESTABLISHMENT_FAR);

    let closer = Arc::clone(&link);
    let closer_task = tokio::spawn(async move {
        tokio::time::sleep(REPEND_AT).await;
        let mut guard = closer.lock().await;
        guard.close();
        let _ = guard.request();
        guard.set_establishment_timeout(ESTABLISHMENT_SHORT);
        assert_eq!(guard.status(), LinkStatus::Pending, "request puts the link back to pending");
        Instant::now()
    });

    let error = await_link_activation(&transport, &link, LONG_WAIT)
        .await
        .expect_err("a pending link with no interface never activates");
    let returned = Instant::now();
    let repended_at = closer_task.await.expect("closer task");
    assert_closed_before_activation(&error, "Timeout");
    let since_repend = returned.saturating_duration_since(repended_at);
    assert!(
        since_repend >= ESTABLISHMENT_SHORT,
        "the wait outlives the re-pended link's establishment timeout: {since_repend:?}"
    );
    assert!(
        since_repend < ESTABLISHMENT_SHORT + TIMEOUT_CLOSE_BOUND,
        "the wait ends with the timeout close, not at its own deadline: {since_repend:?}"
    );
    assert_eq!(link.lock().await.status(), LinkStatus::Closed);
    assert_eq!(link.lock().await.close_reason(), Some(LinkCloseReason::Timeout));

    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
}
