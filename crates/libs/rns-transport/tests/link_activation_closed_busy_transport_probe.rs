//! Usage-pattern probe for `delivery::await_link_activation` on a busy transport, written
//! from the contract alone: the wait ends with `io::ErrorKind::ConnectionAborted` and the
//! text `link closed before activation: <reason>` as soon as the link reads
//! `LinkStatus::Closed` (a `LinkEvent::Closed` for its id on `Transport::out_link_events`
//! only wakes the reread), its own deadline still answers `TimedOut`, and `UnexpectedEof`
//! is reserved for an event channel that is gone. `tests/link_activation_closed_probe.rs`
//! and `tests/link_activation_closed_usage_probe.rs` pin the closes a quiet transport hands
//! a single waiter; this file pins the shapes a consumer meets on a loaded one: a `Closed`
//! event that the broadcast channel dropped before the waiter read it (more than the
//! channel's 1024 events land in one burst, so the status read is all that can answer), a
//! first wait the caller cancelled under `tokio::time::timeout` followed by a second wait on
//! the same link, and the README's claim that a `link` to a destination without a path entry
//! carries the transport-sized establishment timeout of about 774 s until the caller shortens
//! it. The transport has broadcast on and no interface, so `Transport::link` hands back a
//! pending link that nothing can ever activate.

use std::io;
use std::sync::Arc;
use std::time::{Duration, Instant};

use rns_transport::delivery::await_link_activation;
use rns_transport::destination::link::{Link, LinkCloseReason, LinkStatus};
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
/// When the closer task closes the pending link by hand.
const HAND_CLOSE_AT: Duration = Duration::from_millis(500);
/// How late after `HAND_CLOSE_AT` a wait may return and still count as ending with the close.
const HAND_CLOSE_BOUND: Duration = Duration::from_millis(1500);
/// More closes than `Transport::out_link_events` can hold (1024), so a subscriber that has
/// not been polled in between loses the oldest of them.
const BURST: usize = 1100;
/// `6 s + 6 s x PATHFINDER_M (128)`, the README's transport-sized establishment timeout.
const TRANSPORT_SIZED_ESTABLISHMENT: Duration = Duration::from_secs(774);
/// Slack either side of the transport-sized timeout for the moment the link was made.
const TRANSPORT_SIZED_SLACK: Duration = Duration::from_secs(10);
/// How long the transport is given to stop.
const WAIT: Duration = Duration::from_secs(15);

/// Closes a link without yielding, so a burst of these lands on the event channel before
/// any subscriber gets to read it. Nothing else holds these locks, so `try_lock` succeeds.
fn close_now(link: &Arc<tokio::sync::Mutex<Link>>) {
    link.try_lock().expect("no one else holds a pending link the test made").close();
}

/// The target link is closed first and then more links than the event channel holds are
/// closed in one burst, so the waiter's copy of the target's `Closed` event is dropped as
/// the oldest. The contract's status read still ends the wait with `ConnectionAborted`
/// naming `InitiatorClosed`, not `UnexpectedEof` (the channel is lagging, not gone) and not
/// the caller's own `TimedOut`.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn a_close_the_event_channel_dropped_still_ends_the_wait() {
    let transport = Arc::new(broadcasting("link-activation-closed-busy-lagged"));
    let target =
        transport.link(unknown_destination()).await.expect("pending link with broadcast on");
    target.lock().await.set_establishment_timeout(ESTABLISHMENT_FAR);
    let mut others = Vec::with_capacity(BURST);
    for _ in 0..BURST {
        let link =
            transport.link(unknown_destination()).await.expect("pending link with broadcast on");
        link.lock().await.set_establishment_timeout(ESTABLISHMENT_FAR);
        others.push(link);
    }

    let started = Instant::now();
    let waiter_transport = Arc::clone(&transport);
    let waiter_link = Arc::clone(&target);
    let waiter = tokio::spawn(async move {
        await_link_activation(&waiter_transport, &waiter_link, ACTIVATION_WAIT).await
    });

    tokio::time::sleep(HAND_CLOSE_AT).await;
    // A witness subscribed just before the burst shows that the burst really overflowed
    // the channel: its first read is `Lagged`, so the waiter's copy of the target's
    // `Closed` event, the oldest of the burst, was dropped the same way.
    let mut witness = transport.out_link_events();
    close_now(&target);
    for link in &others {
        close_now(link);
    }
    let burst_done = started.elapsed();
    assert!(
        matches!(witness.try_recv(), Err(tokio::sync::broadcast::error::TryRecvError::Lagged(_))),
        "the burst overflows the event channel, so the oldest close is dropped"
    );

    let error = waiter
        .await
        .expect("waiter task")
        .expect_err("a pending link closed by hand never activates");
    let elapsed = started.elapsed();
    assert_closed_before_activation(&error, "InitiatorClosed");
    assert!(elapsed >= HAND_CLOSE_AT, "the wait outlives the hand close, waited {elapsed:?}");
    assert!(
        elapsed < burst_done + HAND_CLOSE_BOUND,
        "the wait ends with the close, not at its own deadline, waited {elapsed:?} \
         (burst done at {burst_done:?})"
    );
    assert_eq!(target.lock().await.close_reason(), Some(LinkCloseReason::InitiatorClosed));

    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
}

