//! Usage probes for `Transport::interface_events` from a consumer's seat, beside the
//! pins in `interface_events_api.rs`: two receivers from one transport each read the
//! whole sequence under the same addresses, a `stop_interface` or a `Transport::stop`
//! of a connected client publishes no `Disconnected` on the broadcast (the per-client
//! reason channel is quiet for the same stop), a receiver subscribed after a client
//! connected reads nothing it missed, and the transport's stream holds the 1024 events
//! the README names before a receiver that fell behind reads `Lagged`.

use std::net::SocketAddr;
use std::time::Duration;

use rns_transport::hash::AddressHash;
use rns_transport::iface::tcp_client::TcpClient;
use rns_transport::transport::{DisconnectReason, InterfaceEvent, InterfaceEventKind, Transport};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::{RecvError, TryRecvError};
use tokio::sync::mpsc;
use tokio::time::timeout;

#[path = "support/client_transport.rs"]
mod client_transport;
use client_transport::client_transport;

const WAIT: Duration = Duration::from_secs(5);
const RECONNECT_WAIT: Duration = Duration::from_millis(50);
/// How long a stream is watched to show that nothing arrived on it.
const QUIET: Duration = Duration::from_millis(300);
/// The capacity the README gives the transport's event streams.
const TRANSPORT_EVENT_CAPACITY: usize = 1024;

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

/// Reads events tagged with `iface` until `Connected` arrives and returns every kind read
/// for it on the way, `Connected` last.
#[deny(clippy::await_holding_invalid_type)]
async fn read_until_connected(
    what: &str,
    events: &mut broadcast::Receiver<InterfaceEvent>,
    iface: AddressHash,
) -> Vec<InterfaceEventKind> {
    let mut kinds = Vec::new();
    loop {
        let event = next_event(what, events).await;
        assert_eq!(event.iface(), iface, "{what}: every event is tagged with the client");
        kinds.push(event.kind());
        if event.kind() == InterfaceEventKind::Connected {
            return kinds;
        }
    }
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

/// Shuts the write side down and reads the client to EOF before the stream drops, so
/// the client reads a FIN rather than a reset.
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

#[deny(clippy::await_holding_invalid_type)]
async fn spawn_plain_client(transport: &Transport, endpoint: SocketAddr) -> AddressHash {
    let client = TcpClient::new(endpoint.to_string()).with_reconnect_wait(RECONNECT_WAIT);
    transport.iface_manager().lock().await.spawn(client, TcpClient::spawn)
}

/// Two receivers taken from one transport before the client is spawned each read the
/// same `Connecting`, `Connected`, `Disconnected { PeerClosed }`, `Connecting` under the
/// same address: the stream is a broadcast, not a queue one reader drains for the other.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_receivers_from_one_transport_each_read_the_whole_sequence() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind listener");
    let endpoint = listener.local_addr().expect("listener address");
    let transport = client_transport("interface-events-probe-two-receivers");
    let mut first = transport.interface_events();
    let mut second = transport.interface_events();
    let iface = spawn_plain_client(&transport, endpoint).await;
    let stream = accept_within(&listener, "the connect").await;

    let expected = [InterfaceEventKind::Connecting, InterfaceEventKind::Connected];
    assert_eq!(read_until_connected("first receiver", &mut first, iface).await, expected);
    assert_eq!(read_until_connected("second receiver", &mut second, iface).await, expected);

    close_orderly(stream).await;
    drop(listener);
    for (what, events) in [("first receiver", &mut first), ("second receiver", &mut second)] {
        let event = next_event(what, events).await;
        assert_eq!(event.iface(), iface);
        assert_eq!(
            event.kind(),
            InterfaceEventKind::Disconnected { reason: DisconnectReason::PeerClosed },
            "{what}: each receiver reads the close with its reason"
        );
        let event = next_event(what, events).await;
        assert_eq!(event.iface(), iface);
        assert_eq!(event.kind(), InterfaceEventKind::Connecting, "{what}: and the redial");
    }

    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
}

