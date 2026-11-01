//! Usage probes for the "publishes no further event" clause of
//! `Transport::interface_events`: a `TcpClient` that gives up after
//! `max_reconnect_tries` publishes one `Connecting` and one `DialFailed` per attempt
//! and then nothing, its last event being the `DialFailed` of the attempt it
//! abandoned; a `TcpClient` stopped through `Transport::stop_interface` while it waits
//! to redial publishes nothing after the `DialFailed` it had already published. Both
//! read beside the per-client dial-failure sender, which sees the same attempts.

use std::time::Duration;

use rns_transport::hash::AddressHash;
use rns_transport::identity::PrivateIdentity;
use rns_transport::iface::events::DialFailure;
use rns_transport::iface::tcp_client::TcpClient;
use rns_transport::rand_core::OsRng;
use rns_transport::transport::{
    DialFailureKind, InterfaceEvent, InterfaceEventKind, Transport, TransportConfig,
};
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::mpsc;
use tokio::time::timeout;

const WAIT: Duration = Duration::from_secs(5);
const RECONNECT_WAIT: Duration = Duration::from_millis(50);
/// A redial wait long enough that a stop lands inside it.
const LONG_RECONNECT_WAIT: Duration = Duration::from_secs(10);
/// How long a stream is watched to show that nothing arrived on it.
const QUIET: Duration = Duration::from_millis(300);

fn client_transport(name: &str) -> Transport {
    Transport::new(TransportConfig::new(name, &PrivateIdentity::new_from_rand(OsRng), false))
}

#[deny(clippy::await_holding_invalid_type)]
async fn free_loopback_port() -> u16 {
    let probe = TcpListener::bind("127.0.0.1:0").await.expect("bind probe");
    let port = probe.local_addr().expect("probe address").port();
    drop(probe);
    port
}

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

/// Everything tagged with `iface` that arrives within `QUIET`; a closed stream ends the
/// watch, since a stopped transport may drop its sender.
#[deny(clippy::await_holding_invalid_type)]
async fn drain_quiet(
    events: &mut broadcast::Receiver<InterfaceEvent>,
    iface: AddressHash,
) -> Vec<InterfaceEventKind> {
    let mut seen = Vec::new();
    let _ = timeout(QUIET, async {
        loop {
            match events.recv().await {
                Ok(event) if event.iface() == iface => seen.push(event.kind()),
                Ok(_) | Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => return,
            }
        }
    })
    .await;
    seen
}

/// One attempt's pair on the broadcast, tagged with `iface`: `Connecting` then
/// `DialFailed { Refused }`.
#[deny(clippy::await_holding_invalid_type)]
async fn assert_refused_attempt(
    what: &str,
    events: &mut broadcast::Receiver<InterfaceEvent>,
    iface: AddressHash,
) {
    let event = next_event(&format!("{what}: Connecting"), events).await;
    assert_eq!(event.iface(), iface, "{what}: the attempt is tagged with the client");
    assert_eq!(event.kind(), InterfaceEventKind::Connecting, "{what}: an attempt opens");
    let event = next_event(&format!("{what}: DialFailed"), events).await;
    assert_eq!(event.iface(), iface, "{what}: the failure is tagged with the client");
    assert_eq!(
        event.kind(),
        InterfaceEventKind::DialFailed { reason: DialFailureKind::Refused },
        "{what}: nothing listens on the port"
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

/// A client dialing a port nobody listens on with `max_reconnect_tries = 1` publishes
/// the first attempt's `Connecting` and `DialFailed { Refused }`, the one retry's pair,
/// and then nothing: the `DialFailed` of the abandoned attempt is its last event, no
/// `Disconnected` follows the give-up, and the per-client dial-failure sender saw the
/// same two attempts.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn a_client_that_gives_up_publishes_no_further_event() {
    let port = free_loopback_port().await;
    let transport = client_transport("interface-events-give-up");
    let mut events = transport.interface_events();

    let (failures_tx, mut failures) = mpsc::channel::<DialFailure>(8);
    let client = TcpClient::new(format!("127.0.0.1:{port}"))
        .with_reconnect_wait(RECONNECT_WAIT)
        .with_max_reconnect_tries(Some(1))
        .with_dial_failure_events(failures_tx);
    let status = client.runtime_status_handle();
    let iface = transport.iface_manager().lock().await.spawn(client, TcpClient::spawn);

    assert_refused_attempt("the first attempt", &mut events, iface).await;
    assert_refused_attempt("the retry", &mut events, iface).await;
    wait_until("the client to give up", || {
        status.to_json()["stream_state"].as_str() == Some("closed")
    })
    .await;

    assert_eq!(
        drain_quiet(&mut events, iface).await,
        Vec::<InterfaceEventKind>::new(),
        "the give-up publishes nothing after the abandoned attempt's DialFailed"
    );
    let mut per_client = Vec::new();
    while let Ok(failure) = failures.try_recv() {
        per_client.push((failure.iface(), failure.kind()));
    }
    assert_eq!(
        per_client,
        vec![(iface, DialFailureKind::Refused), (iface, DialFailureKind::Refused)],
        "the per-client sender saw the same two attempts and nothing more"
    );

    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
}

/// A client stopped through `Transport::stop_interface` while it waits to redial
/// publishes nothing after the `DialFailed` it had already published: no second
/// `Connecting`, no `Disconnected`.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn a_client_stopped_while_waiting_to_redial_publishes_no_further_event() {
    let port = free_loopback_port().await;
    let transport = client_transport("interface-events-stop-mid-dial");
    let mut events = transport.interface_events();

    let client =
        TcpClient::new(format!("127.0.0.1:{port}")).with_reconnect_wait(LONG_RECONNECT_WAIT);
    let status = client.runtime_status_handle();
    let iface = transport.iface_manager().lock().await.spawn(client, TcpClient::spawn);

    assert_refused_attempt("the only attempt", &mut events, iface).await;
    assert_eq!(
        status.to_json()["stream_state"].as_str(),
        Some("reconnecting"),
        "the client is inside its redial wait"
    );

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
        "a stop inside the redial wait publishes nothing after the DialFailed"
    );

    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
}
