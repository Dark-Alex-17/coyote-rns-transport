//! The one-request rule under two consumer patterns the shipped probes do not cover,
//! measured from outside the crate on the stream a raw listener accepted from the
//! transport's `TcpClient`. Python sends a link request once (`RNS/Link.py:313-320`) and
//! the establishment timeout is the only bound on a link that never comes up
//! (`RNS/Link.py:722-729`); the listener never answers.
//!
//! First, two links opened back to back to two different destinations are two attempts
//! sharing one maintenance tick: each puts exactly one `LinkRequest` on the stream,
//! addressed to its own destination, and each is closed with `LinkCloseReason::Timeout`
//! on its own. Second, a timeout shorter than the 1 s maintenance tick still yields
//! exactly one request before the close, so a consumer that gives up fast does not pay
//! for a retransmit it never asked for.

use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use rns_transport::destination::link::{
    LinkCloseReason, LinkEvent, LinkEventData, LinkId, LinkStatus,
};
use rns_transport::hash::AddressHash;
use rns_transport::packet::{Packet, PacketType};
use tokio::sync::broadcast::{self, error::RecvError};
use tokio::time::timeout;

#[path = "support/accept_within.rs"]
mod accept_within;
#[path = "support/broadcasting.rs"]
mod broadcasting;
#[path = "support/closed_event_for.rs"]
mod closed_event_for;
#[path = "support/initiator_on_a_silent_listener.rs"]
mod initiator_on_a_silent_listener;
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
use packets_in::packets_in;
use read_for::read_for;
use unknown_destination::unknown_destination;

const ESTABLISHMENT: Duration = Duration::from_secs(2);
/// Shorter than the 1 s link maintenance tick.
const SUB_TICK_ESTABLISHMENT: Duration = Duration::from_millis(300);
const WAIT: Duration = Duration::from_secs(5);

/// The destination of every `LinkRequest` packet in `packets`, in order.
fn link_request_destinations_in(packets: &[Packet]) -> Vec<AddressHash> {
    packets
        .iter()
        .filter(|p| p.header.packet_type == PacketType::LinkRequest)
        .map(|p| p.destination)
        .collect()
}

/// The `Closed` event for every id in `ids` within `WAIT`, in whatever order they arrive,
/// keyed by link id; anything else on the event stream is skipped.
#[deny(clippy::await_holding_invalid_type)]
async fn closed_events_for<const N: usize>(
    events: &mut broadcast::Receiver<LinkEventData>,
    ids: [LinkId; N],
) -> BTreeMap<LinkId, LinkEventData> {
    let mut closed = BTreeMap::new();
    let deadline = tokio::time::Instant::now() + WAIT;
    while closed.len() < N {
        let left = deadline.saturating_duration_since(tokio::time::Instant::now());
        let event = match timeout(left, events.recv())
            .await
            .expect("every timeout close publishes an out-link event")
        {
            Ok(event) => event,
            Err(RecvError::Lagged(n)) => {
                panic!("the out-link event receiver lagged by {n} event(s)")
            }
            Err(RecvError::Closed) => panic!("the out-link event stream remains open"),
        };
        if ids.contains(&event.id) && matches!(event.event, LinkEvent::Closed) {
            closed.insert(event.id, event);
        }
    }
    closed
}

/// Two pending links to two destinations, opened back to back, each put exactly one
/// `LinkRequest` addressed to their own destination on the stream over twice their
/// establishment timeout, and each is closed with `Timeout`.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_concurrent_pending_links_send_one_request_each_and_both_time_out() {
    let (transport, mut stream) = initiator_on_a_silent_listener("pending-links-concurrent").await;
    let mut events = transport.out_link_events();
    let (first_destination, second_destination) = (unknown_destination(), unknown_destination());

    let first = transport.link(first_destination).await.expect("first pending link");
    first.lock().await.set_establishment_timeout(ESTABLISHMENT);
    let first_id = *first.lock().await.id();
    let second = transport.link(second_destination).await.expect("second pending link");
    second.lock().await.set_establishment_timeout(ESTABLISHMENT);
    let second_id = *second.lock().await.id();
    assert_ne!(first_id, second_id, "two attempts are two links");

    let requests =
        link_request_destinations_in(&packets_in(&read_for(&mut stream, 2 * ESTABLISHMENT).await));
    assert_eq!(requests.len(), 2, "one link request per pending link, no more: {requests:?}");
    let requested: BTreeSet<AddressHash> = requests.iter().copied().collect();
    let expected: BTreeSet<AddressHash> =
        [first_destination.address_hash, second_destination.address_hash].into_iter().collect();
    assert_eq!(requested, expected, "each request is addressed to its own destination");

    // Both close in the same maintenance tick, in whichever order the job visits them.
    let mut closed = closed_events_for(&mut events, [first_id, second_id]).await;
    let first_closed = closed.remove(&first_id).expect("the first link's Closed event");
    assert_eq!(first_closed.close_reason, Some(LinkCloseReason::Timeout));
    let second_closed = closed.remove(&second_id).expect("the second link's Closed event");
    assert_eq!(second_closed.close_reason, Some(LinkCloseReason::Timeout));
    assert_eq!(first.lock().await.status(), LinkStatus::Closed);
    assert_eq!(second.lock().await.status(), LinkStatus::Closed);

    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
}

/// An establishment timeout shorter than the maintenance tick still means exactly one
/// `LinkRequest` before the `Closed` event with `Timeout`, and none after it.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn a_sub_tick_establishment_timeout_still_sends_exactly_one_request() {
    let (transport, mut stream) = initiator_on_a_silent_listener("pending-link-sub-tick").await;
    let mut events = transport.out_link_events();

    let destination = unknown_destination();
    let link = transport.link(destination).await.expect("pending link");
    link.lock().await.set_establishment_timeout(SUB_TICK_ESTABLISHMENT);
    let id = *link.lock().await.id();

    let closed = closed_event_for(&mut events, id, WAIT).await;
    assert_eq!(closed.close_reason, Some(LinkCloseReason::Timeout));
    assert_eq!(link.lock().await.status(), LinkStatus::Closed);

    // Everything the stream buffered before the close plus two maintenance ticks after it.
    let requests =
        link_request_destinations_in(&packets_in(&read_for(&mut stream, ESTABLISHMENT).await));
    assert_eq!(
        requests,
        vec![destination.address_hash],
        "exactly one link request, to the link's destination, around a sub-tick timeout"
    );

    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
}
