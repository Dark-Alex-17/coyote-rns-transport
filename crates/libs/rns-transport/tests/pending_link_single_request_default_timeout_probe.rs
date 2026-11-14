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

use rns_transport::buffer::OutputBuffer;
use rns_transport::destination::link::{
    LinkCloseReason, LinkEvent, LinkEventData, LinkId, LinkStatus,
};
use rns_transport::destination::{DestinationDesc, DestinationName};
use rns_transport::hash::AddressHash;
use rns_transport::identity::PrivateIdentity;
use rns_transport::iface::hdlc::Hdlc;
use rns_transport::packet::{Packet, PacketType};
use rns_transport::rand_core::OsRng;
use rns_transport::transport::{Transport, TransportConfig};
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;
use tokio::time::timeout;

#[path = "support/accept_within.rs"]
mod accept_within;
#[path = "support/spawn_plain_client.rs"]
mod spawn_plain_client;
#[path = "support/wait_until.rs"]
mod wait_until;
use accept_within::accept_within;
use spawn_plain_client::spawn_plain_client;
use wait_until::wait_until;

/// The interval at which the removed maintenance repeat used to re-send a pending
/// request (`INTERVAL_OUTPUT_LINK_REPEAT`, 6 s, gone from `transport/mod.rs`).
const OLD_REPEAT_INTERVAL: Duration = Duration::from_secs(6);
/// The link maintenance job ticks once a second, so a repeat due at 6 s would have been
/// on the wire by 7 s; the window reaches past that with a second to spare.
const PAST_OLD_REPEAT: Duration = Duration::from_secs(8);
/// The short timeout each attempt is given once the window of interest has passed.
const SHORT_ESTABLISHMENT: Duration = Duration::from_secs(2);
const WAIT: Duration = Duration::from_secs(5);
const RECONNECT_WAIT: Duration = Duration::from_secs(60);

fn unknown_destination() -> DestinationDesc {
    let remote = *PrivateIdentity::new_from_rand(OsRng).as_identity();
    DestinationDesc {
        identity: remote,
        address_hash: remote.address_hash,
        name: DestinationName::new("lxmf", "delivery"),
    }
}

/// Everything `stream` carries during `window`. An end of stream is a failure: the
/// client has no reason to close a stream it just connected.
#[deny(clippy::await_holding_invalid_type)]
async fn read_for(stream: &mut (impl AsyncRead + Unpin), window: Duration) -> Vec<u8> {
    let mut raw = Vec::new();
    let mut buf = [0u8; 1024];
    let deadline = tokio::time::Instant::now() + window;
    loop {
        let left = deadline.saturating_duration_since(tokio::time::Instant::now());
        match timeout(left, stream.read(&mut buf)).await {
            Err(_) => return raw,
            Ok(Ok(0)) => panic!("the client closed the stream"),
            Ok(Ok(read)) => raw.extend_from_slice(&buf[..read]),
            Ok(Err(err)) => panic!("the read failed: {err}"),
        }
    }
}

/// How many `LinkRequest` packets addressed to `destination` the complete HDLC frames
/// of `raw` carry. Anything else on the stream (the path request a non-transport
/// initiator broadcasts after a timeout close) decodes but is not counted.
fn link_requests_to(raw: &[u8], destination: AddressHash) -> usize {
    let mut count = 0;
    let mut cursor = 0;
    while let Some((start, end)) = Hdlc::find(&raw[cursor..]) {
        let frame = &raw[cursor + start..=cursor + end];
        let mut decoded = [0u8; 1024];
        let mut output = OutputBuffer::new(&mut decoded);
        let len = Hdlc::decode(frame, &mut output)
            .unwrap_or_else(|err| panic!("a frame on the stream does not decode: {err:?}"));
        if len > 0 {
            let packet = Packet::from_bytes(&decoded[..len])
                .unwrap_or_else(|err| panic!("a frame on the stream is not a packet: {err:?}"));
            if packet.header.packet_type == PacketType::LinkRequest
                && packet.destination == destination
            {
                count += 1;
            }
        }
        cursor += end + 1;
    }
    count
}

/// The `Closed` event for `id` within `within`, skipping anything else first.
#[deny(clippy::await_holding_invalid_type)]
async fn closed_event_for(
    events: &mut broadcast::Receiver<LinkEventData>,
    id: LinkId,
    within: Duration,
) -> LinkEventData {
    let deadline = tokio::time::Instant::now() + within;
    loop {
        let left = deadline.saturating_duration_since(tokio::time::Instant::now());
        let event = timeout(left, events.recv())
            .await
            .expect("the timeout close publishes an out-link event")
            .expect("the out-link event stream remains open");
        if event.id == id && matches!(event.event, LinkEvent::Closed) {
            return event;
        }
    }
}

/// A non-transport initiator with broadcast on, its plain `TcpClient` connected to a raw
/// loopback listener that never answers; returns the transport and the accepted stream.
#[deny(clippy::await_holding_invalid_type)]
async fn initiator_on_a_silent_listener(name: &str) -> (Transport, TcpStream) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("a loopback listener binds");
    let addr = listener.local_addr().expect("the listener has an address");
    let transport =
        Transport::new(TransportConfig::new(name, &PrivateIdentity::new_from_rand(OsRng), true));
    let (_iface, status) = spawn_plain_client(&transport, addr, RECONNECT_WAIT).await;
    let stream = accept_within(&listener, "the client's connect").await;
    wait_until("the client to count its stream", || status.connected_streams() >= 1).await;
    (transport, stream)
}

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
        link_requests_to(&raw, destination.address_hash),
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
        link_requests_to(&tail, destination.address_hash),
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
        link_requests_to(&raw, destination.address_hash),
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
        link_requests_to(&raw, destination.address_hash),
        1,
        "the second attempt sends its own single request"
    );
    let closed = closed_event_for(&mut events, second_id, WAIT).await;
    assert_eq!(closed.close_reason, Some(LinkCloseReason::Timeout));
    assert_eq!(second.lock().await.status(), LinkStatus::Closed);

    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
}
