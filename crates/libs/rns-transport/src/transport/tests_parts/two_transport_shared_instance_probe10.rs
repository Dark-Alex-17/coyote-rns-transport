// Pins the consumer-facing reconnect claims that no earlier fixture reads back:
// the `SharedInstanceOptions::with_reconnect_wait` rustdoc and the README's "clamped to at
// least 1 ms by both" (the TCP client's getter reads 1 ms for a zero wait; the TCP client
// redials at once the first time a stream drops and waits `reconnect_wait` from the second
// consecutive failure on; the unix client waits `reconnect_wait` before every retry, as
// `RNS/Interfaces/LocalInterface.py:160` does), the link-table count the drop's debug line
// now carries, and the reconnect consumer's rustdoc that the tunnel re-synthesis is not
// posture-gated. Helpers come from the sibling `tests_parts` files included into this
// module.

#[cfg(unix)]
use crate::iface::local::LocalUnixClient as Probe10LocalUnixClient;
use crate::iface::InterfaceManager as Probe10InterfaceManager;
use tokio::net::TcpListener as Probe10TcpListener;

/// A wait the redial timing below can tell apart from "at once" on a loaded host.
const SHARED_INSTANCE_PROBE10_REDIAL_WAIT: Duration = Duration::from_millis(1_500);

/// A redial observed sooner than this after the drop went out at once; slack for the
/// accept poll and the loopback handshake only.
const SHARED_INSTANCE_PROBE10_AT_ONCE: Duration = Duration::from_millis(700);

/// A redial that honoured the wait cannot be observed sooner than this after the drop; the
/// 300 ms taken off the wait is the slack of reading the clock after the drop itself.
const SHARED_INSTANCE_PROBE10_WAITED: Duration =
    SHARED_INSTANCE_PROBE10_REDIAL_WAIT.saturating_sub(Duration::from_millis(300));

/// How long a stream is held before the test drops it: short enough to count as a failed
/// attempt on the TCP client's side, long enough for the connect to have settled.
const SHARED_INSTANCE_PROBE10_HOLD: Duration = Duration::from_millis(50);

/// README: `reconnect_wait` is "clamped to at least 1 ms by both" clients, and the
/// `with_reconnect_wait` rustdoc says the options clamp it too. The TCP client and the
/// options expose the wait in force, so the clamp is read straight back.
#[test]
fn shared_instance_probe10_tcp_reconnect_wait_of_zero_is_clamped_to_one_millisecond() {
    assert_eq!(
        SharedInstanceOptions::default().with_reconnect_wait(Duration::ZERO).reconnect_wait(),
        Duration::from_millis(1),
        "the options clamp a zero wait to 1 ms at origin"
    );
    let client = TcpClient::new("127.0.0.1:1").with_reconnect_wait(Duration::ZERO);
    assert_eq!(client.reconnect_wait(), Duration::from_millis(1), "a zero wait reads as 1 ms");
    let client = TcpClient::new("127.0.0.1:1").with_reconnect_wait(Duration::from_millis(1));
    assert_eq!(client.reconnect_wait(), Duration::from_millis(1), "1 ms is left alone");
}

/// Accepts one connection on `listener`, holds it briefly and drops it; returns the instant
/// of the drop.
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_probe10_accept_hold_and_drop(listener: &Probe10TcpListener) -> Instant {
    let (stream, _) = timeout(LOOPBACK_TIMEOUT, listener.accept())
        .await
        .expect("timed out waiting for the client to connect")
        .expect("accept");
    tokio::time::sleep(SHARED_INSTANCE_PROBE10_HOLD).await;
    drop(stream);
    Instant::now()
}

