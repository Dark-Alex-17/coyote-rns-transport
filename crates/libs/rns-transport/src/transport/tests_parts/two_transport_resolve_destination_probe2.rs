// Usage-pattern coverage for `Transport::resolve_destination` as a caller with a budget
// uses it: a lookup whose timeout is too short to carry the answer still spends its one
// path request, and once the path response has landed a zero-timeout retry answers from
// the cache without a second request; two callers resolving the same unknown destination
// at once each spend their one request and both receive the one desc the table holds.
// Helpers come from the sibling `tests_parts` files included into this module.

/// A timeout far below the loopback round trip, so the first poll decides the call.
const RESOLVE_TOO_SHORT: Duration = Duration::from_millis(1);

/// Registers `aspect` on the listener without announcing it, so the connector can learn
/// it only through a path request, and returns its address.
#[deny(clippy::await_holding_invalid_type)]
async fn register_unannounced(listener: &LoopbackListener, aspect: &str) -> AddressHash {
    let identity = PrivateIdentity::new_from_rand(OsRng);
    listener
        .transport
        .add_destination(identity, DestinationName::new("loopback", aspect))
        .await
        .lock()
        .await
        .desc
        .address_hash
}

/// The too-short call may miss or may catch the answer on its first poll; either way it
/// spends exactly one path request, the path response lands afterwards, and the
/// zero-timeout retry a budgeted caller falls back to answers `Some` for free.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn resolve_destination_too_short_timeout_spends_one_request_and_the_late_answer_is_cached() {
    let listener = listener("loopback-listener", TcpClient::DEFAULT_MTU).await;
    let mut connector =
        connector("loopback-connector", listener.port, TcpClient::DEFAULT_MTU).await;
    listener.await_accepted_connections(1).await;

    let address_hash = register_unannounced(&listener, "resolve-too-short").await;
    assert_eq!(outgoing_request_count(&connector.transport, &address_hash).await, 0);

    let started = Instant::now();
    let first = timeout(
        Duration::from_secs(2),
        connector.transport.resolve_destination(&address_hash, RESOLVE_TOO_SHORT),
    )
    .await
    .expect("a too-short timeout does not hold the caller for long");
    let elapsed = started.elapsed();
    assert!(elapsed >= RESOLVE_TOO_SHORT, "the call waits out its timeout, waited {elapsed:?}");
    assert_eq!(
        outgoing_request_count(&connector.transport, &address_hash).await,
        1,
        "a non-zero timeout spends exactly one path request, hit or miss"
    );
    if let Some(desc) = &first {
        assert_eq!(desc.address_hash, address_hash, "a hit on the first poll is the right desc");
    }

    let announce = await_announce_for(&mut connector.announces, &address_hash).await;
    assert!(announce.is_path_response, "the one request drew the one path response");

    let cached = timeout(
        Duration::from_secs(1),
        connector.transport.resolve_destination(&address_hash, Duration::ZERO),
    )
    .await
    .expect("a zero timeout answers at once")
    .expect("the late answer filled the cache the zero-timeout retry reads");
    assert_eq!(cached.address_hash, address_hash);
    assert_eq!(
        outgoing_request_count(&connector.transport, &address_hash).await,
        1,
        "the zero-timeout retry sends nothing"
    );
    let further = drain_announces_for(&mut connector.announces, &address_hash).await;
    assert!(further.is_empty(), "no second announce follows the cached retry");
}

/// Two callers resolving the same unknown destination at once each spend their one path
/// request (the call coalesces nothing, as documented), both get `Some`, and the two descs
/// are the one entry the table holds.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn resolve_destination_concurrent_callers_each_spend_one_request_and_both_resolve() {
    let listener = listener("loopback-listener", TcpClient::DEFAULT_MTU).await;
    let mut connector =
        connector("loopback-connector", listener.port, TcpClient::DEFAULT_MTU).await;
    listener.await_accepted_connections(1).await;

    let address_hash = register_unannounced(&listener, "resolve-concurrent").await;
    let wait = Duration::from_secs(5);

    let (left, right) = tokio::join!(
        connector.transport.resolve_destination(&address_hash, wait),
        connector.transport.resolve_destination(&address_hash, wait),
    );
    let left = left.expect("the first caller resolves");
    let right = right.expect("the second caller resolves");

    assert_eq!(left.address_hash, address_hash);
    assert_eq!(right.address_hash, address_hash);
    assert_eq!(
        outgoing_request_count(&connector.transport, &address_hash).await,
        2,
        "each caller spends exactly one path request; nothing is coalesced"
    );
    let first = await_announce_for(&mut connector.announces, &address_hash).await;
    assert!(first.is_path_response, "the announce answered a path request");
    let held = connector
        .transport
        .destination_desc(&address_hash)
        .await
        .expect("the table holds the learned destination");
    assert_eq!(held.address_hash, left.address_hash, "both callers read the table's entry");
}