/// A caller that gives up on a first wait under `tokio::time::timeout` (dropping the future
/// mid-wait) and then waits again on the same pending link is told of the close that lands
/// under the second wait, within the poll slice, with `ConnectionAborted` naming
/// `InitiatorClosed`; the cancelled wait leaves nothing behind that the second one trips on.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn a_cancelled_wait_leaves_a_second_wait_able_to_see_the_close() {
    let transport = broadcasting("link-activation-closed-busy-cancelled");
    let link = transport.link(unknown_destination()).await.expect("pending link with broadcast on");
    link.lock().await.set_establishment_timeout(ESTABLISHMENT_FAR);

    let cancelled = timeout(
        Duration::from_millis(200),
        await_link_activation(&transport, &link, ACTIVATION_WAIT),
    )
    .await;
    assert!(cancelled.is_err(), "the first wait is cancelled by the caller, not answered");
    assert_ne!(link.lock().await.status(), LinkStatus::Closed, "a cancelled wait closes nothing");

    let closer = Arc::clone(&link);
    let closer_task = tokio::spawn(async move {
        tokio::time::sleep(HAND_CLOSE_AT).await;
        closer.lock().await.close();
    });

    let started = Instant::now();
    let error = await_link_activation(&transport, &link, ACTIVATION_WAIT)
        .await
        .expect_err("a pending link closed by hand never activates");
    let elapsed = started.elapsed();
    assert_closed_before_activation(&error, "InitiatorClosed");
    assert!(elapsed >= HAND_CLOSE_AT, "the wait outlives the hand close, waited {elapsed:?}");
    assert!(
        elapsed < HAND_CLOSE_BOUND,
        "the wait ends with the close, not at its own deadline, waited {elapsed:?}"
    );
    closer_task.await.expect("closer task");

    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
}

/// The README's sizing claim, as a caller would meet it: a `link` to a destination without
/// a path entry, with `broadcast` on and nothing shortened, carries an establishment
/// timeout of about 774 s (it has not timed out 10 s short of that mark and has 10 s past
/// it), so a short activation wait on it answers `TimedOut` and leaves the link pending with
/// no close reason. Shortening it is the caller's job via `Link::set_establishment_timeout`.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn a_link_without_a_path_carries_the_transport_sized_establishment_timeout() {
    let transport = broadcasting("link-activation-closed-busy-default-sizing");
    let made_at = Instant::now();
    let link = transport.link(unknown_destination()).await.expect("pending link with broadcast on");

    {
        let link = link.lock().await;
        assert!(
            !link.establishment_timed_out(
                made_at + TRANSPORT_SIZED_ESTABLISHMENT - TRANSPORT_SIZED_SLACK
            ),
            "the default establishment timeout is not shorter than the README's 774 s"
        );
        assert!(
            link.establishment_timed_out(
                made_at + TRANSPORT_SIZED_ESTABLISHMENT + TRANSPORT_SIZED_SLACK
            ),
            "the default establishment timeout is not longer than the README's 774 s"
        );
    }

    let error = await_link_activation(&transport, &link, Duration::from_millis(300))
        .await
        .expect_err("a pending link with no interface never activates");
    assert_eq!(error.kind(), io::ErrorKind::TimedOut, "unexpected error: {error}");
    assert_eq!(link.lock().await.status(), LinkStatus::Pending, "the deadline closes nothing");
    assert_eq!(link.lock().await.close_reason(), None);

    link.lock().await.set_establishment_timeout(Duration::from_secs(1));
    assert!(
        link.lock().await.establishment_timed_out(made_at + Duration::from_secs(2)),
        "the caller's shorter establishment timeout takes effect on the returned link"
    );

    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
}
