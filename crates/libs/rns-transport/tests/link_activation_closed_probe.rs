//! `delivery::await_link_activation` ends when the link it waits on is closed, measured
//! from outside the crate: a caller waiting on a link that dies learns of it at the close,
//! within the 250 ms poll slice, with the close reason in a `ConnectionAborted` error,
//! instead of at its own deadline. Python's watchdog closes a pending link at its
//! establishment timeout with `teardown_reason = TIMEOUT` (`RNS/Link.py:722-729`); here the
//! link maintenance job does the same once per second, and `Link::close()` by hand stands
//! in for the initiator giving up. The transport has broadcast on and no interface, so
//! `Transport::link` hands back a pending link that nothing can ever activate.

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

/// The shortened establishment timeout the maintenance job closes the link at.
const ESTABLISHMENT: Duration = Duration::from_secs(2);
/// An establishment timeout long enough that it cannot close the link under the test.
const ESTABLISHMENT_FAR: Duration = Duration::from_secs(60);
/// What the caller passes; the wait has to return long before it.
const ACTIVATION_WAIT: Duration = Duration::from_secs(60);
/// How late after `ESTABLISHMENT` the 1 s maintenance tick may land on a loaded host.
const CLOSE_BOUND: Duration = Duration::from_secs(4);
/// When the closer task closes the pending link by hand.
const HAND_CLOSE_AT: Duration = Duration::from_millis(500);
/// How late after `HAND_CLOSE_AT` the wait may return and still count as ending with the close.
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

/// A 2 s establishment timeout on a pending link ends a 60 s activation wait with
/// `ConnectionAborted` naming `Timeout`, within the next maintenance tick of the close.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn a_timed_out_establishment_ends_the_activation_wait_with_the_close_reason() {
    let transport = broadcasting("link-activation-closed-timeout");
    let link = transport.link(unknown_destination()).await.expect("pending link with broadcast on");
    link.lock().await.set_establishment_timeout(ESTABLISHMENT);

    let started = Instant::now();
    let error = await_link_activation(&transport, &link, ACTIVATION_WAIT)
        .await
        .expect_err("a pending link with no interface never activates");
    let elapsed = started.elapsed();
    assert_eq!(error.kind(), io::ErrorKind::ConnectionAborted, "unexpected error: {error}");
    assert!(error.to_string().contains("Timeout"), "the close reason is in the error: {error}");
    assert!(
        elapsed >= ESTABLISHMENT,
        "the wait outlives the establishment timeout, waited {elapsed:?}"
    );
    assert!(
        elapsed < CLOSE_BOUND,
        "the wait ends with the close, not at its own deadline, waited {elapsed:?}"
    );
    assert_eq!(link.lock().await.status(), LinkStatus::Closed);
    assert_eq!(link.lock().await.close_reason(), Some(LinkCloseReason::Timeout));

    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
}

/// `Link::close()` on a pending link ends the activation wait with `ConnectionAborted`
/// naming `InitiatorClosed`, within the poll slice of the close.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn a_link_closed_by_the_initiator_ends_the_activation_wait_with_initiator_closed() {
    let transport = broadcasting("link-activation-closed-by-hand");
    let link = transport.link(unknown_destination()).await.expect("pending link with broadcast on");
    link.lock().await.set_establishment_timeout(ESTABLISHMENT_FAR);

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
    assert_eq!(error.kind(), io::ErrorKind::ConnectionAborted, "unexpected error: {error}");
    assert!(
        error.to_string().contains("InitiatorClosed"),
        "the close reason is in the error: {error}"
    );
    assert!(elapsed >= HAND_CLOSE_AT, "the wait outlives the hand close, waited {elapsed:?}");
    assert!(
        elapsed < HAND_CLOSE_BOUND,
        "the wait ends with the close, not at its own deadline, waited {elapsed:?}"
    );
    closer_task.await.expect("closer task");

    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
}
