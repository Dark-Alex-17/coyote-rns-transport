//! Usage-pattern probe for `delivery::await_link_activation` on the two points its contract
//! makes about the `Closed` event, written from the contract alone: a `LinkEvent::Closed`
//! for the waited-on link on `Transport::out_link_events` "wakes the wait so it rereads the
//! link without waiting out the poll slice", and the wait ends with `ConnectionAborted`
//! only "as soon as the link reads `LinkStatus::Closed`". `tests/link_activation_closed_probe.rs`
//! bounds a hand close at 1.5 s, which a 250 ms poll alone would also meet; the first test
//! here measures the close-to-return latency over several links so a wake is told apart
//! from a poll. The second test is the one shape where "the event wakes a reread" and "the
//! event ends the wait" answer differently: a link closed and restarted inside one lock
//! scope, before the waiter runs again, reads `Pending` with no reason at the reread, so the
//! wait keeps going to its own deadline (`TimedOut`) instead of answering a stale
//! `ConnectionAborted` for a link that is pending again. `Link::restart()` is public, so a
//! consumer can reach this; the test pins the contract's answer. The transport has
//! broadcast on and no interface, so `Transport::link` hands back a pending link that
//! nothing can ever activate.

use std::io;
use std::sync::Arc;
use std::time::{Duration, Instant};

use rns_transport::delivery::await_link_activation;
use rns_transport::destination::link::LinkStatus;
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
/// What the caller passes when its link is closed by hand; the wait returns long before it.
const LONG_WAIT: Duration = Duration::from_secs(60);
/// A caller deadline the wait has to reach when the link is pending again at the reread.
const SHORT_WAIT: Duration = Duration::from_secs(2);
/// How much later than its deadline a `TimedOut` wait may return on a loaded host.
const DEADLINE_SLACK: Duration = Duration::from_millis(1500);
/// When the closer task acts on the pending link.
const CLOSE_AT: Duration = Duration::from_millis(300);
/// How many fresh links the latency is sampled over.
const SAMPLES: usize = 6;
/// A close-to-return latency a poll slice (250 ms) would exceed more often than not; a
/// woken reread lands in a few milliseconds.
const WAKE_BOUND: Duration = Duration::from_millis(100);
/// How many of the samples must land under `WAKE_BOUND`; one scheduling spike is allowed.
const WAKE_SAMPLES_REQUIRED: usize = SAMPLES - 1;
/// A latency no wait may exceed and still count as ending with the close.
const CLOSE_BOUND: Duration = Duration::from_millis(1500);
/// How long the transport is given to stop.
const WAIT: Duration = Duration::from_secs(15);

/// A hand close under the wait returns within a few milliseconds of the close, not within
/// the 250 ms poll slice: over `SAMPLES` fresh links, at least `WAKE_SAMPLES_REQUIRED`
/// close-to-return latencies land under `WAKE_BOUND`, each with `ConnectionAborted` naming
/// `InitiatorClosed`.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn a_close_under_the_wait_wakes_the_reread_without_waiting_out_the_poll_slice() {
    let transport = broadcasting("link-activation-closed-wake");
    let mut latencies = Vec::with_capacity(SAMPLES);

    for sample in 0..SAMPLES {
        let link =
            transport.link(unknown_destination()).await.expect("pending link with broadcast on");
        link.lock().await.set_establishment_timeout(ESTABLISHMENT_FAR);

        let closer = Arc::clone(&link);
        let closer_task = tokio::spawn(async move {
            tokio::time::sleep(CLOSE_AT).await;
            closer.lock().await.close();
            Instant::now()
        });

        let error = await_link_activation(&transport, &link, LONG_WAIT)
            .await
            .expect_err("a pending link closed by hand never activates");
        let returned = Instant::now();
        assert_closed_before_activation(&error, "InitiatorClosed");
        let closed_at = closer_task.await.expect("closer task");
        let latency = returned.saturating_duration_since(closed_at);
        assert!(
            latency < CLOSE_BOUND,
            "sample {sample}: the wait ends with the close, not at its deadline: {latency:?}"
        );
        latencies.push(latency);
    }

    let woken = latencies.iter().filter(|latency| **latency < WAKE_BOUND).count();
    assert!(
        woken >= WAKE_SAMPLES_REQUIRED,
        "the Closed event wakes the reread instead of waiting out the poll slice: \
         {woken} of {SAMPLES} samples under {WAKE_BOUND:?}, latencies {latencies:?}"
    );

    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
}

/// A link closed and restarted inside one lock scope, before the waiter runs again, reads
/// `Pending` with no close reason when the `Closed` event wakes the reread, so the wait does
/// not end there: it reaches its own deadline with `TimedOut`, and the link is still
/// `Pending`. This is the one shape where ending the wait on the event itself would have
/// answered a stale `ConnectionAborted` naming `InitiatorClosed` for a link that is pending
/// again. The runtime is `current_thread`, so the closer's lock scope runs to its end before
/// the waiter is polled.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn a_close_and_restart_before_the_reread_leaves_the_wait_running_to_its_deadline() {
    let transport = broadcasting("link-activation-closed-then-restarted");
    let link = transport.link(unknown_destination()).await.expect("pending link with broadcast on");
    link.lock().await.set_establishment_timeout(ESTABLISHMENT_FAR);

    let started = Instant::now();
    let closer = Arc::clone(&link);
    let closer_task = tokio::spawn(async move {
        tokio::time::sleep(CLOSE_AT).await;
        let mut guard = closer.lock().await;
        guard.close();
        assert_eq!(guard.status(), LinkStatus::Closed, "the close lands before the restart");
        guard.restart();
        guard.set_establishment_timeout(ESTABLISHMENT_FAR);
        assert_eq!(guard.status(), LinkStatus::Pending, "restart puts the link back to pending");
        assert_eq!(guard.close_reason(), None, "restart clears the close reason");
    });

    let error = await_link_activation(&transport, &link, SHORT_WAIT)
        .await
        .expect_err("a pending link with no interface never activates");
    let elapsed = started.elapsed();
    closer_task.await.expect("closer task");
    assert_eq!(
        error.kind(),
        io::ErrorKind::TimedOut,
        "a link that is pending again at the reread does not end the wait: {error}"
    );
    assert_eq!(error.to_string(), "link activation timed out");
    assert!(elapsed >= SHORT_WAIT, "the wait reaches its own deadline, waited {elapsed:?}");
    assert!(
        elapsed < SHORT_WAIT + DEADLINE_SLACK,
        "the wait returns close to its deadline, waited {elapsed:?}"
    );
    assert_eq!(link.lock().await.status(), LinkStatus::Pending);
    assert_eq!(link.lock().await.close_reason(), None);

    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
}
