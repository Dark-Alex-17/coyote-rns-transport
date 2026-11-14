//! What the opt-in tunnel of `SharedInstanceOptions::with_tunnel_synthesis` writes, read as
//! a consumer would read it off the wire: the frame on an opted-in stream decodes to the
//! Data packet addressed to the `rnstransport.tunnel.synthesize` plain destination, led by
//! the transport's public and verifying keys and the full hash of the attached interface
//! (what Python's `synthesize_tunnel` signs, `RNS/Transport.py:2764-2786`; the receiving
//! shared instance derives the tunnel id as the hash of the public key and that interface
//! hash, `:2794-2795`), and the frame on the stream a forced drop reopens carries the same
//! keys and interface hash, so the shared instance can file the reconnect under the tunnel
//! it already holds (the reappeared branch of `handle_tunnel`, `:2829`). The option is read
//! per attach, so a transport that detaches and attaches again follows the options of the
//! new attach rather than the old one; a `Unix` endpoint honours it the same way a `Tcp`
//! one does; and the builder is `#[must_use]`, so a consumer that drops its return value is
//! told so.
//!
//! The `*_api.rs` sibling pins the byte counts and the builder's chaining; this file pins
//! what the bytes are and that the option does not leak across attaches or endpoint kinds.

use std::time::Duration;

use rns_transport::buffer::OutputBuffer;
use rns_transport::destination::{DestinationName, PlainInputDestination};
use rns_transport::hash::{AddressHash, Hash, ADDRESS_HASH_SIZE, HASH_SIZE};
use rns_transport::identity::{EmptyIdentity, PrivateIdentity, PUBLIC_KEY_LENGTH};
use rns_transport::iface::hdlc::Hdlc;
#[cfg(unix)]
use rns_transport::iface::local::LocalUnixEndpoint;
use rns_transport::packet::{Packet, PacketType};
use rns_transport::rand_core::OsRng;
use rns_transport::transport::{
    SharedInstanceEndpoint, SharedInstanceEvent, SharedInstanceOptions, Transport, TransportConfig,
};
use tokio::io::{AsyncRead, AsyncReadExt as _};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::time::timeout;

#[path = "support/accept_within.rs"]
mod accept_within;
use accept_within::accept_within;
#[path = "support/compile_fail.rs"]
mod compile_fail;
use compile_fail::{candidate_rlibs, deps_dir, rustc, Scratch};

const WAIT: Duration = Duration::from_secs(5);
const RECONNECT_WAIT: Duration = Duration::from_millis(50);
/// How long a fresh stream is watched for a frame the client should not write; an
/// opted-in client's tunnel frame lands within milliseconds on loopback.
const QUIET: Duration = Duration::from_secs(1);

/// The signed body of a tunnel packet: public key, verifying key, interface hash, random
/// hash; the signature follows (the slices `tunnel_synthesize_handler` takes at
/// `RNS/Transport.py:2790-2798`).
const TUNNEL_BODY: usize = PUBLIC_KEY_LENGTH * 2 + HASH_SIZE + ADDRESS_HASH_SIZE;

#[deny(clippy::await_holding_invalid_type)]
async fn next_event(
    what: &str,
    events: &mut mpsc::Receiver<SharedInstanceEvent>,
) -> Option<SharedInstanceEvent> {
    timeout(WAIT, events.recv()).await.unwrap_or_else(|_| panic!("{what} timed out"))
}

/// A non-broadcasting transport under `identity`.
fn transport_under(name: &str, identity: &PrivateIdentity) -> Transport {
    Transport::new(TransportConfig::new(name, identity, false))
}

/// Attaches `transport` at `endpoint` with `options` plus a state stream and the short
/// reconnect wait, reads the attach's `Connecting`, and returns the stream with the
/// interface address the attach reported.
#[deny(clippy::await_holding_invalid_type)]
async fn attach(
    transport: &Transport,
    endpoint: SharedInstanceEndpoint,
    options: SharedInstanceOptions,
) -> (mpsc::Receiver<SharedInstanceEvent>, AddressHash) {
    let (events_tx, mut events) = mpsc::channel(16);
    let iface = transport
        .attach_shared_instance(
            endpoint,
            options.with_reconnect_wait(RECONNECT_WAIT).with_state_events(events_tx),
        )
        .await
        .expect("the attach is accepted");
    assert_eq!(
        next_event("the attach's Connecting", &mut events).await,
        Some(SharedInstanceEvent::Connecting)
    );
    (events, iface)
}

/// The full interface hash the transport registered for `iface`, which is what the
/// tunnel packet carries.
#[deny(clippy::await_holding_invalid_type)]
async fn full_hash_of(transport: &Transport, iface: AddressHash) -> Hash {
    let manager = transport.iface_manager();
    let manager = manager.lock().await;
    manager.full_hash(&iface).expect("the attached interface is registered")
}

