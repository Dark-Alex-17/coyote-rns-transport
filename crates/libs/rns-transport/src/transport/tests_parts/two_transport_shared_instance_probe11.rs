// Pins two consumer-facing claims of the drop teardown that no earlier fixture reads
// back from the outside: the drop's LINKCLOSE goes to the peer of an active link the client
// is the RESPONDER of (Python's `active_links` holds both sides, `RNS/Transport.py:3692`;
// the flush fixture covers the initiator side only), and the unix client's new
// `reconnect_wait` sleep before a redial is cancel-aware, so a `stop` issued inside a long
// wait completes at once instead of waiting out the stop budget. Helpers come from the
// sibling `tests_parts` files included into this module.

#[cfg(unix)]
use crate::iface::local::LocalUnixClient as Probe11LocalUnixClient;
#[cfg(unix)]
use crate::iface::local::LocalUnixEndpoint as Probe11LocalUnixEndpoint;

/// A redial wait long enough that a `stop` which had to wait it out would exhaust the stop
/// budget first; the stop below must not come anywhere near it.
#[cfg(unix)]
const SHARED_INSTANCE_PROBE11_LONG_WAIT: Duration = Duration::from_secs(30);

/// A stop that honoured the cancel inside the redial sleep ends well inside this; one that
/// slept the wait out ends only when `STOP_BUDGET` aborts the worker (5 s).
#[cfg(unix)]
const SHARED_INSTANCE_PROBE11_PROMPT_STOP: Duration = Duration::from_secs(2);

/// How long the raw listener holds the stream before dropping it, so the client has settled
/// into its connected read loop and the drop is a genuine established-stream drop.
#[cfg(unix)]
const SHARED_INSTANCE_PROBE11_HOLD: Duration = Duration::from_millis(100);

/// `shared_connection_disappeared` tears down every link in `active_links`
/// (`RNS/Transport.py:3692`), the ones the node accepted included, and `Link.teardown`
/// sends LINKCLOSE for each (`RNS/Link.py:657-668`). A third transport that initiated a
/// link to a destination on the client, over a plain TCP interface, therefore learns the
/// close from the wire when the client's shared instance drops, with the responder-side
/// reason `DestinationClosed` rather than its own watchdog's `Timeout`.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_probe11_drop_sends_link_close_to_the_initiator_of_an_accepted_link() {
    install_test_logger();
    let name = "shared-instance-probe11-responder";
    let server = shared_instance_server().await;
    let client = shared_instance_reconnect_client(name, &server).await;
    server.await_accepted_connections(1).await;
    shared_instance_assert_attached(&client.transport, client.iface).await;

    let third = listener("shared-instance-probe11-third", TcpClient::DEFAULT_MTU).await;
    let mut third_announces = third.transport.recv_announces().await;
    let tcp = TcpClient::new(format!("127.0.0.1:{}", third.port)).with_mtu(TcpClient::DEFAULT_MTU);
    let status = tcp.runtime_status_handle();
    let context = client.transport.iface_manager().lock().await.new_context(tcp);
    tokio::spawn(TcpClient::spawn(context));
    wait_until(
        "client to reach the third transport",
        || status.to_json()["stream_state"].as_str() == Some("connected"),
        || status.to_json().to_string(),
    )
    .await;
    third.await_accepted_connections(1).await;

    let mut client_in_events = client.transport.in_link_events();
    let (_destination, desc) =
        register_in_destination(&client.transport, "shared-instance-probe11", "client").await;
    let (learned, _) =
        shared_instance_learn_announce(&mut third_announces, &desc.address_hash).await;
    let mut third_out_events = third.transport.out_link_events();
    let (_link, link_id) = open_link(&third.transport, learned).await;
    let activated = await_event(&mut client_in_events, |event| {
        event.id == link_id && matches!(event.event, LinkEvent::Activated)
    })
    .await;
    assert_eq!(activated.id, link_id, "both ends share one link id");

    let (restarted, _restarted_announces, _) = shared_instance_reconnect_restart(server).await;
    let record = shared_instance_probe10_await_debug_record(name, "disconnected,").await;
    assert!(
        record.contains("1 link(s) torn down (1 close packet(s) sent)"),
        "the drop line counts the accepted link and its close packet: {record}"
    );

    let closed = await_event(&mut third_out_events, |event| {
        event.id == link_id && matches!(event.event, LinkEvent::Closed)
    })
    .await;
    assert_eq!(
        closed.close_reason,
        Some(LinkCloseReason::DestinationClosed),
        "the initiator learned the close from the responder's LINKCLOSE, not from a timeout"
    );
    assert!(
        client.transport.get_handler().lock().await.in_links.is_empty(),
        "the accepted link left the client's in_links"
    );
    shared_instance_assert_never_warned(name);

    timeout(STOP_TIMEOUT, client.transport.stop()).await.expect("client stop completes");
    timeout(STOP_TIMEOUT, third.transport.stop()).await.expect("third transport stop completes");
    timeout(STOP_TIMEOUT, restarted.transport.stop())
        .await
        .expect("restarted shared instance stop completes");
}

