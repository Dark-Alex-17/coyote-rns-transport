//! The opt-in tunnel synthesis on `SharedInstanceOptions`, observed from outside the crate
//! against a raw `TcpListener`, which stands in for the shared instance so what the client
//! writes on a fresh stream is the test's to read. A default-option attach writes nothing
//! on connect, as Python's local client does (`RNS/Interfaces/LocalInterface.py` has no
//! tunnel code; `wants_tunnel` is set by the network-facing clients, such as
//! `RNS/Interfaces/TCPInterface.py:179`), and neither does one with
//! `with_tunnel_synthesis(false)`; an attach with
//! `with_tunnel_synthesis(true)` writes its tunnel packet as soon as the stream is up, and
//! again on every reconnect, where the default writes nothing again. The builder chains
//! with the other `with_*` options and follows its last call; its read-back accessor is
//! crate-private, and `Transport::synthesize_tunnel_on_interface` stays public for a
//! consumer that wants to synthesize by hand.

use std::time::Duration;

use rns_transport::transport::{
    SharedInstanceEndpoint, SharedInstanceEvent, SharedInstanceOptions,
};
use tokio::io::AsyncReadExt as _;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio::time::timeout;

#[path = "support/accept_within.rs"]
mod accept_within;
#[path = "support/attach_with_events.rs"]
mod attach_with_events;
use attach_with_events::attach_with_events;
#[path = "support/client_transport.rs"]
mod client_transport;
use client_transport::client_transport;
#[path = "support/compile_fail.rs"]
mod compile_fail;
use compile_fail::{artifact_root, candidate_rlibs, rustc, Scratch};
#[path = "support/next_event.rs"]
mod next_event;
use next_event::next_event;
#[path = "support/serve_connected.rs"]
mod serve_connected;
use serve_connected::serve_connected;

const WAIT: Duration = Duration::from_secs(5);
/// How long a fresh stream is watched for bytes the client should not write. The tunnel
/// packet of an opted-in client lands within milliseconds on loopback, so a quiet second
/// is well past where one would have arrived.
const QUIET: Duration = Duration::from_secs(1);

/// How many bytes `stream` carries within `QUIET` of the accept: zero when the client
/// writes nothing on connect, the tunnel packet's length when it does. An end of stream
/// is neither: the client never closes a stream it just connected, so it panics.
#[deny(clippy::await_holding_invalid_type)]
async fn bytes_within_quiet(stream: &mut TcpStream, what: &str) -> usize {
    let mut buf = [0u8; 1024];
    match timeout(QUIET, stream.read(&mut buf)).await {
        Err(_) => 0,
        Ok(Ok(0)) => panic!("{what}: the client closed the stream"),
        Ok(Ok(read)) => read,
        Ok(Err(err)) => panic!("{what}: the read failed: {err}"),
    }
}

/// Drops the served `stream` and reads the client's `Disconnected`, `Connecting`.
#[deny(clippy::await_holding_invalid_type)]
async fn drop_and_read_reconnect(
    stream: TcpStream,
    events: &mut mpsc::Receiver<SharedInstanceEvent>,
    what: &str,
) {
    drop(stream);
    assert_eq!(
        next_event(what, events, WAIT).await,
        Some(SharedInstanceEvent::Disconnected),
        "{what}: the drop is served"
    );
    assert_eq!(
        next_event(what, events, WAIT).await,
        Some(SharedInstanceEvent::Connecting),
        "{what}: `Connecting` follows the drop"
    );
}

/// Drives one attach with `options` through a first connect and one forced reconnect and
/// returns the bytes the client wrote within `QUIET` on each of the two streams.
#[deny(clippy::await_holding_invalid_type)]
async fn first_bytes_on_two_connects(name: &str, options: SharedInstanceOptions) -> (usize, usize) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind a loopback listener");
    let endpoint = listener.local_addr().expect("the listener's address");
    let transport = client_transport(name);
    let (mut events, _) =
        attach_with_events(&transport, SharedInstanceEndpoint::Tcp(endpoint), options).await;

    let mut first = serve_connected(&listener, &mut events, "first connection").await;
    let on_first = bytes_within_quiet(&mut first, "first connection").await;
    drop_and_read_reconnect(first, &mut events, "first drop").await;

    let mut second = serve_connected(&listener, &mut events, "second connection").await;
    let on_second = bytes_within_quiet(&mut second, "second connection").await;

    assert!(transport.detach_shared_instance().await, "the attach is still held");
    drop(second);
    timeout(WAIT, transport.stop()).await.expect("client stop completes");
    (on_first, on_second)
}

/// A default-option client writes nothing on its first stream and nothing on the stream
/// a forced drop makes it redial: no tunnel at attach, none on the reconnect.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn default_options_write_no_tunnel_at_attach_or_reconnect() {
    let (on_first, on_second) =
        first_bytes_on_two_connects("tunnel-synthesis-default", SharedInstanceOptions::default())
            .await;
    assert_eq!(on_first, 0, "a default attach writes nothing on connect");
    assert_eq!(on_second, 0, "a default reconnect writes nothing either");
}

