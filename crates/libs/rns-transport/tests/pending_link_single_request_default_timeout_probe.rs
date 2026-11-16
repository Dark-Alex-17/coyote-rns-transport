//! The one-request rule from where a consumer stands: `Transport::link`
//! with nothing overridden. The shipped `pending_link_single_request_probe.rs` shortens
//! the establishment timeout to 2 s, which closes the link before the 6 s mark at which
//! the removed maintenance repeat used to fire, so it cannot tell the two behaviours
//! apart. This probe leaves the timeout the transport sized alone (first hop plus
//! `ESTABLISHMENT_TIMEOUT_PER_HOP` per hop, and an unknown destination counts
//! `PATHFINDER_M` hops, as `RNS.Transport.hops_to` does, so minutes) and reads the raw
//! listener's stream across that 6 s mark: Python sends the request once
//! (`RNS/Link.py:313-320`) and the establishment timeout is the only bound on a link
//! that never comes up (`RNS/Link.py:722-729`), so exactly one `LinkRequest` frame
//! arrives and the link is still `Pending` when the window ends. The clock that was
//! running all along is then shortened under the link, and the `Closed` event with
//! `LinkCloseReason::Timeout` follows with nothing more addressed to the link.
//!
//! The second test pins what the rule does not mean: it is one request per attempt, not
//! per destination. A fresh `Transport::link` after the timeout close is a new attempt
//! and puts a new `LinkRequest` on the same stream.

use std::time::Duration;

use rns_transport::destination::link::{LinkCloseReason, LinkStatus};
use tokio::time::timeout;

#[path = "support/accept_within.rs"]
mod accept_within;
#[path = "support/broadcasting.rs"]
mod broadcasting;
#[path = "support/closed_event_for.rs"]
mod closed_event_for;
#[path = "support/initiator_on_a_silent_listener.rs"]
mod initiator_on_a_silent_listener;
#[path = "support/link_requests_to.rs"]
mod link_requests_to;
#[path = "support/packets_in.rs"]
mod packets_in;
#[path = "support/read_for.rs"]
mod read_for;
#[path = "support/spawn_plain_client.rs"]
mod spawn_plain_client;
#[path = "support/unknown_destination.rs"]
mod unknown_destination;
#[path = "support/wait_until.rs"]
mod wait_until;
use closed_event_for::closed_event_for;
use initiator_on_a_silent_listener::initiator_on_a_silent_listener;
use link_requests_to::link_requests_to;
use packets_in::packets_in;
use read_for::read_for;
use unknown_destination::unknown_destination;

/// The interval at which the removed maintenance repeat used to re-send a pending
/// request (`INTERVAL_OUTPUT_LINK_REPEAT`, 6 s, gone from `transport/mod.rs`).
const OLD_REPEAT_INTERVAL: Duration = Duration::from_secs(6);
/// The link maintenance job ticks once a second, so a repeat due at 6 s would have been
/// on the wire by 7 s; the window reaches past that with a second to spare.
const PAST_OLD_REPEAT: Duration = Duration::from_secs(8);
/// The short timeout each attempt is given once the window of interest has passed.
const SHORT_ESTABLISHMENT: Duration = Duration::from_secs(2);
const WAIT: Duration = Duration::from_secs(5);

/// With nothing overridden, a pending link's stream carries one `LinkRequest` across the
/// 6 s mark at which the removed repeat used to fire and the link is still `Pending` when
/// that window ends; the `Closed` event with `Timeout` follows once the clock that was
/// running all along is shortened under it, and no link request follows the close.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn a_default_pending_link_sends_one_request_past_the_old_repeat_interval() {
    assert!(PAST_OLD_REPEAT > OLD_REPEAT_INTERVAL + Duration::from_secs(1));
    let (transport, mut stream) =
        initiator_on_a_silent_listener("pending-link-default-timeout").await;
    let mut events = transport.out_link_events();

    let destination = unknown_destination();
    let link = transport.link(destination).await.expect("pending link with broadcast on");
    let id = *link.lock().await.id();

    let raw = read_for(&mut stream, PAST_OLD_REPEAT).await;
    assert_eq!(
        link_requests_to(&packets_in(&raw), destination.address_hash),
        1,
        "one link request over {PAST_OLD_REPEAT:?} with the transport-sized establishment timeout"
    );
    assert_eq!(
        link.lock().await.status(),
        LinkStatus::Pending,
        "the link is still pending inside its transport-sized establishment timeout"
    );

    // The clock started at `link()`, so a 2 s bound is already past: the next tick closes.
    link.lock().await.set_establishment_timeout(SHORT_ESTABLISHMENT);
    let closed = closed_event_for(&mut events, id, WAIT).await;
    assert_eq!(closed.close_reason, Some(LinkCloseReason::Timeout));
    assert_eq!(link.lock().await.status(), LinkStatus::Closed);

    // No link request follows the close on this stream.
    let tail = read_for(&mut stream, SHORT_ESTABLISHMENT).await;
    assert_eq!(
        link_requests_to(&packets_in(&tail), destination.address_hash),
        0,
        "no link request follows the timeout close"
    );

    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
}

/// One request per attempt, not per destination: after a link to a destination has timed
/// out, a fresh `Transport::link` to the same destination is a new link with a new id and
/// puts a new `LinkRequest` on the same stream, which is then also sent once.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn a_fresh_link_after_a_timeout_close_sends_its_own_single_request() {
    let (transport, mut stream) =
        initiator_on_a_silent_listener("pending-link-fresh-attempt").await;
    let mut events = transport.out_link_events();
    let destination = unknown_destination();

    let first = transport.link(destination).await.expect("first pending link");
    first.lock().await.set_establishment_timeout(SHORT_ESTABLISHMENT);
    let first_id = *first.lock().await.id();
    let raw = read_for(&mut stream, 2 * SHORT_ESTABLISHMENT).await;
    assert_eq!(
        link_requests_to(&packets_in(&raw), destination.address_hash),
        1,
        "the first attempt sends one link request"
    );
    let closed = closed_event_for(&mut events, first_id, WAIT).await;
    assert_eq!(closed.close_reason, Some(LinkCloseReason::Timeout));

    let second = transport.link(destination).await.expect("second pending link");
    second.lock().await.set_establishment_timeout(SHORT_ESTABLISHMENT);
    let second_id = *second.lock().await.id();
    assert_ne!(second_id, first_id, "a fresh attempt is a new link");
    let raw = read_for(&mut stream, 2 * SHORT_ESTABLISHMENT).await;
    assert_eq!(
        link_requests_to(&packets_in(&raw), destination.address_hash),
        1,
        "the second attempt sends its own single request"
    );
    let closed = closed_event_for(&mut events, second_id, WAIT).await;
    assert_eq!(closed.close_reason, Some(LinkCloseReason::Timeout));
    assert_eq!(second.lock().await.status(), LinkStatus::Closed);

    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
}
