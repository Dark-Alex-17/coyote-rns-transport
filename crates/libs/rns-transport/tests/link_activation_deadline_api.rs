//! `delivery::await_link_activation` returns at its deadline, measured from outside the
//! crate: a short timeout answers `TimedOut` close to the timeout, and a timeout the clock
//! cannot hold waits instead of panicking. The transport here has broadcast on and no
//! interface, so `Transport::link` hands back a pending link that nothing can ever
//! activate, and every wait is decided by the deadline alone.

use std::io;
use std::time::{Duration, Instant};

use rns_transport::delivery::await_link_activation;
use rns_transport::destination::{DestinationDesc, DestinationName};
use rns_transport::identity::PrivateIdentity;
use rns_transport::rand_core::OsRng;
use rns_transport::transport::{Transport, TransportConfig};
use tokio::time::timeout;

/// A timeout short enough that the bound below can tell it from a stuck wait.
const SHORT: Duration = Duration::from_millis(100);
/// How far past `SHORT` a loaded host may land before the test calls the wait late.
const SHORT_BOUND: Duration = Duration::from_millis(500);
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

/// `Duration::MAX` waits instead of panicking: the pending link outlives an outer bound
/// rather than failing on the deadline sum, and a short timeout on the same link answers
/// `TimedOut` close to the timeout.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn max_timeout_waits_and_short_timeout_times_out() {
    let transport = broadcasting("link-activation-deadline");
    let link = transport.link(unknown_destination()).await.expect("pending link with broadcast on");

    let outcome =
        timeout(Duration::from_secs(1), await_link_activation(&transport, &link, Duration::MAX))
            .await;
    assert!(outcome.is_err(), "an unbounded activation wait on a pending link never returns");

    let started = Instant::now();
    let error = await_link_activation(&transport, &link, SHORT)
        .await
        .expect_err("a pending link with no interface never activates");
    let elapsed = started.elapsed();
    assert_eq!(error.kind(), io::ErrorKind::TimedOut, "unexpected error: {error}");
    assert!(elapsed >= SHORT, "the wait lasts its timeout, waited {elapsed:?}");
    assert!(elapsed < SHORT_BOUND, "the wait returns at its deadline, waited {elapsed:?}");

    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
}
