// Loopback coverage for `Transport::resolve_destination`: a known destination answers
// at once with no path request, an unknown but reachable one is learned through exactly
// one path request and its path-response announce, an unreachable one times out after
// one request, and a zero timeout is a cached lookup alone. Helpers come from the
// sibling `tests_parts` files included into this module.

/// How long a channel is watched to show that nothing further arrives.
const RESOLVE_QUIET: Duration = Duration::from_millis(300);

/// Path requests the transport has sent for `destination`, one per send.
#[deny(clippy::await_holding_invalid_type)]
async fn outgoing_request_count(transport: &Transport, destination: &AddressHash) -> usize {
    transport.handler.lock().await.path_requests.outgoing_request_count(destination)
}

/// Drains announces for `hash` until `RESOLVE_QUIET` passes without one.
#[deny(clippy::await_holding_invalid_type)]
async fn drain_announces_for(
    announces: &mut broadcast::Receiver<AnnounceEvent>,
    hash: &AddressHash,
) -> Vec<AnnounceEvent> {
    let mut drained = Vec::new();
    loop {
        match timeout(RESOLVE_QUIET, announces.recv()).await {
            Ok(Ok(event)) => {
                if event.destination.lock().await.desc.address_hash == *hash {
                    drained.push(event);
                }
            }
            Ok(Err(RecvError::Lagged(_))) => continue,
            Ok(Err(RecvError::Closed)) => panic!("announce channel closed"),
            Err(_) => return drained,
        }
    }
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn resolve_destination_answers_a_known_destination_without_a_path_request() {
    let listener = listener("loopback-listener", TcpClient::DEFAULT_MTU).await;
    let mut connector =
        connector("loopback-connector", listener.port, TcpClient::DEFAULT_MTU).await;
    listener.await_accepted_connections(1).await;

    let (_destination, announced) =
        register_in_destination(&listener.transport, "loopback", "resolve-known").await;
    learn_announce(&mut connector.announces, &announced.address_hash).await;
    let requests_before =
        outgoing_request_count(&connector.transport, &announced.address_hash).await;

    let desc = timeout(
        Duration::from_secs(1),
        connector.transport.resolve_destination(&announced.address_hash, LOOPBACK_TIMEOUT),
    )
    .await
    .expect("a known destination answers without waiting")
    .expect("the connector learned the destination");

    assert_eq!(desc.address_hash, announced.address_hash);
    assert_eq!(
        outgoing_request_count(&connector.transport, &announced.address_hash).await,
        requests_before,
        "a known destination sends no path request"
    );
    let further = drain_announces_for(&mut connector.announces, &announced.address_hash).await;
    assert!(
        further.iter().all(|event| !event.is_path_response),
        "no path response follows a cached answer"
    );
}

/// The destination is registered without announcing, so the connector learns it only
/// through the path request the call sends and the announce that answers it.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn resolve_destination_learns_a_reachable_destination_through_one_path_request() {
    let listener = listener("loopback-listener", TcpClient::DEFAULT_MTU).await;
    let mut connector =
        connector("loopback-connector", listener.port, TcpClient::DEFAULT_MTU).await;
    listener.await_accepted_connections(1).await;

    let identity = PrivateIdentity::new_from_rand(OsRng);
    let destination = listener
        .transport
        .add_destination(identity, DestinationName::new("loopback", "resolve-reachable"))
        .await;
    let address_hash = destination.lock().await.desc.address_hash;
    assert_eq!(outgoing_request_count(&connector.transport, &address_hash).await, 0);

    let desc = connector
        .transport
        .resolve_destination(&address_hash, Duration::from_secs(5))
        .await
        .expect("the path request drew the announce");
    let requests_after = outgoing_request_count(&connector.transport, &address_hash).await;

    assert_eq!(desc.address_hash, address_hash);
    assert_eq!(requests_after, 1, "exactly one path request goes out");
    let first = await_announce_for(&mut connector.announces, &address_hash).await;
    assert!(first.is_path_response, "the announce answered the path request");
    let further = drain_announces_for(&mut connector.announces, &address_hash).await;
    assert!(further.is_empty(), "exactly one announce arrives for the destination");
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn resolve_destination_times_out_on_an_unreachable_destination_after_one_request() {
    let listener = listener("loopback-listener", TcpClient::DEFAULT_MTU).await;
    let connector = connector("loopback-connector", listener.port, TcpClient::DEFAULT_MTU).await;
    listener.await_accepted_connections(1).await;

    let unknown = AddressHash::new_from_rand(OsRng);
    let wait = Duration::from_millis(300);
    let started = Instant::now();
    let resolved = connector.transport.resolve_destination(&unknown, wait).await;
    let elapsed = started.elapsed();

    assert!(resolved.is_none(), "nobody holds the destination");
    assert!(elapsed >= wait, "the call waits out the timeout, waited {elapsed:?}");
    assert_eq!(outgoing_request_count(&connector.transport, &unknown).await, 1);
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn resolve_destination_with_zero_timeout_is_a_cached_lookup_alone() {
    let listener = listener("loopback-listener", TcpClient::DEFAULT_MTU).await;
    let connector = connector("loopback-connector", listener.port, TcpClient::DEFAULT_MTU).await;
    listener.await_accepted_connections(1).await;

    let unknown = AddressHash::new_from_rand(OsRng);
    let resolved = timeout(
        Duration::from_secs(1),
        connector.transport.resolve_destination(&unknown, Duration::ZERO),
    )
    .await
    .expect("a zero timeout answers at once");

    assert!(resolved.is_none(), "an unknown destination is not looked up on the network");
    assert_eq!(
        outgoing_request_count(&connector.transport, &unknown).await,
        0,
        "a zero timeout sends no path request"
    );
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn resolve_destination_with_zero_timeout_still_answers_a_known_destination() {
    let listener = listener("loopback-listener", TcpClient::DEFAULT_MTU).await;
    let mut connector =
        connector("loopback-connector", listener.port, TcpClient::DEFAULT_MTU).await;
    listener.await_accepted_connections(1).await;

    let (_destination, announced) =
        register_in_destination(&listener.transport, "loopback", "resolve-known-zero").await;
    learn_announce(&mut connector.announces, &announced.address_hash).await;

    let desc = connector
        .transport
        .resolve_destination(&announced.address_hash, Duration::ZERO)
        .await
        .expect("a known destination answers from the cache");

    assert_eq!(desc.address_hash, announced.address_hash);
    assert_eq!(outgoing_request_count(&connector.transport, &announced.address_hash).await, 0);
}