/// The first HDLC frame on `stream` within `QUIET` of the accept, decoded to its packet;
/// `None` when the client writes nothing in that window. An end of stream is neither: the
/// client never closes a stream it just connected, so it panics.
#[deny(clippy::await_holding_invalid_type)]
async fn first_frame_within_quiet(
    stream: &mut (impl AsyncRead + Unpin),
    what: &str,
) -> Option<Packet> {
    let mut raw = Vec::new();
    let mut buf = [0u8; 1024];
    let deadline = tokio::time::Instant::now() + QUIET;
    loop {
        match timeout(
            deadline.saturating_duration_since(tokio::time::Instant::now()),
            stream.read(&mut buf),
        )
        .await
        {
            Err(_) if raw.is_empty() => return None,
            Err(_) => panic!("{what}: an incomplete frame of {} bytes", raw.len()),
            Ok(Ok(0)) => panic!("{what}: the client closed the stream"),
            Ok(Ok(read)) => raw.extend_from_slice(&buf[..read]),
            Ok(Err(err)) => panic!("{what}: the read failed: {err}"),
        }
        if Hdlc::find(&raw).is_some() {
            let mut decoded = [0u8; 1024];
            let mut output = OutputBuffer::new(&mut decoded);
            let len = Hdlc::decode(&raw, &mut output)
                .unwrap_or_else(|err| panic!("{what}: the frame does not decode: {err:?}"));
            return Some(
                Packet::from_bytes(&decoded[..len])
                    .unwrap_or_else(|err| panic!("{what}: not a packet: {err:?}")),
            );
        }
    }
}

/// The plain destination the tunnel packet is addressed to.
fn tunnel_synthesize_destination() -> AddressHash {
    PlainInputDestination::new(
        EmptyIdentity {},
        DestinationName::new("rnstransport", "tunnel.synthesize"),
    )
    .desc
    .address_hash
}

/// Asserts `packet` is the tunnel packet of `identity` on the interface with `iface_hash`
/// and returns its random hash.
fn assert_tunnel_packet(
    packet: &Packet,
    identity: &PrivateIdentity,
    iface_hash: &Hash,
    what: &str,
) -> Vec<u8> {
    assert_eq!(
        packet.destination,
        tunnel_synthesize_destination(),
        "{what}: addressed to the tunnel.synthesize plain destination"
    );
    assert_eq!(packet.header.packet_type, PacketType::Data, "{what}: a Data packet");
    assert_eq!(packet.header.hops, 0, "{what}: synthesized at zero hops");
    let data = packet.data.as_slice();
    assert!(
        data.len() > TUNNEL_BODY,
        "{what}: the body carries the keys, the hashes and a signature, got {} bytes",
        data.len()
    );
    let public = identity.as_identity();
    assert_eq!(
        &data[..PUBLIC_KEY_LENGTH],
        public.public_key_bytes(),
        "{what}: led by the transport's public key"
    );
    assert_eq!(
        &data[PUBLIC_KEY_LENGTH..PUBLIC_KEY_LENGTH * 2],
        public.verifying_key_bytes(),
        "{what}: then its verifying key"
    );
    assert_eq!(
        &data[PUBLIC_KEY_LENGTH * 2..PUBLIC_KEY_LENGTH * 2 + HASH_SIZE],
        iface_hash.as_slice(),
        "{what}: then the full hash of the attached interface"
    );
    data[PUBLIC_KEY_LENGTH * 2 + HASH_SIZE..TUNNEL_BODY].to_vec()
}

/// An opted-in attach writes the tunnel packet of its own identity and interface, and the
/// stream a forced drop reopens carries one with the same keys and interface hash (a fresh
/// random hash aside), so the shared instance files the reconnect under the tunnel it holds.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn the_opt_in_frame_is_the_tunnel_packet_of_the_identity_and_the_interface() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind a loopback listener");
    let endpoint = SharedInstanceEndpoint::Tcp(listener.local_addr().expect("the address"));
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let transport = transport_under("tunnel-synthesis-frame", &identity);
    let (mut events, iface) =
        attach(&transport, endpoint, SharedInstanceOptions::default().with_tunnel_synthesis(true))
            .await;
    let iface_hash = full_hash_of(&transport, iface).await;

    let mut first = accept_within(&listener, "first connection").await;
    assert_eq!(
        next_event("first Connected", &mut events).await,
        Some(SharedInstanceEvent::Connected)
    );
    let packet = first_frame_within_quiet(&mut first, "first connection")
        .await
        .expect("an opted-in attach writes its tunnel packet on connect");
    let first_random = assert_tunnel_packet(&packet, &identity, &iface_hash, "at attach");

    drop(first);
    assert_eq!(next_event("the drop", &mut events).await, Some(SharedInstanceEvent::Disconnected));
    assert_eq!(next_event("the redial", &mut events).await, Some(SharedInstanceEvent::Connecting));
    let mut second = accept_within(&listener, "second connection").await;
    assert_eq!(
        next_event("second Connected", &mut events).await,
        Some(SharedInstanceEvent::Connected)
    );
    let packet = first_frame_within_quiet(&mut second, "second connection")
        .await
        .expect("an opted-in reconnect re-synthesizes the tunnel");
    let second_random = assert_tunnel_packet(&packet, &identity, &iface_hash, "on reconnect");
    assert_ne!(first_random, second_random, "each synthesis draws a fresh random hash");
    assert_eq!(
        transport.shared_instance(),
        Some(iface),
        "the reconnect keeps the interface, so the tunnel id the shared instance derives is unchanged"
    );

    assert!(transport.detach_shared_instance().await, "the attach is still held");
    drop(second);
    timeout(WAIT, transport.stop()).await.expect("client stop completes");
}

