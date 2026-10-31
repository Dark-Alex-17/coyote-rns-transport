// Usage-pattern coverage for `Transport::resolve_destination` beyond the four arms of
// `two_transport_resolve_destination.rs`: a caller that retries after a miss sends exactly
// one more path request per call and no request once the destination is learned, and the
// two documented splits hold: a destination deregistered while its path table entry stands
// answers `None` at once without a request, and a destination whose desc is held but whose
// path has expired answers `Some` at once without a request. Helpers come from the sibling
// `tests_parts` files included into this module. The two edges of the timeout are pinned
// as well: `Duration::MAX` waits instead of panicking, and a timeout shorter than one poll
// returns at its deadline.

/// A timeout the clock cannot hold: the call must wait, not panic on the deadline sum.
const RESOLVE_FOREVER: Duration = Duration::MAX;

/// A timeout shorter than the 50 ms poll, so the poll has to be cut to the time left.
const RESOLVE_SHORT: Duration = Duration::from_millis(10);

/// The upper bound on a short miss; generous so a loaded CI host does not fail the pin.
const RESOLVE_SHORT_SLACK: Duration = Duration::from_millis(200);

/// `Duration::MAX` is treated as waiting indefinitely: the call outlives an outer one-second
/// bound instead of panicking on the deadline sum, and it still spends its one path request.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn resolve_destination_with_a_max_timeout_waits_instead_of_panicking() {
    let listener = listener("loopback-listener", TcpClient::DEFAULT_MTU).await;
    let connector = connector("loopback-connector", listener.port, TcpClient::DEFAULT_MTU).await;
    listener.await_accepted_connections(1).await;

    let unknown = AddressHash::new_from_rand(OsRng);
    let outcome = timeout(
        Duration::from_secs(1),
        connector.transport.resolve_destination(&unknown, RESOLVE_FOREVER),
    )
    .await;
    assert!(outcome.is_err(), "an unbounded wait on an unreachable destination never returns");
    assert_eq!(
        outgoing_request_count(&connector.transport, &unknown).await,
        1,
        "the unbounded wait still spends exactly one path request"
    );
}

/// The same edge on `await_path` itself, which `resolve_destination` delegates the wait to.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn await_path_with_a_max_timeout_waits_instead_of_panicking() {
    let listener = listener("loopback-listener", TcpClient::DEFAULT_MTU).await;
    let connector = connector("loopback-connector", listener.port, TcpClient::DEFAULT_MTU).await;
    listener.await_accepted_connections(1).await;

    let unknown = AddressHash::new_from_rand(OsRng);
    let outcome = timeout(
        Duration::from_secs(1),
        connector.transport.await_path(&unknown, RESOLVE_FOREVER, None),
    )
    .await;
    assert!(outcome.is_err(), "an unbounded wait for a path nobody holds never returns");
    assert_eq!(
        outgoing_request_count(&connector.transport, &unknown).await,
        1,
        "the unbounded wait still spends exactly one path request"
    );
}

/// A timeout shorter than one poll still returns: the miss takes at least the timeout,
/// stays under a generous bound and spends one request.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn resolve_destination_with_a_timeout_shorter_than_a_poll_returns_at_the_deadline() {
    let listener = listener("loopback-listener", TcpClient::DEFAULT_MTU).await;
    let connector = connector("loopback-connector", listener.port, TcpClient::DEFAULT_MTU).await;
    listener.await_accepted_connections(1).await;

    let unknown = AddressHash::new_from_rand(OsRng);
    let started = Instant::now();
    let resolved = connector.transport.resolve_destination(&unknown, RESOLVE_SHORT).await;
    let elapsed = started.elapsed();

    assert!(resolved.is_none(), "nobody holds the destination");
    assert!(elapsed >= RESOLVE_SHORT, "the call waits out its timeout, waited {elapsed:?}");
    assert!(
        elapsed < RESOLVE_SHORT_SLACK,
        "a short timeout does not hold the caller, waited {elapsed:?}"
    );
    assert_eq!(
        outgoing_request_count(&connector.transport, &unknown).await,
        1,
        "the short wait spends exactly one path request"
    );
}

/// Two misses on an unreachable destination are two calls, each sending exactly one path
/// request; a caller that retries sees the count grow by one per call, never by more.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn resolve_destination_retried_after_a_miss_sends_one_more_request_per_call() {
    let listener = listener("loopback-listener", TcpClient::DEFAULT_MTU).await;
    let connector = connector("loopback-connector", listener.port, TcpClient::DEFAULT_MTU).await;
    listener.await_accepted_connections(1).await;

    let unknown = AddressHash::new_from_rand(OsRng);
    let wait = Duration::from_millis(200);
    for attempt in 1..=2usize {
        let resolved = connector.transport.resolve_destination(&unknown, wait).await;
        assert!(resolved.is_none(), "attempt {attempt}: nobody holds the destination");
        assert_eq!(
            outgoing_request_count(&connector.transport, &unknown).await,
            attempt,
            "attempt {attempt} sends exactly one more path request"
        );
    }
}