/// A connected client stopped through `Transport::stop_interface` publishes no
/// `Disconnected`: the stream stays quiet for its address while the per-client reason
/// channel stays quiet too. A second connected client taken down by `Transport::stop`
/// publishes nothing either.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn a_stop_publishes_no_disconnected_on_the_broadcast() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind listener");
    let endpoint = listener.local_addr().expect("listener address");
    let transport = client_transport("interface-events-probe-stop");
    let mut events = transport.interface_events();

    let (reason_tx, mut reasons) = mpsc::channel(4);
    let stopped_client = TcpClient::new(endpoint.to_string())
        .with_reconnect_wait(RECONNECT_WAIT)
        .with_disconnect_reason_events(reason_tx);
    let stopped = transport.iface_manager().lock().await.spawn(stopped_client, TcpClient::spawn);
    let stopped_stream = accept_within(&listener, "the first connect").await;
    assert_eq!(
        read_until_connected("the stopped client", &mut events, stopped).await,
        [InterfaceEventKind::Connecting, InterfaceEventKind::Connected]
    );

    assert!(
        timeout(WAIT, transport.stop_interface(stopped)).await.expect("stop_interface returns"),
        "the manager knew the interface"
    );
    // The peer sees the client's side close; the client itself was stopped, not disconnected.
    timeout(WAIT, async {
        let mut stream = stopped_stream;
        let mut sink = [0u8; 1024];
        while stream.read(&mut sink).await.unwrap_or(0) > 0 {}
    })
    .await
    .expect("the stopped client closes its socket");
    assert_eq!(
        drain_quiet(&mut events, stopped).await,
        Vec::<InterfaceEventKind>::new(),
        "a stop_interface publishes nothing on the broadcast"
    );
    // The stopped client dropped its sender on the way out; either way no reason was sent.
    assert!(reasons.try_recv().is_err(), "nor on the reason channel: {:?}", reasons.try_recv());

    let survivor = spawn_plain_client(&transport, endpoint).await;
    let survivor_stream = accept_within(&listener, "the second connect").await;
    assert_eq!(
        read_until_connected("the surviving client", &mut events, survivor).await,
        [InterfaceEventKind::Connecting, InterfaceEventKind::Connected]
    );
    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
    timeout(WAIT, async {
        let mut stream = survivor_stream;
        let mut sink = [0u8; 1024];
        while stream.read(&mut sink).await.unwrap_or(0) > 0 {}
    })
    .await
    .expect("the stopped transport's client closes its socket");
    assert_eq!(
        drain_quiet(&mut events, survivor).await,
        Vec::<InterfaceEventKind>::new(),
        "a Transport::stop publishes nothing on the broadcast"
    );
    drop(listener);
}

/// The stream has no replay: a receiver subscribed after the client connected reads the
/// close as its first event, never the `Connecting` or `Connected` it missed.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn a_receiver_subscribed_late_reads_nothing_it_missed() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind listener");
    let endpoint = listener.local_addr().expect("listener address");
    let transport = client_transport("interface-events-probe-no-replay");
    let mut early = transport.interface_events();
    let iface = spawn_plain_client(&transport, endpoint).await;
    let stream = accept_within(&listener, "the connect").await;
    assert_eq!(
        read_until_connected("the early receiver", &mut early, iface).await,
        [InterfaceEventKind::Connecting, InterfaceEventKind::Connected]
    );

    let mut late = transport.interface_events();
    assert_eq!(late.try_recv(), Err(TryRecvError::Empty), "nothing is replayed on subscribe");
    close_orderly(stream).await;
    drop(listener);
    let event = next_event("the late receiver", &mut late).await;
    assert_eq!(event.iface(), iface);
    assert_eq!(
        event.kind(),
        InterfaceEventKind::Disconnected { reason: DisconnectReason::PeerClosed },
        "the first event a late receiver reads is the first one published after it subscribed"
    );
    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
}

/// The transport's stream holds 1024 events, as the README says: a receiver left
/// unread while a channel minted from the transport's manager publishes 1024 events
/// reads all of them in order and no `Lagged`; one more and the receiver reads
/// `Lagged(1)` first.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn the_transport_stream_holds_the_1024_events_the_readme_names() {
    let transport = client_transport("interface-events-probe-capacity");
    let mut events = transport.interface_events();
    let channel = transport.iface_manager().lock().await.new_channel(8);

    for _ in 0..TRANSPORT_EVENT_CAPACITY {
        channel.publish_event(InterfaceEventKind::Connecting);
    }
    for n in 0..TRANSPORT_EVENT_CAPACITY {
        let event = events.try_recv().unwrap_or_else(|err| panic!("event {n}: {err:?}"));
        assert_eq!(event.iface(), *channel.address());
        assert_eq!(event.kind(), InterfaceEventKind::Connecting);
    }
    assert_eq!(events.try_recv(), Err(TryRecvError::Empty), "exactly 1024 fit");

    for _ in 0..=TRANSPORT_EVENT_CAPACITY {
        channel.publish_event(InterfaceEventKind::Connected);
    }
    assert_eq!(events.try_recv(), Err(TryRecvError::Lagged(1)), "the 1025th pushes one out");
    for n in 0..TRANSPORT_EVENT_CAPACITY {
        let event = events.try_recv().unwrap_or_else(|err| panic!("event {n}: {err:?}"));
        assert_eq!(event.kind(), InterfaceEventKind::Connected);
    }
    assert_eq!(events.try_recv(), Err(TryRecvError::Empty));
    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
}
