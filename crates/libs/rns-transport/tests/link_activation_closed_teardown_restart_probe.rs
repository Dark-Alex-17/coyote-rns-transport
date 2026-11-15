//! Usage-pattern probe for `delivery::await_link_activation` around the two other ways a
//! consumer moves a pending link through `Closed`, written from the contract alone: the wait
//! ends with `io::ErrorKind::ConnectionAborted` and `link closed before activation: <reason>`
//! as soon as the link reads `LinkStatus::Closed` or its `LinkEvent::Closed` arrives, and
//! only then. `tests/link_activation_closed_probe.rs` and
//! `tests/link_activation_closed_usage_probe.rs` pin `Link::close()` and the establishment
//! timeout; this file pins `Link::teardown()` (the close that would also tell the peer) under
//! a wait, and `Link::restart()` after a close: a restarted link reads `Pending` again with
//! no close reason, so a fresh wait on it must take the `TimedOut` path at its own deadline
//! rather than fail fast on the close that came before, and a second close ends it again.
//! The transport has broadcast on and no interface, so `Transport::link` hands back a
//! pending link that nothing can ever activate.

use std::io;
use std::sync::Arc;
use std::time::{Duration, Instant};

use rns_transport::delivery::await_link_activation;
use rns_transport::destination::link::{LinkCloseReason, LinkStatus};
use rns_transport::destination::{DestinationDesc, DestinationName};
use rns_transport::identity::PrivateIdentity;
use rns_transport::rand_core::OsRng;
use rns_transport::transport::{Transport, TransportConfig};
use tokio::time::timeout;

/// What the caller passes; every close-ended wait here has to return long before it.
const ACTIVATION_WAIT: Duration = Duration::from_secs(60);
/// A wait short enough that the bound below can tell its deadline from a stuck wait.
const SHORT_WAIT: Duration = Duration::from_millis(400);
/// How far past `SHORT_WAIT` a loaded host may land before the wait counts as late.
const SHORT_WAIT_BOUND: Duration = Duration::from_millis(1500);
/// An establishment timeout long enough that it cannot close the link under the test.
const ESTABLISHMENT_FAR: Duration = Duration::from_secs(60);
/// How long a wait on an already-closed link may take and still count as immediate.
const ALREADY_CLOSED_BOUND: Duration = Duration::from_secs(1);
/// When the closer task tears the pending link down by hand.
const HAND_CLOSE_AT: Duration = Duration::from_millis(500);
/// How late after `HAND_CLOSE_AT` a wait may return and still count as ending with the close.
const HAND_CLOSE_BOUND: Duration = Duration::from_millis(1500);
/// How long the transport is given to stop.
const WAIT: Duration = Duration::from_secs(15);

fn broadcasting(name: &str) -> Transport {
    Transport::new(TransportConfig::new(name, &PrivateIdentity::new_from_rand(OsRng), true))
}

fn unknown_destination() -> DestinationDesc {
    let remote = *PrivateIdentity::new_from_rand(OsRng).as_identity();
    DestinationDesc {
        identity: remote,
        address_hash: remote.address_hash,
        name: DestinationName::new("lxmf", "delivery"),
    }
}

fn assert_closed_before_activation(error: &io::Error, reason: &str) {
    assert_eq!(error.kind(), io::ErrorKind::ConnectionAborted, "unexpected error: {error}");
    assert_eq!(
        error.to_string(),
        format!("link closed before activation: {reason}"),
        "the error text names the close reason exactly as the contract spells it"
    );
}

/// `Link::teardown()` on a pending initiator link ends the activation wait with
/// `ConnectionAborted` naming `InitiatorClosed`, within the poll slice of the close, the same
/// way `Link::close()` does: the contract keys on the link reading `Closed`, not on which
/// call closed it.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn a_pending_link_torn_down_by_hand_ends_the_wait_naming_initiator_closed() {
    let transport = broadcasting("link-activation-closed-teardown");
    let link = transport.link(unknown_destination()).await.expect("pending link with broadcast on");
    link.lock().await.set_establishment_timeout(ESTABLISHMENT_FAR);

    let started = Instant::now();
    let closer = Arc::clone(&link);
    let closer_task = tokio::spawn(async move {
        tokio::time::sleep(HAND_CLOSE_AT).await;
        let _teardown_packet = closer.lock().await.teardown();
    });

    let error = await_link_activation(&transport, &link, ACTIVATION_WAIT)
        .await
        .expect_err("a pending link torn down by hand never activates");
    let elapsed = started.elapsed();
    assert_closed_before_activation(&error, "InitiatorClosed");
    assert!(elapsed >= HAND_CLOSE_AT, "the wait outlives the teardown, waited {elapsed:?}");
    assert!(
        elapsed < HAND_CLOSE_BOUND,
        "the wait ends with the teardown, not at its own deadline, waited {elapsed:?}"
    );
    closer_task.await.expect("closer task");
    assert_eq!(link.lock().await.status(), LinkStatus::Closed);
    assert_eq!(link.lock().await.close_reason(), Some(LinkCloseReason::InitiatorClosed));

    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
}

/// A closed link that the consumer restarts reads `Pending` again with no close reason, so
/// a fresh short wait on it takes the `TimedOut` path at its own deadline: the close that
/// came before the restart does not end a wait that started after it. Closing the restarted
/// link ends a further wait with `ConnectionAborted` once more.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn a_restarted_link_waits_to_its_deadline_and_a_second_close_ends_the_wait_again() {
    let transport = broadcasting("link-activation-closed-restart");
    let link = transport.link(unknown_destination()).await.expect("pending link with broadcast on");
    link.lock().await.set_establishment_timeout(ESTABLISHMENT_FAR);

    link.lock().await.close();
    let first = await_link_activation(&transport, &link, ACTIVATION_WAIT)
        .await
        .expect_err("a closed link never activates");
    assert_closed_before_activation(&first, "InitiatorClosed");

    {
        let mut guard = link.lock().await;
        guard.restart();
        guard.set_establishment_timeout(ESTABLISHMENT_FAR);
        assert_eq!(guard.status(), LinkStatus::Pending, "restart puts the link back to pending");
        assert_eq!(guard.close_reason(), None, "restart clears the close reason");
    }

    let started = Instant::now();
    let second = await_link_activation(&transport, &link, SHORT_WAIT)
        .await
        .expect_err("a restarted pending link with no interface never activates");
    let elapsed = started.elapsed();
    assert_eq!(
        second.kind(),
        io::ErrorKind::TimedOut,
        "a restarted link is pending, not closed; the wait reaches its deadline: {second}"
    );
    assert!(elapsed >= SHORT_WAIT, "the wait lasts its timeout, waited {elapsed:?}");
    assert!(elapsed < SHORT_WAIT_BOUND, "the wait returns at its deadline, waited {elapsed:?}");
    assert_eq!(link.lock().await.status(), LinkStatus::Pending, "the deadline does not close");

    link.lock().await.close();
    let started = Instant::now();
    let third = await_link_activation(&transport, &link, ACTIVATION_WAIT)
        .await
        .expect_err("a closed link never activates");
    let elapsed = started.elapsed();
    assert_closed_before_activation(&third, "InitiatorClosed");
    assert!(
        elapsed < ALREADY_CLOSED_BOUND,
        "the second close ends the wait at once, waited {elapsed:?}"
    );

    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
}
