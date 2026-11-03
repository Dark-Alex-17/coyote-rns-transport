//! Usage probe for the reconciliation `Transport::interface_connect_count` promises a
//! lagging `interface_events()` subscriber: against a listener that accepts and drops a
//! plain `TcpClient` more times than the transport's 1024-event broadcast can hold, a
//! subscriber that stopped reading after the first `Connected` reads `RecvError::Lagged`
//! and compares the count it took before going idle with the count it takes afterwards;
//! the difference is the number of reconnects the dropped events would have shown it,
//! and the stream still delivers a `Connected` from the retained tail.

use std::time::Duration;

use rns_transport::transport::InterfaceEventKind;
use tokio::net::TcpListener;
use tokio::sync::broadcast::error::TryRecvError;
use tokio::time::timeout;

#[path = "support/accept_within.rs"]
mod accept_within;
#[path = "support/await_kind.rs"]
mod await_kind;
#[path = "support/client_transport.rs"]
mod client_transport;
#[path = "support/close_orderly.rs"]
mod close_orderly;
#[path = "support/spawn_plain_client.rs"]
mod spawn_plain_client;
#[path = "support/wait_for_count.rs"]
mod wait_for_count;
use accept_within::accept_within;
use await_kind::await_kind;
use client_transport::client_transport;
use close_orderly::close_orderly;
use spawn_plain_client::spawn_plain_client;
use wait_for_count::wait_for_count;

const WAIT: Duration = Duration::from_secs(5);
/// Short enough that several hundred redials fit in one test; the client's short-stream
/// arm sleeps it before every redial but the first of a streak.
const RECONNECT_WAIT: Duration = Duration::from_millis(1);
/// `Transport`'s interface-event broadcast capacity (`TRANSPORT_EVENT_CHANNEL_CAPACITY`).
const EVENT_CAPACITY: u64 = 1024;
/// Every accept-and-drop cycle publishes `Disconnected`, `Connecting` and `Connected`.
const EVENTS_PER_CYCLE: u64 = 3;
/// Enough cycles that the idle subscriber's backlog overflows the broadcast with margin.
const DROPS: u64 = 400;

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
    let (iface, _status) = spawn_plain_client(&transport, endpoint, RECONNECT_WAIT).await;

    let mut stream = accept_within(&listener, "the first connect").await;
    await_kind("the first Connected", &mut events, iface, |kind| {
        kind == InterfaceEventKind::Connected
    })
    .await;
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
        missed + EVENT_CAPACITY >= published - 1 && missed + EVENT_CAPACITY <= published,
        "Lagged({missed}) accounts for the {published} events the subscriber skipped minus the \
         {EVENT_CAPACITY} the broadcast retains; the count moves before the matching Connected \
         is published, so at most that one event is still in flight"
    );

    let after = transport.interface_connect_count(&iface).await;
    assert_eq!(
        after,
        Some(1 + DROPS),
        "the count read after the lag carries every reconnect the dropped events would have shown"
    );
    assert!(after > before, "two reads either side of a Lagged differ when a reconnect happened");

    // The retained tail is still a live stream: it delivers a Connected.
    await_kind("a Connected from the retained tail", &mut events, iface, |kind| {
        kind == InterfaceEventKind::Connected
    })
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

    let (steady_iface, _steady_status) =
        spawn_plain_client(&transport, steady_endpoint, RECONNECT_WAIT).await;
    let steady_stream = accept_within(&steady_listener, "the steady connect").await;
    await_kind("the steady Connected", &mut events, steady_iface, |kind| {
        kind == InterfaceEventKind::Connected
    })
    .await;
    let before = transport.interface_connect_count(&steady_iface).await;
    assert_eq!(before, Some(1));

    let (noisy_iface, _noisy_status) =
        spawn_plain_client(&transport, noisy_endpoint, RECONNECT_WAIT).await;
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
