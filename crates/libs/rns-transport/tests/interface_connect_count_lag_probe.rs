//! Usage probe for the reconciliation `Transport::interface_connect_count` promises a
//! lagging `interface_events()` subscriber: against a listener that accepts and drops a
//! plain `TcpClient` more times than the transport's 1024-event broadcast can hold, a
//! subscriber that stopped reading after the first `Connected` reads `RecvError::Lagged`
//! and compares the count it took before going idle with the count it takes afterwards;
//! the difference is the number of reconnects the dropped events would have shown it,
//! and the stream still delivers a `Connected` from the retained tail.

use std::time::{Duration, Instant};

use rns_transport::hash::AddressHash;
use rns_transport::iface::tcp_client::TcpClient;
use rns_transport::transport::{InterfaceEvent, InterfaceEventKind, Transport};
use tokio::io::{AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::{RecvError, TryRecvError};
use tokio::time::timeout;

#[path = "support/client_transport.rs"]
mod client_transport;
use client_transport::client_transport;

const WAIT: Duration = Duration::from_secs(5);
const POLL: Duration = Duration::from_millis(20);
/// Short enough that several hundred redials fit in one test; the client's short-stream
/// arm sleeps it before every redial but the first of a streak.
const RECONNECT_WAIT: Duration = Duration::from_millis(1);
/// `Transport`'s interface-event broadcast capacity (`TRANSPORT_EVENT_CHANNEL_CAPACITY`).
const EVENT_CAPACITY: u64 = 1024;
/// Every accept-and-drop cycle publishes `Disconnected`, `Connecting` and `Connected`.
const EVENTS_PER_CYCLE: u64 = 3;
/// Enough cycles that the idle subscriber's backlog overflows the broadcast with margin.
const DROPS: u64 = 400;

#[deny(clippy::await_holding_invalid_type)]
async fn wait_for_count(what: &str, transport: &Transport, iface: AddressHash, expected: u64) {
    let deadline = Instant::now() + WAIT;
    loop {
        let count = transport.interface_connect_count(&iface).await;
        if count == Some(expected) {
            return;
        }
        assert!(Instant::now() < deadline, "timed out waiting for {what}: read {count:?}");
        tokio::time::sleep(POLL).await;
    }
}

/// The next event tagged with `iface` whose kind is `kind`; other events and a `Lagged`
/// are skipped.
#[deny(clippy::await_holding_invalid_type)]
async fn await_kind(
    what: &str,
    events: &mut broadcast::Receiver<InterfaceEvent>,
    iface: AddressHash,
    kind: InterfaceEventKind,
) {
    timeout(WAIT, async {
        loop {
            match events.recv().await {
                Ok(event) if event.iface() == iface && event.kind() == kind => return,
                Ok(_) | Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => panic!("{what}: the interface event stream closed"),
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("{what} timed out"))
}

#[deny(clippy::await_holding_invalid_type)]
async fn accept_within(listener: &TcpListener, what: &str) -> TcpStream {
    timeout(WAIT, listener.accept())
        .await
        .unwrap_or_else(|_| panic!("{what} timed out"))
        .unwrap_or_else(|err| panic!("{what} failed: {err}"))
        .0
}

/// Shuts the write side down and reads to the client's EOF before dropping, so the
/// client sees an orderly close rather than a reset.
#[deny(clippy::await_holding_invalid_type)]
async fn close_orderly(mut stream: impl AsyncRead + AsyncWrite + Unpin) {
    stream.shutdown().await.expect("shut the write side down");
    timeout(WAIT, async {
        let mut sink = [0u8; 1024];
        while stream.read(&mut sink).await.expect("drain the client's writes") > 0 {}
    })
    .await
    .expect("timed out waiting for the client to close its side");
}

/// The subscriber reads the first `Connected` and the count `Some(1)`, then stops reading
/// while the listener drops the client `DROPS` times. Its next read is `Lagged(n)` with
/// `n` the overflow of the `3 * DROPS` events it missed; the count now reads
/// `Some(1 + DROPS)`, so the two reads differ by exactly the reconnects the dropped events
/// would have shown, and the retained tail still delivers a `Connected`.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn a_lagging_subscriber_learns_the_reconnects_from_two_count_reads() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind listener");
    let endpoint = listener.local_addr().expect("listener address");
    let transport = client_transport("connect-count-lag");
    let mut events = transport.interface_events();
    let client = TcpClient::new(endpoint.to_string()).with_reconnect_wait(RECONNECT_WAIT);
    let iface = transport.iface_manager().lock().await.spawn(client, TcpClient::spawn);

    let mut stream = accept_within(&listener, "the first connect").await;
    await_kind("the first Connected", &mut events, iface, InterfaceEventKind::Connected).await;
    let before = transport.interface_connect_count(&iface).await;
    assert_eq!(before, Some(1));

    // The subscriber goes idle; the listener flaps the client past the broadcast's capacity.
    for drop_index in 0..DROPS {
        close_orderly(stream).await;
        stream = accept_within(&listener, &format!("redial {}", drop_index + 1)).await;
    }
    wait_for_count("every redial to be counted", &transport, iface, 1 + DROPS).await;

    let missed = match events.try_recv() {
        Err(TryRecvError::Lagged(missed)) => missed,
        other => panic!("an idle subscriber behind {DROPS} drops reads Lagged, got {other:?}"),
    };
    let published = DROPS * EVENTS_PER_CYCLE;
    assert!(
        missed + EVENT_CAPACITY >= published - EVENTS_PER_CYCLE
            && missed + EVENT_CAPACITY <= published,
        "Lagged({missed}) accounts for the {published} events the subscriber skipped minus the \
         {EVENT_CAPACITY} the broadcast retains (one in-flight cycle of slack)"
    );

    let after = transport.interface_connect_count(&iface).await;
    assert_eq!(
        after,
        Some(1 + DROPS),
        "the count read after the lag carries every reconnect the dropped events would have shown"
    );
    assert!(after > before, "two reads either side of a Lagged differ when a reconnect happened");

    // The retained tail is still a live stream: it delivers a Connected.
    await_kind(
        "a Connected from the retained tail",
        &mut events,
        iface,
        InterfaceEventKind::Connected,
    )
    .await;

    close_orderly(stream).await;
    drop(listener);
    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
}

/// The complementary negative: a subscriber that lags over a quiet interval reads
/// `Lagged` from some other interface's traffic yet the same count before and after, so
/// "the count moved" is what tells a reconnect from noise. Here the noise is a second
/// client that flaps while the first stays connected on its single stream.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn a_lagging_subscriber_reads_an_unmoved_count_for_an_interface_that_kept_its_stream() {
    let steady_listener = TcpListener::bind("127.0.0.1:0").await.expect("bind steady listener");
    let steady_endpoint = steady_listener.local_addr().expect("steady address");
    let noisy_listener = TcpListener::bind("127.0.0.1:0").await.expect("bind noisy listener");
    let noisy_endpoint = noisy_listener.local_addr().expect("noisy address");
    let transport = client_transport("connect-count-lag-steady");
    let mut events = transport.interface_events();

    let steady = TcpClient::new(steady_endpoint.to_string()).with_reconnect_wait(RECONNECT_WAIT);
    let steady_iface = transport.iface_manager().lock().await.spawn(steady, TcpClient::spawn);
    let steady_stream = accept_within(&steady_listener, "the steady connect").await;
    await_kind("the steady Connected", &mut events, steady_iface, InterfaceEventKind::Connected)
        .await;
    let before = transport.interface_connect_count(&steady_iface).await;
    assert_eq!(before, Some(1));

    let noisy = TcpClient::new(noisy_endpoint.to_string()).with_reconnect_wait(RECONNECT_WAIT);
    let noisy_iface = transport.iface_manager().lock().await.spawn(noisy, TcpClient::spawn);
    let mut noisy_stream = accept_within(&noisy_listener, "the noisy connect").await;
    for drop_index in 0..DROPS {
        close_orderly(noisy_stream).await;
        noisy_stream =
            accept_within(&noisy_listener, &format!("noisy redial {}", drop_index + 1)).await;
    }
    wait_for_count("the noisy redials to be counted", &transport, noisy_iface, 1 + DROPS).await;

    assert!(
        matches!(events.try_recv(), Err(TryRecvError::Lagged(_))),
        "the idle subscriber lagged on the noisy interface's events"
    );
    let after = transport.interface_connect_count(&steady_iface).await;
    assert_eq!(after, before, "the steady interface's count did not move across the lag");

    close_orderly(steady_stream).await;
    close_orderly(noisy_stream).await;
    drop(steady_listener);
    drop(noisy_listener);
    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
}