/// `SharedInstanceOptions::with_reconnect_wait` rustdoc: "The TCP client redials at once
/// the first time an established stream drops and waits `wait` from the second consecutive
/// failure on." With the listener kept up, the re-accept after the first drop lands well
/// inside the wait; the re-accept after the second consecutive drop lands no sooner than
/// the wait.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_probe10_tcp_client_redials_at_once_on_the_first_drop_and_waits_from_the_second(
) {
    let listener = Probe10TcpListener::bind("127.0.0.1:0").await.expect("bind listener");
    let addr = listener.local_addr().expect("listener addr");
    let mut manager = Probe10InterfaceManager::new(8);
    let client =
        TcpClient::new(addr.to_string()).with_reconnect_wait(SHARED_INSTANCE_PROBE10_REDIAL_WAIT);
    let context = manager.new_context(client);
    let cancel = context.cancel.clone();
    let task = tokio::spawn(TcpClient::spawn(context));

    let first_drop = shared_instance_probe10_accept_hold_and_drop(&listener).await;
    let second_drop = shared_instance_probe10_accept_hold_and_drop(&listener).await;
    let first_redial =
        second_drop.saturating_duration_since(first_drop) - SHARED_INSTANCE_PROBE10_HOLD;
    assert!(
        first_redial < SHARED_INSTANCE_PROBE10_AT_ONCE,
        "the first redial after a drop took {first_redial:?}: it should go out at once"
    );

    let (third, _) = timeout(LOOPBACK_TIMEOUT, listener.accept())
        .await
        .expect("timed out waiting for the second redial")
        .expect("accept");
    let second_redial = Instant::now().saturating_duration_since(second_drop);
    assert!(
        second_redial >= SHARED_INSTANCE_PROBE10_WAITED,
        "the second consecutive redial came {second_redial:?} after the drop, before the \
         {SHARED_INSTANCE_PROBE10_REDIAL_WAIT:?} wait"
    );

    cancel.cancel();
    drop(third);
    timeout(STOP_TIMEOUT, task).await.expect("client task ends").expect("client task");
}

/// `SharedInstanceOptions::with_reconnect_wait` rustdoc: "the unix client waits `wait`
/// before every retry, as Python does (`:160`)". `RNS/Interfaces/LocalInterface.py:159-160`
/// sleeps `RECONNECT_WAIT` before every connect attempt of a reconnect, the first one
/// included. With the listener kept up, the re-accept after a drop therefore lands no
/// sooner than the wait.
#[cfg(unix)]
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_probe10_unix_client_waits_the_reconnect_wait_before_redialing_after_a_drop(
) {
    let temp = tempfile::tempdir().expect("tempdir");
    let socket = temp.path().join("probe10.sock");
    let listener = tokio::net::UnixListener::bind(&socket).expect("bind the socket");
    let mut manager = Probe10InterfaceManager::new(8);
    let client = Probe10LocalUnixClient::new_connect(LocalUnixEndpoint::filesystem(socket.clone()))
        .with_reconnect_wait(SHARED_INSTANCE_PROBE10_REDIAL_WAIT);
    let context = manager.new_context(client);
    let cancel = context.cancel.clone();
    let task = tokio::spawn(Probe10LocalUnixClient::spawn(context));

    let (stream, _) = timeout(LOOPBACK_TIMEOUT, listener.accept())
        .await
        .expect("timed out waiting for the unix client to connect")
        .expect("accept");
    tokio::time::sleep(SHARED_INSTANCE_PROBE10_HOLD).await;
    drop(stream);
    let dropped = Instant::now();

    let (again, _) = timeout(LOOPBACK_TIMEOUT, listener.accept())
        .await
        .expect("timed out waiting for the unix client to redial")
        .expect("accept");
    let redial = Instant::now().saturating_duration_since(dropped);
    assert!(
        redial >= SHARED_INSTANCE_PROBE10_WAITED,
        "the unix client redialed {redial:?} after the drop, before the \
         {SHARED_INSTANCE_PROBE10_REDIAL_WAIT:?} wait it is documented to sleep \
         (RNS/Interfaces/LocalInterface.py:160)"
    );

    cancel.cancel();
    drop(again);
    timeout(STOP_TIMEOUT, task).await.expect("client task ends").expect("client task");
}

/// Polls the test logger until a debug record of `name` containing `needle` is stored and
/// returns it.
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_probe10_await_debug_record(name: &str, needle: &str) -> String {
    let deadline = Instant::now() + LOOPBACK_TIMEOUT;
    loop {
        if let Some(record) = shared_instance_debug_records_containing(name, needle).pop() {
            return record;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for the `{needle}` debug record of tp({name}): {:?}",
            shared_instance_records_for(name)
        );
        tokio::time::sleep(LOOPBACK_POLL).await;
    }
}

