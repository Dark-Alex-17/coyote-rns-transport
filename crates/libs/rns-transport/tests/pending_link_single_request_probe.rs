//! A pending outbound link's request decodes as one `LinkRequest` frame on the stream a
//! raw listener accepted from the transport's `TcpClient`, and the establishment
//! timeout closes the link with `LinkCloseReason::Timeout`, measured from outside the
//! crate. Python sends the request once (`RNS/Link.py:313-320`) and the establishment
//! timeout is the only bound on a link that never comes up (`RNS/Link.py:722-729`). The
//! listener never answers, so the link stays pending until its shortened timeout closes
//! it; the path request a non-transport initiator broadcasts after that close is read
//! from the same stream and is not a `LinkRequest`. The 2 s timeout used here closes the
//! link before the 6 s mark at which the removed maintenance repeat used to fire; that
//! mark is covered by `pending_link_single_request_default_timeout_probe.rs`.

use std::time::Duration;

use rns_transport::destination::link::{LinkCloseReason, LinkStatus};
use tokio::time::timeout;

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
#[path = "support/unknown_destination.rs"]
mod unknown_destination;
use closed_event_for::closed_event_for;
use initiator_on_a_silent_listener::initiator_on_a_silent_listener;
use link_requests_to::link_requests_to;
use packets_in::packets_in;
use read_for::read_for;
use unknown_destination::unknown_destination;

/// What the pending link is given before the transport closes it.
const ESTABLISHMENT_SECS: u64 = 2;
const ESTABLISHMENT: Duration = Duration::from_secs(ESTABLISHMENT_SECS);
/// How long the stream is read: twice the timeout, so the close falls inside the window.
const WINDOW: Duration = Duration::from_secs(2 * ESTABLISHMENT_SECS);
const WAIT: Duration = Duration::from_secs(5);

/// Over twice its establishment timeout, a pending link's stream carries one
/// `LinkRequest` frame, the link is closed with `Timeout`, and the path request that
/// follows the close is not a `LinkRequest`.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn a_pending_link_sends_one_request_and_is_closed_at_its_timeout() {
    let (transport, mut stream) =
        initiator_on_a_silent_listener("pending-link-single-request").await;
    let mut events = transport.out_link_events();

    let destination = unknown_destination();
    let link = transport.link(destination).await.expect("pending link with broadcast on");
    link.lock().await.set_establishment_timeout(ESTABLISHMENT);
    let id = *link.lock().await.id();

    let raw = read_for(&mut stream, WINDOW).await;
    assert_eq!(
        link_requests_to(&packets_in(&raw), destination.address_hash),
        1,
        "one link request over {WINDOW:?} for a {ESTABLISHMENT:?} establishment timeout"
    );

    let closed = closed_event_for(&mut events, id, WAIT).await;
    assert_eq!(closed.close_reason, Some(LinkCloseReason::Timeout));
    assert_eq!(link.lock().await.status(), LinkStatus::Closed);

    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
}
