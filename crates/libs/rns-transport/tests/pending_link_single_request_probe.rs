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

use rns_transport::buffer::OutputBuffer;
use rns_transport::destination::link::{
    LinkCloseReason, LinkEvent, LinkEventData, LinkId, LinkStatus,
};
use rns_transport::destination::{DestinationDesc, DestinationName};
use rns_transport::identity::PrivateIdentity;
use rns_transport::iface::hdlc::Hdlc;
use rns_transport::packet::{Packet, PacketType};
use rns_transport::rand_core::OsRng;
use rns_transport::transport::{Transport, TransportConfig};
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::net::TcpListener;
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

/// What the pending link is given before the transport closes it.
const ESTABLISHMENT_SECS: u64 = 2;
const ESTABLISHMENT: Duration = Duration::from_secs(ESTABLISHMENT_SECS);
/// How long the stream is read: twice the timeout, so the close falls inside the window.
const WINDOW: Duration = Duration::from_secs(2 * ESTABLISHMENT_SECS);
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

/// Everything `stream` carries during `WINDOW`. An end of stream is a failure: the
/// client has no reason to close a stream it just connected.
#[deny(clippy::await_holding_invalid_type)]
async fn read_for_window(stream: &mut (impl AsyncRead + Unpin)) -> Vec<u8> {
    let mut raw = Vec::new();
    let mut buf = [0u8; 1024];
    let deadline = tokio::time::Instant::now() + WINDOW;
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

/// The packets in every complete HDLC frame of `raw`, in order.
fn packets_in(raw: &[u8]) -> Vec<Packet> {
    let mut packets = Vec::new();
    let mut cursor = 0;
    while let Some((start, end)) = Hdlc::find(&raw[cursor..]) {
        let frame = &raw[cursor + start..=cursor + end];
        let mut decoded = [0u8; 1024];
        let mut output = OutputBuffer::new(&mut decoded);
        let len = Hdlc::decode(frame, &mut output)
            .unwrap_or_else(|err| panic!("a frame on the stream does not decode: {err:?}"));
        if len > 0 {
            packets
                .push(Packet::from_bytes(&decoded[..len]).unwrap_or_else(|err| {
                    panic!("a frame on the stream is not a packet: {err:?}")
                }));
        }
        cursor += end + 1;
    }
    packets
}

/// The `Closed` event for `id`, skipping anything else the stream carries first.
#[deny(clippy::await_holding_invalid_type)]
async fn closed_event_for(
    events: &mut broadcast::Receiver<LinkEventData>,
    id: LinkId,
) -> LinkEventData {
    loop {
        let event = timeout(WAIT, events.recv())
            .await
            .expect("the timeout close publishes an out-link event")
            .expect("the out-link event stream remains open");
        if event.id == id && matches!(event.event, LinkEvent::Closed) {
            return event;
        }
    }
}

/// Over twice its establishment timeout, a pending link's stream carries one
/// `LinkRequest` frame, the link is closed with `Timeout`, and the path request that
/// follows the close is not a `LinkRequest`.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn a_pending_link_sends_one_request_and_is_closed_at_its_timeout() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("a loopback listener binds");
    let addr = listener.local_addr().expect("the listener has an address");
    let transport = Transport::new(TransportConfig::new(
        "pending-link-single-request",
        &PrivateIdentity::new_from_rand(OsRng),
        true,
    ));
    let (_iface, status) = spawn_plain_client(&transport, addr, RECONNECT_WAIT).await;
    let mut stream = accept_within(&listener, "the client's connect").await;
    wait_until("the client to count its stream", || status.connected_streams() >= 1).await;
    let mut events = transport.out_link_events();

    let link = transport.link(unknown_destination()).await.expect("pending link with broadcast on");
    link.lock().await.set_establishment_timeout(ESTABLISHMENT);
    let id = *link.lock().await.id();

    let raw = read_for_window(&mut stream).await;
    let link_requests = packets_in(&raw)
        .iter()
        .filter(|packet| packet.header.packet_type == PacketType::LinkRequest)
        .count();
    assert_eq!(
        link_requests, 1,
        "one link request over {WINDOW:?} for a {ESTABLISHMENT:?} establishment timeout"
    );

    let closed = closed_event_for(&mut events, id).await;
    assert_eq!(closed.close_reason, Some(LinkCloseReason::Timeout));
    assert_eq!(link.lock().await.status(), LinkStatus::Closed);

    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
}
