//! Usage-pattern probe for `delivery::await_link_activation` with more than one pending link
//! on the transport, written from the contract alone: the wait ends with
//! `io::ErrorKind::ConnectionAborted` when the link it waits on reads `LinkStatus::Closed` or
//! a `LinkEvent::Closed` for ITS id arrives on `Transport::out_link_events`, and only then.
//! `tests/link_activation_closed_probe.rs` pins the close of the waited-on link; this file
//! pins that the close of a neighbour does not leak into the wait: a hand close of another
//! pending link, and the establishment-timeout close of another pending link (the event that
//! carries `Timeout`), both leave the wait on the first link running to its own deadline
//! (`TimedOut`) or its own close (`ConnectionAborted` with the first link's reason). The
//! transport has broadcast on and no interface, so every `Transport::link` hands back a
//! pending link that nothing can ever activate.

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

/// The shortened establishment timeout the maintenance job closes the neighbour at.
const ESTABLISHMENT: Duration = Duration::from_secs(2);
/// An establishment timeout long enough that it cannot close a link under the test.
const ESTABLISHMENT_FAR: Duration = Duration::from_secs(60);
/// A caller deadline the wait has to reach when only the neighbour closes.
const SHORT_WAIT: Duration = Duration::from_secs(3);
/// A caller deadline that outlives a 2 s establishment timeout plus the 1 s maintenance tick.
const PAST_NEIGHBOUR_TIMEOUT_WAIT: Duration = Duration::from_secs(5);
/// What the caller passes when its own link is closed by hand; the wait returns long before it.
const LONG_WAIT: Duration = Duration::from_secs(60);
/// When the closer task closes the neighbour by hand.
const NEIGHBOUR_CLOSE_AT: Duration = Duration::from_millis(300);
/// When the closer task closes the waited-on link by hand.
const OWN_CLOSE_AT: Duration = Duration::from_millis(1200);
/// How late after `OWN_CLOSE_AT` the wait may return and still count as ending with the close.
const OWN_CLOSE_BOUND: Duration = Duration::from_millis(1500);
/// How much later than its deadline a `TimedOut` wait may return on a loaded host.
const DEADLINE_SLACK: Duration = Duration::from_millis(1500);
/// How long the transport is given to stop.
const WAIT: Duration = Duration::from_secs(15);

/// `Link::close()` on a neighbouring pending link does not end the wait on another: the
/// wait runs to its own deadline with `TimedOut` and the waited-on link is still `Pending`.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn a_hand_close_of_another_link_does_not_end_the_wait() {
    let transport = broadcasting("link-activation-closed-neighbour-hand");
    let waited =
        transport.link(unknown_destination()).await.expect("pending link with broadcast on");
    let neighbour =
        transport.link(unknown_destination()).await.expect("pending link with broadcast on");
    let waited_id = *waited.lock().await.id();
    let neighbour_id = *neighbour.lock().await.id();
    assert_ne!(waited_id, neighbour_id, "two distinct links");
    waited.lock().await.set_establishment_timeout(ESTABLISHMENT_FAR);
    neighbour.lock().await.set_establishment_timeout(ESTABLISHMENT_FAR);

    let started = Instant::now();
    let closer = Arc::clone(&neighbour);
    let closer_task = tokio::spawn(async move {
        tokio::time::sleep(NEIGHBOUR_CLOSE_AT).await;
        closer.lock().await.close();
    });

    let error = await_link_activation(&transport, &waited, SHORT_WAIT)
        .await
        .expect_err("a pending link with no interface never activates");
    let elapsed = started.elapsed();
    assert_eq!(
        error.kind(),
        io::ErrorKind::TimedOut,
        "the neighbour's close must not end this wait: {error}"
    );
    assert_eq!(error.to_string(), "link activation timed out");
    assert!(elapsed >= SHORT_WAIT, "the wait reaches its own deadline, waited {elapsed:?}");
    assert!(
        elapsed < SHORT_WAIT + DEADLINE_SLACK,
        "the wait returns close to its deadline, waited {elapsed:?}"
    );
    closer_task.await.expect("closer task");
    assert_eq!(waited.lock().await.status(), LinkStatus::Pending);
    assert_eq!(waited.lock().await.close_reason(), None);
    assert_eq!(neighbour.lock().await.status(), LinkStatus::Closed);
    assert_eq!(neighbour.lock().await.close_reason(), Some(LinkCloseReason::InitiatorClosed));

    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
}

