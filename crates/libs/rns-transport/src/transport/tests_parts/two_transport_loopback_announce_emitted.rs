// Loopback coverage for `AnnounceEvent::emitted`, `AnnounceEvent::random_blob` and
// `Transport::latest_announce_emitted`: the event carries the emitter's emission second
// and the blob that encodes it, a re-announce is strictly newer, a transport relay replays
// the stored blob unchanged as a path response, and a destination answering a path request
// itself emits afresh. Helpers come from the sibling `tests_parts` files included into
// this module.

/// Whole seconds of the local clock, the reading `SingleInputDestination::announce`
/// stamps into the random blob (`now_secs().floor()`).
fn clock_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock after the epoch")
        .as_secs()
}

/// The emission second a random blob carries: bytes 5..10 big-endian
/// (`RNS/Transport.py:2217`). Decoded here independently of the transport's own helper.
fn blob_emitted(random_blob: &[u8; crate::destination::RAND_HASH_LENGTH]) -> u64 {
    let mut emitted = [0u8; 8];
    emitted[3..].copy_from_slice(&random_blob[5..]);
    u64::from_be_bytes(emitted)
}

/// A transport-enabled listener. `announce_retry_limit` 0 keeps the relay to one
/// rebroadcast of each announce it hears, so no second rebroadcast lands between a
/// peer's `expire_path` and the path response the test waits for.
#[deny(clippy::await_holding_invalid_type)]
async fn relay(name: &str) -> LoopbackListener {
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let mut config = TransportConfig::new(name, &identity, false);
    config.set_transport_enabled(true);
    config.set_announce_retry_limit(0);
    let transport = Transport::new(config);
    let port = free_loopback_port().await;
    let server = TcpServer::new(format!("127.0.0.1:{port}"), transport.iface_manager());
    let status = server.runtime_status_handle();
    transport.iface_manager().lock().await.spawn(server, TcpServer::spawn);
    wait_until(
        "relay to listen",
        || status.to_json()["listener_state"].as_str() == Some("listening"),
        || status.to_json().to_string(),
    )
    .await;
    LoopbackListener { transport, port, status }
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn announce_event_carries_the_emission_second_and_a_re_announce_is_newer() {
    let listener = listener("loopback-listener", TcpClient::DEFAULT_MTU).await;
    let mut connector =
        connector("loopback-connector", listener.port, TcpClient::DEFAULT_MTU).await;
    listener.await_accepted_connections(1).await;

    let before = clock_secs();
    let (destination, desc) =
        register_in_destination(&listener.transport, "loopback", "announce-emitted").await;
    let first_done = Instant::now();
    let after = clock_secs();
    let first = await_announce_for(&mut connector.announces, &desc.address_hash).await;

    assert!(
        before <= first.emitted && first.emitted <= after,
        "emitted {} lies in the clock bracket {before}..={after}",
        first.emitted
    );
    assert_eq!(blob_emitted(&first.random_blob), first.emitted, "the blob encodes emitted");
    assert_eq!(
        connector.transport.latest_announce_emitted(&desc.address_hash).await,
        Some(first.emitted),
        "the path table holds the one blob the announce carried"
    );
    assert_eq!(
        connector.transport.latest_announce_emitted(&AddressHash::new_from_rand(OsRng)).await,
        None,
        "no path table entry, no emission second"
    );

    // Equal hops need a strictly newer emission second (`RNS/Transport.py:2238`), so the
    // re-announce must fall in a later wall-clock second than the first.
    tokio::time::sleep_until(first_done + Duration::from_millis(1100)).await;
    let packet = destination.lock().await.announce(OsRng, None).expect("valid announce packet");
    listener.transport.send_packet(packet).await;
    let second = await_announce_for(&mut connector.announces, &desc.address_hash).await;

    assert!(
        second.emitted > first.emitted,
        "re-announce emitted {} is newer than {}",
        second.emitted,
        first.emitted
    );
    assert_ne!(second.random_blob, first.random_blob, "a fresh announce draws a fresh blob");
    assert_eq!(blob_emitted(&second.random_blob), second.emitted);
    assert_eq!(
        connector.transport.latest_announce_emitted(&desc.address_hash).await,
        Some(second.emitted),
        "the newest blob on file is the re-announce"
    );
}

/// The relay answers B's path request from its own announce cache (`RNS/Transport.py`
/// `path_request` with a known path), so B hears A's original announce bytes again under
/// `PacketContext::PathResponse`: same blob, same emission second.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn relayed_path_response_keeps_the_original_blob_and_emission_second() {
    let relay = relay("loopback-relay").await;
    let a = connector("loopback-a", relay.port, TcpClient::DEFAULT_MTU).await;
    let mut b = connector("loopback-b", relay.port, TcpClient::DEFAULT_MTU).await;
    relay.await_accepted_connections(2).await;

    let (_destination, desc) =
        register_in_destination(&a.transport, "loopback", "announce-relayed").await;
    let original = await_announce_for(&mut b.announces, &desc.address_hash).await;
    assert!(!original.is_path_response, "the relayed announce is unsolicited");
    assert_eq!(blob_emitted(&original.random_blob), original.emitted);

    assert!(b.transport.expire_path(&desc.address_hash).await, "the path table entry is removed");
    assert_eq!(b.transport.latest_announce_emitted(&desc.address_hash).await, None);
    b.transport.request_path(&desc.address_hash, None, None).await;
    let replayed = await_announce_for(&mut b.announces, &desc.address_hash).await;

    assert!(replayed.is_path_response, "the relay answered the path request");
    assert_eq!(replayed.random_blob, original.random_blob, "the relay replays the stored blob");
    assert_eq!(replayed.emitted, original.emitted, "a replay is not a fresh emission");
    assert_eq!(
        b.transport.latest_announce_emitted(&desc.address_hash).await,
        Some(original.emitted)
    );
}

/// The destination is registered without announcing, so the first announce the connector
/// hears is the listener's own answer to the path request, built afresh when the request
/// arrives (`RNS/Destination.py:308`): its emission second falls in the request bracket.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn own_path_response_is_a_fresh_emission() {
    let listener = listener("loopback-listener", TcpClient::DEFAULT_MTU).await;
    let mut connector =
        connector("loopback-connector", listener.port, TcpClient::DEFAULT_MTU).await;
    listener.await_accepted_connections(1).await;

    let identity = PrivateIdentity::new_from_rand(OsRng);
    let destination = listener
        .transport
        .add_destination(identity, DestinationName::new("loopback", "announce-own-response"))
        .await;
    let address_hash = destination.lock().await.desc.address_hash;

    let before = clock_secs();
    connector.transport.request_path(&address_hash, None, None).await;
    let event = await_announce_for(&mut connector.announces, &address_hash).await;
    let after = clock_secs();

    assert!(event.is_path_response, "the announce answered the path request");
    assert!(
        before <= event.emitted && event.emitted <= after,
        "emitted {} lies in the request bracket {before}..={after}",
        event.emitted
    );
    assert_eq!(blob_emitted(&event.random_blob), event.emitted);
    assert_eq!(
        connector.transport.latest_announce_emitted(&address_hash).await,
        Some(event.emitted)
    );
}
