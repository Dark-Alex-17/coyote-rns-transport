//! Two edges of the one-request rule, measured from outside the crate on the stream a raw
//! listener accepted from the transport's `TcpClient`; the listener never answers.
//!
//! First, the rule binds the transport, not the consumer: a direct second `Link::request()`
//! on a still-pending link is still allowed and, handed to `Transport::send_packet`, is a
//! second `LinkRequest` frame on the wire, and it does not restart the establishment clock
//! (`establishment_timed_out` is measured from the start of the attempt, as
//! `RNS/Link.py:722-729` measures from `request_time`), so the `Closed` event with
//! `LinkCloseReason::Timeout` arrives at the original deadline, not a repeat-shifted one.
//!
//! Second, what follows the timeout close on a non-transport initiator is the rediscovery
//! path request for the destination, once, and not another `LinkRequest`: the stream
//! carries exactly one `LinkRequest` and exactly one path request naming the destination
//! over a window that reaches well past the close.

use std::time::{Duration, Instant};

use rns_transport::destination::link::{LinkCloseReason, LinkStatus};
use rns_transport::destination::{DestinationName, PlainInputDestination};
use rns_transport::hash::{AddressHash, ADDRESS_HASH_SIZE};
use rns_transport::identity::EmptyIdentity;
use rns_transport::packet::{Packet, PacketType};
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

/// The establishment timeout each attempt is given.
const ESTABLISHMENT: Duration = Duration::from_secs(3);
/// When the consumer repeats the request by hand: inside the timeout, past one tick.
const REPEAT_AT: Duration = Duration::from_secs(2);
/// The link maintenance job ticks once a second, so a close due at 3 s is published by
/// 4 s; a clock restarted at the 2 s repeat would publish no earlier than 5 s.
const CLOSE_BY: Duration = Duration::from_millis(4500);
const WAIT: Duration = Duration::from_secs(6);

/// The address every Reticulum path request is sent to (`rnstransport.path.request`).
fn path_request_destination() -> AddressHash {
    PlainInputDestination::new(
        EmptyIdentity {},
        DestinationName::new("rnstransport", "path.request"),
    )
    .desc
    .address_hash
}

/// Path requests naming `destination`: sent to the path-request address, with the asked-for
/// destination hash leading the payload.
fn path_requests_for(packets: &[Packet], destination: AddressHash) -> usize {
    let path_request = path_request_destination();
    packets
        .iter()
        .filter(|p| {
            p.header.packet_type != PacketType::LinkRequest
                && p.destination == path_request
                && p.data.as_slice().len() >= ADDRESS_HASH_SIZE
                && &p.data.as_slice()[..ADDRESS_HASH_SIZE] == destination.as_slice()
        })
        .count()
}

/// A consumer that repeats `Link::request()` by hand on a still-pending link puts a second
/// `LinkRequest` on the wire (the rule binds the maintenance job, not the caller), and the
/// `Closed` event with `Timeout` still arrives at the original deadline: the repeat does not
/// restart the establishment clock.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn a_consumer_repeated_request_goes_out_and_does_not_restart_the_clock() {
    let (transport, mut stream) =
        initiator_on_a_silent_listener("pending-link-consumer-repeat").await;
    let mut events = transport.out_link_events();

    let destination = unknown_destination();
    let opened_at = Instant::now();
    let link = transport.link(destination).await.expect("pending link with broadcast on");
    link.lock().await.set_establishment_timeout(ESTABLISHMENT);
    let id = *link.lock().await.id();

    // Up to the hand-made repeat: exactly the transport's own single request.
    let before = packets_in(&read_for(&mut stream, REPEAT_AT).await);
    assert_eq!(
        link_requests_to(&before, destination.address_hash),
        1,
        "one request before the repeat"
    );
    assert_eq!(link.lock().await.status(), LinkStatus::Pending, "still pending at the repeat");

    let repeat = link.lock().await.request();
    assert_eq!(repeat.header.packet_type, PacketType::LinkRequest);
    assert_eq!(repeat.destination, destination.address_hash);
    transport.send_packet(repeat).await;

    let closed = closed_event_for(&mut events, id, WAIT).await;
    let closed_after = opened_at.elapsed();
    assert_eq!(closed.close_reason, Some(LinkCloseReason::Timeout));
    assert_eq!(link.lock().await.status(), LinkStatus::Closed);
    assert!(
        closed_after >= ESTABLISHMENT,
        "the close is not early: {closed_after:?} < {ESTABLISHMENT:?}"
    );
    assert!(
        closed_after < CLOSE_BY,
        "the hand-made repeat at {REPEAT_AT:?} restarted the establishment clock: closed after {closed_after:?}, expected before {CLOSE_BY:?}"
    );

    // The repeat reached the wire; nothing else addressed to the link follows the close.
    let after = packets_in(&read_for(&mut stream, Duration::from_secs(2)).await);
    assert_eq!(
        link_requests_to(&after, destination.address_hash),
        1,
        "the consumer's own repeat is the only further link request on the stream"
    );

    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
}

/// After the timeout close, a non-transport initiator asks for the path once: the stream
/// carries one `LinkRequest` and one path request naming the destination, nothing else for
/// it, over a window that reaches well past the close.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn the_timeout_close_is_followed_by_one_path_request_and_no_link_request() {
    let (transport, mut stream) =
        initiator_on_a_silent_listener("pending-link-rediscovery-wire").await;
    let mut events = transport.out_link_events();

    let destination = unknown_destination();
    let link = transport.link(destination).await.expect("pending link with broadcast on");
    link.lock().await.set_establishment_timeout(Duration::from_secs(2));
    let id = *link.lock().await.id();

    let closed = closed_event_for(&mut events, id, WAIT).await;
    assert_eq!(closed.close_reason, Some(LinkCloseReason::Timeout));

    // Everything from `link()` through the close and three seconds past it.
    let packets = packets_in(&read_for(&mut stream, Duration::from_secs(3)).await);
    assert_eq!(
        link_requests_to(&packets, destination.address_hash),
        1,
        "exactly one link request across the close"
    );
    assert_eq!(
        path_requests_for(&packets, destination.address_hash),
        1,
        "exactly one rediscovery path request for the destination follows the close"
    );
    let other: Vec<_> = packets
        .iter()
        .filter(|p| {
            let is_the_link_request = p.header.packet_type == PacketType::LinkRequest
                && p.destination == destination.address_hash;
            let is_a_path_request = p.destination == path_request_destination();
            !is_the_link_request && !is_a_path_request
        })
        .map(|p| (p.header.packet_type, p.destination))
        .collect();
    assert!(other.is_empty(), "nothing else is on the stream: {other:?}");

    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
}