/// `LocalUnixClient::with_reconnect_wait` rustdoc: the wait is slept "before each redial
/// after an established stream drops". The sleep must still yield to the transport's
/// shutdown: a `stop` issued while the client is inside a 30 s redial wait completes at
/// once, not when the 5 s stop budget aborts a worker that is still asleep.
#[cfg(unix)]
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_probe11_stop_inside_the_unix_redial_wait_completes_at_once() {
    install_test_logger();
    let name = "shared-instance-probe11-stop-in-wait";
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let transport = Transport::new(TransportConfig::new(name, &identity, false));
    let temp = tempfile::tempdir().expect("tempdir");
    let socket = temp.path().join("probe11.sock");
    let listener = tokio::net::UnixListener::bind(&socket).expect("bind the socket");

    let iface = transport
        .attach_shared_instance(
            SharedInstanceEndpoint::Unix(Probe11LocalUnixEndpoint::filesystem(socket.clone())),
            SharedInstanceOptions::default().with_reconnect_wait(SHARED_INSTANCE_PROBE11_LONG_WAIT),
        )
        .await
        .expect("attach to the unix socket");
    shared_instance_assert_attached(&transport, iface).await;

    let (stream, _) = timeout(LOOPBACK_TIMEOUT, listener.accept())
        .await
        .expect("timed out waiting for the unix client to connect")
        .expect("accept");
    tokio::time::sleep(SHARED_INSTANCE_PROBE11_HOLD).await;
    drop(stream);
    // The client has observed the EOF once the drop flush has run; the redial wait starts
    // right after that disconnect event is sent.
    let dropped = Instant::now();
    let record = shared_instance_probe10_await_debug_record(name, "disconnected,").await;
    let noticed = dropped.elapsed();
    assert!(
        record.contains("0 link(s) torn down"),
        "nothing to tear down on a bare socket: {record}"
    );
    tokio::time::sleep(SHARED_INSTANCE_PROBE11_HOLD).await;

    let started = Instant::now();
    timeout(STOP_TIMEOUT, transport.stop()).await.expect("stop completes inside its timeout");
    let took = started.elapsed();
    assert!(
        took < SHARED_INSTANCE_PROBE11_PROMPT_STOP,
        "stop took {took:?} with the unix client inside a {SHARED_INSTANCE_PROBE11_LONG_WAIT:?} \
         redial wait (drop noticed after {noticed:?}): the wait did not yield to the cancel"
    );
    shared_instance_assert_released(&transport, iface).await;
    assert!(
        timeout(SHARED_INSTANCE_PROBE11_HOLD, listener.accept()).await.is_err(),
        "a stopped client must not redial"
    );
}

/// CHANGELOG: "a client built from a pre-supplied stream still finishes at once when that
/// stream ends". With a 30 s `reconnect_wait` configured, a `LocalUnixClient::new_from_stream`
/// worker whose peer end is dropped ends inside the hold, neither sleeping the wait nor
/// redialing anything.
#[cfg(unix)]
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_probe11_from_stream_unix_client_ends_at_once_when_the_stream_drops() {
    let (ours, theirs) = tokio::net::UnixStream::pair().expect("socket pair");
    let mut manager = Probe10InterfaceManager::new(8);
    let client = Probe11LocalUnixClient::new_from_stream("probe11-pair", theirs)
        .with_reconnect_wait(SHARED_INSTANCE_PROBE11_LONG_WAIT);
    let context = manager.new_context(client);
    let task = tokio::spawn(Probe11LocalUnixClient::spawn(context));

    tokio::time::sleep(SHARED_INSTANCE_PROBE11_HOLD).await;
    assert!(!task.is_finished(), "the from-stream client runs while its stream is open");
    drop(ours);
    let dropped = Instant::now();
    timeout(SHARED_INSTANCE_PROBE11_PROMPT_STOP, task)
        .await
        .unwrap_or_else(|_| {
            panic!(
                "the from-stream client was still running {:?} after its stream ended",
                dropped.elapsed()
            )
        })
        .expect("from-stream client task");
}
