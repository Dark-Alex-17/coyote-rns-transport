//! `Transport::resolve_destination` and `Transport::await_path` return at their deadline,
//! measured from outside the crate: a timeout shorter than the 50 ms poll returns at the
//! timeout, not a poll later; a timeout that is not a whole number of polls returns at the
//! timeout, its last poll cut to the time left; and a timeout the clock cannot hold waits
//! instead of panicking even on a transport with no interface to broadcast on. The
//! transport here has no interface at all, so nothing can answer and every miss is decided
//! by the deadline alone. Each timing is the fastest of several runs: a loaded host can
//! only make a run slower, never faster, so the fastest run is what the code promises, and
//! a loop that slept a whole poll past the deadline could never read under it.

use std::time::{Duration, Instant};

use rns_transport::hash::AddressHash;
use rns_transport::identity::PrivateIdentity;
use rns_transport::rand_core::OsRng;
use rns_transport::transport::{Transport, TransportConfig};
use tokio::time::timeout;

/// The poll the call sleeps between table reads, as the README and rustdoc name it.
const POLL: Duration = Duration::from_millis(50);
/// A timeout shorter than one poll.
const SHORT: Duration = Duration::from_millis(10);
/// A timeout of two polls and a fraction, so the last sleep must be cut to the time left.
const UNEVEN: Duration = Duration::from_millis(120);
/// How far under the next whole poll the fastest run must land: a loop that rounds the
/// last sleep up to a poll reads at least one poll past the deadline, so the fastest run
/// of the fixed code sits well below that by this margin.
const MARGIN: Duration = Duration::from_millis(10);
/// How many runs each timing takes the fastest of.
const RUNS: usize = 3;
/// How long the transport is given to stop.
const WAIT: Duration = Duration::from_secs(15);

fn isolated(name: &str) -> Transport {
    Transport::new(TransportConfig::new(name, &PrivateIdentity::new_from_rand(OsRng), false))
}

/// The fastest of `RUNS` misses of `resolve_destination` on a fresh unknown destination,
/// every run checked to wait out its timeout and to answer `None`.
#[deny(clippy::await_holding_invalid_type)]
async fn fastest_resolve_miss(transport: &Transport, budget: Duration) -> Duration {
    let mut fastest = Duration::MAX;
    for _ in 0..RUNS {
        let unknown = AddressHash::new_from_rand(OsRng);
        let started = Instant::now();
        let missed = transport.resolve_destination(&unknown, budget).await;
        let elapsed = started.elapsed();
        assert!(missed.is_none(), "nothing can answer without an interface");
        assert!(elapsed >= budget, "the call waits out its timeout, waited {elapsed:?}");
        fastest = fastest.min(elapsed);
    }
    fastest
}

/// The fastest of `RUNS` misses of `await_path` on a fresh unknown destination, every run
/// checked to wait out its timeout and to answer `false`.
#[deny(clippy::await_holding_invalid_type)]
async fn fastest_await_path_miss(transport: &Transport, budget: Duration) -> Duration {
    let mut fastest = Duration::MAX;
    for _ in 0..RUNS {
        let unknown = AddressHash::new_from_rand(OsRng);
        let started = Instant::now();
        let found = transport.await_path(&unknown, budget, None).await;
        let elapsed = started.elapsed();
        assert!(!found, "nobody holds a path without an interface");
        assert!(elapsed >= budget, "the wait lasts its timeout, waited {elapsed:?}");
        fastest = fastest.min(elapsed);
    }
    fastest
}

/// A timeout shorter than one poll returns at the timeout: the fastest miss sits under the
/// poll by the margin, where a full poll past the deadline would read at least `POLL`.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn resolve_destination_shorter_than_a_poll_returns_at_the_timeout() {
    let transport = isolated("deadline-short-resolve");
    let fastest = fastest_resolve_miss(&transport, SHORT).await;
    assert!(
        fastest < POLL - MARGIN,
        "a timeout shorter than a poll returns at the deadline, not a poll later: fastest miss {fastest:?}"
    );
    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
}

/// The same edge on `await_path`, which `resolve_destination` delegates its wait to.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn await_path_shorter_than_a_poll_returns_at_the_timeout() {
    let transport = isolated("deadline-short-await-path");
    let fastest = fastest_await_path_miss(&transport, SHORT).await;
    assert!(
        fastest < POLL - MARGIN,
        "a timeout shorter than a poll returns at the deadline, not a poll later: fastest miss {fastest:?}"
    );
    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
}

/// A timeout that is not a whole number of polls returns at the timeout: two polls and a
/// 20 ms remainder sit under three polls by the margin, where rounding the last sleep up
/// to a poll would read at least three.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn resolve_destination_cuts_the_last_poll_to_the_time_left() {
    let transport = isolated("deadline-uneven-resolve");
    let fastest = fastest_resolve_miss(&transport, UNEVEN).await;
    assert!(
        fastest < 3 * POLL - MARGIN,
        "the last poll is cut to the time left, not rounded up to a poll: fastest miss {fastest:?}"
    );
    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
}

/// `Duration::MAX` waits instead of panicking even where there is nothing to broadcast on:
/// an isolated transport has no interface, the request has no wire, and both calls still
/// outlive an outer bound rather than failing on the deadline sum.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn max_timeout_waits_without_an_interface() {
    let transport = isolated("deadline-forever");
    let unknown = AddressHash::new_from_rand(OsRng);

    let outcome =
        timeout(Duration::from_secs(1), transport.resolve_destination(&unknown, Duration::MAX))
            .await;
    assert!(outcome.is_err(), "an unbounded resolve with no interface never returns");

    let outcome =
        timeout(Duration::from_secs(1), transport.await_path(&unknown, Duration::MAX, None)).await;
    assert!(outcome.is_err(), "an unbounded path wait with no interface never returns");

    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
}
