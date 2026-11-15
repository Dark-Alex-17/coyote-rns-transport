//! Usage-pattern probe for `delivery::await_link_activation` on a closed link, written from
//! the contract alone: the wait ends with `io::ErrorKind::ConnectionAborted` and the text
//! `link closed before activation: <reason>` as soon as the link reads `LinkStatus::Closed`
//! or its `LinkEvent::Closed` arrives. `tests/link_activation_closed_probe.rs` pins the two
//! closes that land under a single wait; this file pins the shapes a consumer meets around
//! them: a link that is already closed when the wait starts (no event can reach a subscriber
//! that joins late, so the status read has to answer), the exact error text, a second wait on
//! the same closed link, and two waiters on one link both learning of the one close. The
//! transport has broadcast on and no interface, so `Transport::link` hands back a pending
//! link that nothing can ever activate.

use std::io;
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

/// What the caller passes; every wait here has to return long before it.
const ACTIVATION_WAIT: Duration = Duration::from_secs(60);
/// An establishment timeout long enough that it cannot close the link under the test.
const ESTABLISHMENT_FAR: Duration = Duration::from_secs(60);
/// The shortened establishment timeout the maintenance job closes the link at.
const ESTABLISHMENT: Duration = Duration::from_secs(2);
/// How long a wait on an already-closed link may take and still count as immediate: the
/// contract reads the status in the loop head, so no poll slice or event is needed.
const ALREADY_CLOSED_BOUND: Duration = Duration::from_secs(1);
/// When the closer task closes the pending link by hand.
const HAND_CLOSE_AT: Duration = Duration::from_millis(500);
/// How late after `HAND_CLOSE_AT` a wait may return and still count as ending with the close.
const HAND_CLOSE_BOUND: Duration = Duration::from_millis(1500);
/// How late after `ESTABLISHMENT` the 1 s maintenance tick may land on a loaded host.
const CLOSE_BOUND: Duration = Duration::from_secs(4);
/// How long the transport is given to stop.
const WAIT: Duration = Duration::from_secs(15);

/// A link that is already closed by hand when the wait starts answers at once with
/// `ConnectionAborted` naming `InitiatorClosed`: the `Closed` event went out before the wait
/// subscribed, so only the status read can end it.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn a_link_already_closed_by_hand_ends_the_wait_at_once() {
    let transport = broadcasting("link-activation-already-closed-by-hand");
    let link = transport.link(unknown_destination()).await.expect("pending link with broadcast on");
    link.lock().await.set_establishment_timeout(ESTABLISHMENT_FAR);
    link.lock().await.close();
    assert_eq!(link.lock().await.status(), LinkStatus::Closed);

    let started = Instant::now();
    let error = await_link_activation(&transport, &link, ACTIVATION_WAIT)
        .await
        .expect_err("a closed link never activates");
    let elapsed = started.elapsed();
    assert_closed_before_activation(&error, "InitiatorClosed");
    assert!(
        elapsed < ALREADY_CLOSED_BOUND,
        "an already-closed link ends the wait at once, waited {elapsed:?}"
    );

    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
}

/// A link the maintenance job already closed at its establishment timeout, before any wait
/// began, answers at once with `ConnectionAborted` naming `Timeout`.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn a_link_already_timed_out_ends_the_wait_at_once_naming_timeout() {
    let transport = broadcasting("link-activation-already-timed-out");
    let link = transport.link(unknown_destination()).await.expect("pending link with broadcast on");
    link.lock().await.set_establishment_timeout(ESTABLISHMENT);

    let closed_by = Instant::now() + CLOSE_BOUND;
    loop {
        if link.lock().await.status() == LinkStatus::Closed {
            break;
        }
        assert!(Instant::now() < closed_by, "the maintenance job closes the link at its timeout");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert_eq!(link.lock().await.close_reason(), Some(LinkCloseReason::Timeout));

    let started = Instant::now();
    let error = await_link_activation(&transport, &link, ACTIVATION_WAIT)
        .await
        .expect_err("a closed link never activates");
    let elapsed = started.elapsed();
    assert_closed_before_activation(&error, "Timeout");
    assert!(
        elapsed < ALREADY_CLOSED_BOUND,
        "an already-closed link ends the wait at once, waited {elapsed:?}"
    );

    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
}

