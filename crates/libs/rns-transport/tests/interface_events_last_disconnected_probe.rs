//! Usage probes for the last-event clause of `Transport::interface_events` that names
//! `Disconnected { reason }`: a `TcpClient` whose stream ended just before it stopped,
//! or just before it gave up, publishes that `Disconnected` and then nothing. The
//! clause is read as a consumer would: subscribe, let the peer close the stream, then
//! stop the client inside its redial wait, or let a client with no retries left give
//! up, and watch the broadcast stay quiet after the `Disconnected`.
//!
//! A stream the peer closes right after the connect counts as a failed attempt: the
//! first such close redials at once, the second backs off for the reconnect wait, so a
//! stop lands inside that wait deterministically.

use std::time::Duration;

use rns_transport::hash::AddressHash;
use rns_transport::iface::tcp_client::TcpClient;
use rns_transport::transport::{DisconnectReason, InterfaceEvent, InterfaceEventKind};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;
use tokio::time::timeout;

#[path = "support/client_transport.rs"]
mod client_transport;
use client_transport::client_transport;

const WAIT: Duration = Duration::from_secs(5);
/// A redial wait long enough that a stop lands inside it.
const LONG_RECONNECT_WAIT: Duration = Duration::from_secs(10);
/// How long a stream is watched to show that nothing arrived on it.
const QUIET: Duration = Duration::from_millis(300);