/// Once the path request has drawn the announce, a second call answers from the table and
/// sends nothing: the request count stays at the one the first call spent.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn resolve_destination_repeated_on_a_learned_destination_sends_no_second_request() {
    let listener = listener("loopback-listener", TcpClient::DEFAULT_MTU).await;
    let mut connector =
        connector("loopback-connector", listener.port, TcpClient::DEFAULT_MTU).await;
    listener.await_accepted_connections(1).await;

    let identity = PrivateIdentity::new_from_rand(OsRng);
    let destination = listener
        .transport
        .add_destination(identity, DestinationName::new("loopback", "resolve-twice"))
        .await;
    let address_hash = destination.lock().await.desc.address_hash;

    let first = connector
        .transport
        .resolve_destination(&address_hash, Duration::from_secs(5))
        .await
        .expect("the path request drew the announce");
    assert_eq!(outgoing_request_count(&connector.transport, &address_hash).await, 1);
    let announce = await_announce_for(&mut connector.announces, &address_hash).await;
    assert!(announce.is_path_response);

    let second = timeout(
        Duration::from_secs(1),
        connector.transport.resolve_destination(&address_hash, Duration::from_secs(5)),
    )
    .await
    .expect("a learned destination answers without waiting")
    .expect("the table still holds the destination");

    assert_eq!(
        second.address_hash, first.address_hash,
        "both calls return the one desc the table holds"
    );
    assert_eq!(second.address_hash, address_hash);
    assert_eq!(
        outgoing_request_count(&connector.transport, &address_hash).await,
        1,
        "a learned destination costs no second path request"
    );
    let further = drain_announces_for(&mut connector.announces, &address_hash).await;
    assert!(further.is_empty(), "no announce follows the cached answer");
}

/// The first split the rustdoc names: `deregister_destination` drops the learned entry
/// but leaves the path table alone, so `await_path` takes the standing path as its answer
/// and the call returns `None` at once, sending no path request that could refill the entry.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn resolve_destination_on_a_deregistered_destination_with_a_path_answers_none_at_once() {
    let listener = listener("loopback-listener", TcpClient::DEFAULT_MTU).await;
    let mut connector =
        connector("loopback-connector", listener.port, TcpClient::DEFAULT_MTU).await;
    listener.await_accepted_connections(1).await;

    let (_destination, announced) =
        register_in_destination(&listener.transport, "loopback", "resolve-deregistered").await;
    learn_announce(&mut connector.announces, &announced.address_hash).await;
    assert!(connector.transport.destination_desc(&announced.address_hash).await.is_some());

    assert!(
        connector.transport.deregister_destination(&announced.address_hash).await,
        "the learned destination is removed"
    );
    assert!(
        connector.transport.has_path(&announced.address_hash).await,
        "deregistering leaves the path table entry standing"
    );
    let requests_before =
        outgoing_request_count(&connector.transport, &announced.address_hash).await;

    let started = Instant::now();
    let resolved = timeout(
        Duration::from_secs(1),
        connector.transport.resolve_destination(&announced.address_hash, Duration::from_secs(5)),
    )
    .await
    .expect("the standing path answers at once, not after the timeout");

    assert!(resolved.is_none(), "the entry is gone and no announce refilled it");
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "the call does not wait out its timeout, took {:?}",
        started.elapsed()
    );
    assert_eq!(
        outgoing_request_count(&connector.transport, &announced.address_hash).await,
        requests_before,
        "no path request goes out while the path stands"
    );
}

/// The opposite split: `expire_path` drops the path table entry but leaves the learned
/// desc, so the cached lookup answers `Some` at once and no path request goes out; a
/// caller that then needs a next hop still has to call `await_path` itself.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn resolve_destination_on_a_held_desc_with_an_expired_path_answers_some_at_once() {
    let listener = listener("loopback-listener", TcpClient::DEFAULT_MTU).await;
    let mut connector =
        connector("loopback-connector", listener.port, TcpClient::DEFAULT_MTU).await;
    listener.await_accepted_connections(1).await;

    let (_destination, announced) =
        register_in_destination(&listener.transport, "loopback", "resolve-stale-path").await;
    learn_announce(&mut connector.announces, &announced.address_hash).await;
    assert!(connector.transport.has_path(&announced.address_hash).await);

    assert!(
        connector.transport.expire_path(&announced.address_hash).await,
        "the path table entry is removed"
    );
    assert!(
        !connector.transport.has_path(&announced.address_hash).await,
        "expiring leaves no path behind"
    );
    assert!(
        connector.transport.destination_desc(&announced.address_hash).await.is_some(),
        "expiring the path leaves the learned desc standing"
    );
    let requests_before =
        outgoing_request_count(&connector.transport, &announced.address_hash).await;

    let started = Instant::now();
    let resolved = timeout(
        Duration::from_secs(5),
        connector.transport.resolve_destination(&announced.address_hash, Duration::from_secs(1)),
    )
    .await
    .expect("the held desc answers well inside the outer bound")
    .expect("the desc is still held after the path expired");

    assert_eq!(resolved.address_hash, announced.address_hash);
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "the call does not wait out its own timeout on a path, took {:?}",
        started.elapsed()
    );
    assert_eq!(
        outgoing_request_count(&connector.transport, &announced.address_hash).await,
        requests_before,
        "no path request goes out for a held desc"
    );
}