/// `with_tunnel_synthesis(false)` spelled out reads the same as the default.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn explicit_false_writes_no_tunnel_either() {
    let (on_first, on_second) = first_bytes_on_two_connects(
        "tunnel-synthesis-explicit-off",
        SharedInstanceOptions::default().with_tunnel_synthesis(false),
    )
    .await;
    assert_eq!(on_first, 0, "an explicit off writes nothing on connect");
    assert_eq!(on_second, 0, "an explicit off writes nothing on the reconnect");
}

/// `with_tunnel_synthesis(true)` writes the tunnel packet on the first stream and again on
/// the stream the reconnect opens.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn opt_in_writes_the_tunnel_at_attach_and_again_on_reconnect() {
    let (on_first, on_second) = first_bytes_on_two_connects(
        "tunnel-synthesis-on",
        SharedInstanceOptions::default().with_tunnel_synthesis(true),
    )
    .await;
    assert!(on_first > 0, "an opted-in attach writes its tunnel packet on connect");
    assert!(on_second > 0, "an opted-in reconnect re-synthesizes the tunnel");
}

/// The option follows its last call: `true` then `false` is off, and the builder chains
/// around the other options.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn last_call_wins_and_the_builder_chains() {
    let (on_first, on_second) = first_bytes_on_two_connects(
        "tunnel-synthesis-last-call",
        SharedInstanceOptions::default()
            .with_tunnel_synthesis(true)
            .with_disconnect_reasons(false)
            .with_tunnel_synthesis(false),
    )
    .await;
    assert_eq!(on_first, 0, "the last `with_tunnel_synthesis` call is the one in force");
    assert_eq!(on_second, 0, "and it holds across the reconnect");
}

/// A consumer that attaches with the default and synthesizes a tunnel by hand through the
/// public `Transport::synthesize_tunnel_on_interface` sees the packet on the stream, and
/// the call reports the live interface as `true`.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn manual_synthesis_through_the_public_transport_method_writes_the_packet() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind a loopback listener");
    let endpoint = listener.local_addr().expect("the listener's address");
    let transport = client_transport("tunnel-synthesis-manual");
    let (mut events, _) = attach_with_events(
        &transport,
        SharedInstanceEndpoint::Tcp(endpoint),
        SharedInstanceOptions::default(),
    )
    .await;
    let mut stream = serve_connected(&listener, &mut events, "the connection").await;
    assert_eq!(bytes_within_quiet(&mut stream, "before the manual call").await, 0);

    let iface = transport.shared_instance().expect("the slot names the attached iface");
    assert!(
        transport.synthesize_tunnel_on_interface(iface).await,
        "the attached iface is live, so the public method synthesizes on it"
    );
    let mut buf = [0u8; 1024];
    let read = timeout(WAIT, stream.read(&mut buf))
        .await
        .expect("the manual tunnel packet arrives")
        .expect("read the manual tunnel packet");
    assert!(read > 0, "the manual call wrote a packet on the stream");

    assert!(transport.detach_shared_instance().await, "the attach is still held");
    drop(stream);
    timeout(WAIT, transport.stop()).await.expect("client stop completes");
}

/// What a downstream crate sees of the surface at compile time: the `#[must_use]` builder
/// chained with its neighbours, and the public method a default-off consumer calls by hand.
const CONSUMER: &str = r#"
use rns_transport::hash::AddressHash;
use rns_transport::transport::{SharedInstanceOptions, Transport};

pub fn tunneled_options() -> SharedInstanceOptions {
    SharedInstanceOptions::default()
        .with_disconnect_reasons(true)
        .with_tunnel_synthesis(true)
}

pub async fn by_hand(transport: &Transport, iface: AddressHash) -> bool {
    transport.synthesize_tunnel_on_interface(iface).await
}
"#;

/// The read-back accessor is crate-private: a consumer cannot ask an options value whether
/// its tunnel synthesis is on.
const REJECTED: &str = "use rns_transport::transport::SharedInstanceOptions;\n\
     pub fn is_on(options: &SharedInstanceOptions) -> bool { options.tunnel_synthesis() }\n";
const REJECTED_DIAGNOSTIC: &str = "method `tunnel_synthesis` is private";

#[test]
fn the_builder_compiles_for_a_consumer_and_the_accessor_does_not() {
    let scratch = Scratch::new("tunnel-synthesis-compile-fail");
    let deps = artifact_root();
    let candidates = candidate_rlibs(&deps);
    assert!(!candidates.is_empty(), "no librns_transport-*.rlib under {}", deps.display());
    let mut failures = Vec::new();
    let mut rlib = None;
    for candidate in candidates {
        let output = rustc(&scratch.0, &deps, &candidate, "tunnel_consumer", CONSUMER);
        if output.status.success() {
            rlib = Some(candidate);
            break;
        }
        failures.push(format!(
            "{}:\n{}",
            candidate.display(),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let rlib = rlib.unwrap_or_else(|| {
        panic!(
            "the opt-in builder and the public method should compile downstream:\n{}",
            failures.join("\n")
        )
    });

    let output = rustc(&scratch.0, &deps, &rlib, "tunnel_synthesis_accessor", REJECTED);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "tunnel_synthesis() compiled downstream; it is crate-private"
    );
    assert!(stderr.contains(REJECTED_DIAGNOSTIC), "not the promised diagnostic:\n{stderr}");
}