#[deny(clippy::await_holding_invalid_type)]
async fn next_event(
    what: &str,
    events: &mut broadcast::Receiver<InterfaceEvent>,
) -> InterfaceEvent {
    timeout(WAIT, async {
        loop {
            match events.recv().await {
                Ok(event) => return event,
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => panic!("{what}: the interface event stream closed"),
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("{what} timed out"))
}

/// Everything tagged with `iface` that arrives within `QUIET`.
#[deny(clippy::await_holding_invalid_type)]
async fn drain_quiet(
    events: &mut broadcast::Receiver<InterfaceEvent>,
    iface: AddressHash,
) -> Vec<InterfaceEventKind> {
    let mut seen = Vec::new();
    assert!(
        timeout(QUIET, async {
            loop {
                match events.recv().await {
                    Ok(event) if event.iface() == iface => seen.push(event.kind()),
                    Ok(_) | Err(RecvError::Lagged(_)) => continue,
                    Err(RecvError::Closed) => return,
                }
            }
        })
        .await
        .is_err(),
        "the stream stayed open for the whole quiet window"
    );
    seen
}

#[deny(clippy::await_holding_invalid_type)]
async fn accept_within(listener: &TcpListener, what: &str) -> TcpStream {
    timeout(WAIT, listener.accept())
        .await
        .unwrap_or_else(|_| panic!("{what} timed out"))
        .unwrap_or_else(|err| panic!("{what} failed: {err}"))
        .0
}

/// Closes `stream` in an orderly way (write side shut down, client's EOF read) so the
/// client reads `PeerClosed` rather than a reset.
#[deny(clippy::await_holding_invalid_type)]
async fn close_orderly(mut stream: TcpStream) {
    stream.shutdown().await.expect("shut the write side down");
    timeout(WAIT, async {
        let mut sink = [0u8; 1024];
        while stream.read(&mut sink).await.expect("drain the client's writes") > 0 {}
    })
    .await
    .expect("timed out waiting for the client to close its side");
}

/// Accepts the client's connect on `listener`, reads `Connecting` then `Connected` for
/// `iface`, closes the accepted stream in an orderly way and reads
/// `Disconnected { PeerClosed }`.
#[deny(clippy::await_holding_invalid_type)]
async fn connect_then_peer_closes(
    what: &str,
    listener: &TcpListener,
    events: &mut broadcast::Receiver<InterfaceEvent>,
    iface: AddressHash,
) {
    let stream = accept_within(listener, &format!("{what}: the client's connect")).await;
    for (step, kind) in [
        ("Connecting", InterfaceEventKind::Connecting),
        ("Connected", InterfaceEventKind::Connected),
    ] {
        let event = next_event(&format!("{what}: {step}"), events).await;
        assert_eq!(event.iface(), iface, "{what}: {step} is tagged with the client");
        assert_eq!(event.kind(), kind, "{what}: {step}");
    }
    close_orderly(stream).await;
    let event = next_event(&format!("{what}: Disconnected"), events).await;
    assert_eq!(event.iface(), iface, "{what}: the disconnect is tagged with the client");
    assert_eq!(
        event.kind(),
        InterfaceEventKind::Disconnected { reason: DisconnectReason::PeerClosed },
        "{what}: an orderly close reads PeerClosed"
    );
}

#[deny(clippy::await_holding_invalid_type)]
async fn wait_until(what: &str, mut ready: impl FnMut() -> bool) {
    timeout(WAIT, async {
        while !ready() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("timed out waiting for {what}"));
}

/// The peer closes two streams in a row right after each connect; the second close puts
/// the client into its redial wait, where `Transport::stop_interface` stops it. The
/// broadcast reads `Connecting`, `Connected`, `Disconnected { PeerClosed }` twice and then
/// nothing: no redial `Connecting`, no third `Disconnected`.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn a_client_stopped_after_its_stream_ended_leaves_disconnected_as_its_last_event() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind listener");
    let endpoint = listener.local_addr().expect("listener address");
    let transport = client_transport("interface-events-last-disconnected-stop");
    let mut events = transport.interface_events();

    let client = TcpClient::new(endpoint.to_string()).with_reconnect_wait(LONG_RECONNECT_WAIT);
    let status = client.runtime_status_handle();
    let iface = transport.iface_manager().lock().await.spawn(client, TcpClient::spawn);

    connect_then_peer_closes("the first stream", &listener, &mut events, iface).await;
    connect_then_peer_closes("the second stream", &listener, &mut events, iface).await;
    wait_until("the client to enter its redial wait", || {
        status.to_json()["stream_state"].as_str() == Some("reconnecting")
    })
    .await;

    assert!(
        timeout(WAIT, transport.stop_interface(iface)).await.expect("stop_interface returns"),
        "the manager knew the interface"
    );
    wait_until("the stopped client to close", || {
        status.to_json()["stream_state"].as_str() == Some("closed")
    })
    .await;
    assert_eq!(
        drain_quiet(&mut events, iface).await,
        Vec::<InterfaceEventKind>::new(),
        "a stop inside the redial wait publishes nothing after the Disconnected"
    );

    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
}

/// The peer closes an established stream of a client with no retries left
/// (`max_reconnect_tries` of zero): the broadcast reads `Connecting`, `Connected`,
/// `Disconnected { PeerClosed }`, the client gives up, and nothing follows: no
/// `Connecting` of an attempt it never makes.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn a_client_with_no_retries_left_leaves_disconnected_as_its_last_event() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind listener");
    let endpoint = listener.local_addr().expect("listener address");
    let transport = client_transport("interface-events-last-disconnected-give-up");
    let mut events = transport.interface_events();

    let client = TcpClient::new(endpoint.to_string())
        .with_reconnect_wait(Duration::from_millis(50))
        .with_max_reconnect_tries(Some(0));
    let status = client.runtime_status_handle();
    let iface = transport.iface_manager().lock().await.spawn(client, TcpClient::spawn);

    connect_then_peer_closes("the only stream", &listener, &mut events, iface).await;
    drop(listener);
    wait_until("the client to give up", || {
        status.to_json()["stream_state"].as_str() == Some("closed")
    })
    .await;
    assert_eq!(
        drain_quiet(&mut events, iface).await,
        Vec::<InterfaceEventKind>::new(),
        "a give-up right after the peer's close publishes nothing after the Disconnected"
    );

    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
}