/// The establishment-timeout close of a neighbouring pending link (the `Closed` event that
/// carries `Timeout`) does not end the wait on another link whose own timeout is far off.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn another_links_establishment_timeout_does_not_end_the_wait() {
    let transport = broadcasting("link-activation-closed-neighbour-timeout");
    let waited =
        transport.link(unknown_destination()).await.expect("pending link with broadcast on");
    let neighbour =
        transport.link(unknown_destination()).await.expect("pending link with broadcast on");
    let waited_id = *waited.lock().await.id();
    let neighbour_id = *neighbour.lock().await.id();
    assert_ne!(waited_id, neighbour_id, "two distinct links");
    waited.lock().await.set_establishment_timeout(ESTABLISHMENT_FAR);
    neighbour.lock().await.set_establishment_timeout(ESTABLISHMENT);

    let started = Instant::now();
    let error = await_link_activation(&transport, &waited, PAST_NEIGHBOUR_TIMEOUT_WAIT)
        .await
        .expect_err("a pending link with no interface never activates");
    let elapsed = started.elapsed();
    assert_eq!(
        error.kind(),
        io::ErrorKind::TimedOut,
        "the neighbour's timeout close must not end this wait: {error}"
    );
    assert_eq!(error.to_string(), "link activation timed out");
    assert!(
        elapsed >= PAST_NEIGHBOUR_TIMEOUT_WAIT,
        "the wait reaches its own deadline, waited {elapsed:?}"
    );
    assert_eq!(waited.lock().await.status(), LinkStatus::Pending);
    assert_eq!(waited.lock().await.close_reason(), None);
    assert_eq!(
        neighbour.lock().await.status(),
        LinkStatus::Closed,
        "the neighbour was closed by its establishment timeout under the wait"
    );
    assert_eq!(neighbour.lock().await.close_reason(), Some(LinkCloseReason::Timeout));

    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
}

/// With a neighbour closed first and the waited-on link closed later, the wait ends at the
/// later close with the waited-on link's own reason, not at the neighbour's close.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn the_wait_ends_at_its_own_close_after_a_neighbour_closed_first() {
    let transport = broadcasting("link-activation-closed-neighbour-then-own");
    let waited =
        transport.link(unknown_destination()).await.expect("pending link with broadcast on");
    let neighbour =
        transport.link(unknown_destination()).await.expect("pending link with broadcast on");
    let waited_id = *waited.lock().await.id();
    let neighbour_id = *neighbour.lock().await.id();
    assert_ne!(waited_id, neighbour_id, "two distinct links");
    waited.lock().await.set_establishment_timeout(ESTABLISHMENT_FAR);
    neighbour.lock().await.set_establishment_timeout(ESTABLISHMENT_FAR);

    let started = Instant::now();
    let neighbour_closer = Arc::clone(&neighbour);
    let own_closer = Arc::clone(&waited);
    let closer_task = tokio::spawn(async move {
        tokio::time::sleep(NEIGHBOUR_CLOSE_AT).await;
        neighbour_closer.lock().await.close();
        tokio::time::sleep(OWN_CLOSE_AT - NEIGHBOUR_CLOSE_AT).await;
        own_closer.lock().await.close();
    });

    let error = await_link_activation(&transport, &waited, LONG_WAIT)
        .await
        .expect_err("a pending link closed by hand never activates");
    let elapsed = started.elapsed();
    assert_closed_before_activation(&error, "InitiatorClosed");
    assert!(
        elapsed >= OWN_CLOSE_AT,
        "the wait outlives the neighbour's close and ends at its own, waited {elapsed:?}"
    );
    assert!(
        elapsed < OWN_CLOSE_AT + OWN_CLOSE_BOUND,
        "the wait ends with its own close, not at its deadline, waited {elapsed:?}"
    );
    closer_task.await.expect("closer task");
    assert_eq!(waited.lock().await.status(), LinkStatus::Closed);
    assert_eq!(waited.lock().await.close_reason(), Some(LinkCloseReason::InitiatorClosed));

    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
}