/// The option belongs to the attach, not the transport: an opted-in attach, detached,
/// followed by a default attach writes nothing on the new stream, and a third attach that
/// opts in again writes the tunnel packet again.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn a_reattach_follows_the_options_of_the_new_attach() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind a loopback listener");
    let addr = listener.local_addr().expect("the address");
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let transport = transport_under("tunnel-synthesis-reattach", &identity);

    for (nth, opted_in) in [(1, true), (2, false), (3, true)] {
        let what = format!("attach {nth} (opt-in {opted_in})");
        let (mut events, iface) = attach(
            &transport,
            SharedInstanceEndpoint::Tcp(addr),
            SharedInstanceOptions::default().with_tunnel_synthesis(opted_in),
        )
        .await;
        let iface_hash = full_hash_of(&transport, iface).await;
        let mut stream = accept_within(&listener, &what).await;
        assert_eq!(next_event(&what, &mut events).await, Some(SharedInstanceEvent::Connected));
        match first_frame_within_quiet(&mut stream, &what).await {
            Some(packet) => {
                assert!(opted_in, "{what}: a default attach writes nothing on connect");
                assert_tunnel_packet(&packet, &identity, &iface_hash, &what);
            }
            None => assert!(!opted_in, "{what}: an opted-in attach writes its tunnel packet"),
        }
        assert!(transport.detach_shared_instance().await, "{what}: the attach is held");
        drop(stream);
    }
    timeout(WAIT, transport.stop()).await.expect("client stop completes");
}

/// A `Unix` endpoint honours the option as a `Tcp` one does: nothing on connect under the
/// default, the tunnel packet under the opt-in.
#[cfg(unix)]
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn a_unix_endpoint_honours_the_option_too() {
    for (opted_in, name) in
        [(false, "tunnel-synthesis-unix-off"), (true, "tunnel-synthesis-unix-on")]
    {
        let dir = tempfile::tempdir().expect("socket dir");
        let socket = dir.path().join("shared.sock");
        let listener =
            tokio::net::UnixListener::bind(&socket).expect("bind the socket the client dials");
        let identity = PrivateIdentity::new_from_rand(OsRng);
        let transport = transport_under(name, &identity);
        let (mut events, iface) = attach(
            &transport,
            SharedInstanceEndpoint::Unix(LocalUnixEndpoint::filesystem(&socket)),
            SharedInstanceOptions::default().with_tunnel_synthesis(opted_in),
        )
        .await;
        let iface_hash = full_hash_of(&transport, iface).await;
        let (mut stream, _) = timeout(WAIT, listener.accept())
            .await
            .expect("unix connection timed out")
            .expect("unix connection failed");
        assert_eq!(next_event(name, &mut events).await, Some(SharedInstanceEvent::Connected));
        match first_frame_within_quiet(&mut stream, name).await {
            Some(packet) => {
                assert!(opted_in, "{name}: a default unix attach writes nothing on connect");
                assert_tunnel_packet(&packet, &identity, &iface_hash, name);
            }
            None => assert!(!opted_in, "{name}: an opted-in unix attach writes its tunnel packet"),
        }
        assert!(transport.detach_shared_instance().await, "{name}: the attach is held");
        drop(stream);
        drop(listener);
        timeout(WAIT, transport.stop()).await.expect("client stop completes");
    }
}

/// A consumer that drops the builder's return value under `deny(unused_must_use)`.
const DISCARDED: &str = "#![deny(unused_must_use)]\n\
     use rns_transport::transport::SharedInstanceOptions;\n\
     pub fn discard() { SharedInstanceOptions::default().with_tunnel_synthesis(true); }\n";

/// The builder is `#[must_use]`: a consumer that drops its return value is told the
/// options it built went nowhere.
#[test]
fn dropping_the_builder_result_is_refused_under_deny_unused_must_use() {
    let scratch = Scratch::new("tunnel-synthesis-must-use");
    let deps = deps_dir();
    let candidates = candidate_rlibs(&deps);
    assert!(!candidates.is_empty(), "no librns_transport-*.rlib next to {}", deps.display());
    let mut outputs = Vec::new();
    for candidate in candidates {
        let output = rustc(&scratch.0, &deps, &candidate, "tunnel_must_use", DISCARDED);
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        if !output.status.success() && stderr.contains("with_tunnel_synthesis") {
            assert!(
                stderr.contains("must be used"),
                "the rejection should be the must_use lint, not another error:\n{stderr}"
            );
            return;
        }
        outputs.push(format!("{}:\n{stderr}", candidate.display()));
    }
    panic!(
        "discarding `with_tunnel_synthesis` compiled, so the builder is not #[must_use]:\n{}",
        outputs.join("\n")
    );
}