/// The drop's debug line now reports the link table entries it cleared next to the links
/// it tore down: a client with one active link to the shared instance and one entry seeded
/// in its link table logs `1 link(s) torn down` and `1 link table entr(ies)` when the
/// shared instance restarts, the counts taken before the clear.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_probe10_drop_debug_line_counts_the_link_table_entries_it_clears() {
    install_test_logger();
    let name = "shared-instance-probe10-counts";
    let server = shared_instance_server().await;
    let mut client = shared_instance_reconnect_client(name, &server).await;
    server.await_accepted_connections(1).await;
    shared_instance_assert_attached(&client.transport, client.iface).await;
    let (_server_destination, _link_id) =
        shared_instance_reconnect_open_link(&server, &mut client).await;
    {
        let handler = client.transport.get_handler();
        let mut handler = handler.lock().await;
        let linked = AddressHash::new_from_rand(OsRng);
        handler.link_table.add(
            &Packet { destination: linked, ..Packet::default() },
            linked,
            AddressHash::new_from_rand(OsRng),
            AddressHash::new_from_rand(OsRng),
            client.iface,
        );
        assert_eq!(handler.link_table.len(), 1, "the link table is seeded");
    }

    let (restarted, _restarted_announces, _) = shared_instance_reconnect_restart(server).await;
    let record = shared_instance_probe10_await_debug_record(name, "disconnected,").await;
    assert!(
        record.contains("1 link(s) torn down"),
        "the drop line counts the active link it tore down: {record}"
    );
    assert!(
        record.contains("1 link table entr(ies)"),
        "the drop line counts the link table entries it cleared: {record}"
    );
    assert_eq!(
        client.transport.get_handler().lock().await.link_table.len(),
        0,
        "the link table is cleared"
    );

    timeout(STOP_TIMEOUT, client.transport.stop()).await.expect("client stop completes");
    timeout(STOP_TIMEOUT, restarted.transport.stop())
        .await
        .expect("restarted shared instance stop completes");
}

/// `SharedInstanceReconnectConsumer::on_reconnected` rustdoc: "Unlike the re-announce, the
/// tunnel re-synthesis is not posture-gated: it goes out for any interface that is still
/// registered." A client whose connected posture is dropped before the shared instance
/// restarts still re-synthesizes its tunnel on the reconnect, and re-announces nothing.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_probe10_posture_dropped_before_the_restart_still_resynthesizes_the_tunnel()
{
    install_test_logger();
    let name = "shared-instance-probe10-tunnel";
    let server = shared_instance_server().await;
    let mut server_announces = server.transport.recv_announces().await;
    let client = shared_instance_reconnect_client(name, &server).await;
    server.await_accepted_connections(1).await;
    shared_instance_assert_attached(&client.transport, client.iface).await;
    let first = shared_instance_reconnect_register(&client, &mut server_announces, "first").await;
    client.transport.set_connected_to_shared_instance(false).await;

    let (restarted, mut restarted_announces, _) = shared_instance_reconnect_restart(server).await;
    let record =
        shared_instance_probe10_await_debug_record(name, "reconnected, tunnel re-synthesized")
            .await;
    assert!(record.contains(&client.iface.to_string()), "the record names the iface: {record}");
    assert!(
        shared_instance_debug_records_containing(name, "tunnel not re-synthesized").is_empty(),
        "a registered interface re-synthesizes its tunnel whatever the posture"
    );

    let seen = shared_instance_probe7_path_responses_within(
        &mut restarted_announces,
        &[first],
        SHARED_INSTANCE_PROBE7_REANNOUNCE_SILENCE,
    )
    .await;
    assert!(seen.is_empty(), "a client whose posture was dropped re-announced: {seen:?}");

    timeout(STOP_TIMEOUT, client.transport.stop()).await.expect("client stop completes");
    timeout(STOP_TIMEOUT, restarted.transport.stop())
        .await
        .expect("restarted shared instance stop completes");
}