/// A second wait on a link whose first wait ended with the close answers the same way, at
/// once: the close is a property of the link, not of the event the first wait took.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn a_repeated_wait_on_a_closed_link_answers_the_same_way() {
    let transport = broadcasting("link-activation-closed-repeat-wait");
    let link = transport.link(unknown_destination()).await.expect("pending link with broadcast on");
    link.lock().await.set_establishment_timeout(ESTABLISHMENT_FAR);

    let closer = Arc::clone(&link);
    let closer_task = tokio::spawn(async move {
        tokio::time::sleep(HAND_CLOSE_AT).await;
        closer.lock().await.close();
    });

    let first = await_link_activation(&transport, &link, ACTIVATION_WAIT)
        .await
        .expect_err("a pending link closed by hand never activates");
    assert_closed_before_activation(&first, "InitiatorClosed");
    closer_task.await.expect("closer task");

    let started = Instant::now();
    let second = await_link_activation(&transport, &link, ACTIVATION_WAIT)
        .await
        .expect_err("a closed link stays closed");
    let elapsed = started.elapsed();
    assert_closed_before_activation(&second, "InitiatorClosed");
    assert!(
        elapsed < ALREADY_CLOSED_BOUND,
        "the second wait on a closed link ends at once, waited {elapsed:?}"
    );

    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
}

/// Two waiters on the same pending link both end with the one close, each with
/// `ConnectionAborted` naming `InitiatorClosed`, within the poll slice of the close.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_waiters_on_one_link_both_learn_of_the_close() {
    let transport = Arc::new(broadcasting("link-activation-closed-two-waiters"));
    let link = transport.link(unknown_destination()).await.expect("pending link with broadcast on");
    link.lock().await.set_establishment_timeout(ESTABLISHMENT_FAR);

    let started = Instant::now();
    let closer = Arc::clone(&link);
    let closer_task = tokio::spawn(async move {
        tokio::time::sleep(HAND_CLOSE_AT).await;
        closer.lock().await.close();
    });

    let second_transport = Arc::clone(&transport);
    let second_link = Arc::clone(&link);
    let second_waiter = tokio::spawn(async move {
        await_link_activation(&second_transport, &second_link, ACTIVATION_WAIT).await
    });
    let first = await_link_activation(&transport, &link, ACTIVATION_WAIT)
        .await
        .expect_err("a pending link closed by hand never activates");
    let first_elapsed = started.elapsed();
    let second = second_waiter
        .await
        .expect("second waiter task")
        .expect_err("a pending link closed by hand never activates");
    let second_elapsed = started.elapsed();
    closer_task.await.expect("closer task");

    assert_closed_before_activation(&first, "InitiatorClosed");
    assert_closed_before_activation(&second, "InitiatorClosed");
    for (who, elapsed) in [("first", first_elapsed), ("second", second_elapsed)] {
        assert!(elapsed >= HAND_CLOSE_AT, "the {who} wait outlives the hand close: {elapsed:?}");
        assert!(
            elapsed < HAND_CLOSE_BOUND,
            "the {who} wait ends with the close, not at its own deadline: {elapsed:?}"
        );
    }

    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
}

/// The `TimedOut` path is untouched next to the new exit: a short wait on a pending link
/// that nothing closes still answers `TimedOut`, and the link is still pending afterwards.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn a_short_wait_on_an_open_pending_link_still_times_out() {
    let transport = broadcasting("link-activation-closed-probe-timed-out-path");
    let link = transport.link(unknown_destination()).await.expect("pending link with broadcast on");
    link.lock().await.set_establishment_timeout(ESTABLISHMENT_FAR);

    let error = await_link_activation(&transport, &link, Duration::from_millis(300))
        .await
        .expect_err("a pending link with no interface never activates");
    assert_eq!(error.kind(), io::ErrorKind::TimedOut, "unexpected error: {error}");
    assert_ne!(
        link.lock().await.status(),
        LinkStatus::Closed,
        "the deadline does not close the link"
    );
    assert_eq!(link.lock().await.close_reason(), None);

    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
}
