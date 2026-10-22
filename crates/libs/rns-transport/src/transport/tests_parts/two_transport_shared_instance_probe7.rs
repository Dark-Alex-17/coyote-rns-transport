// Usage-pattern probes for the shared-instance client parity behaviours, seventh set,
// written from `RNS/Transport.py:2911-2916`, `:3690-3709` and
// `RNS/Interfaces/LocalInterface.py:153-181` before the implementation was read: the
// registration announce waits its quarter second rather than going out at once, the
// reconnect re-announce waits `reconnect_wait + 2 s` rather than firing on the reconnect
// itself, a client whose connected posture is dropped between the reconnect and that timer
// re-announces nothing, so does one whose interface is unflagged as the shared instance in
// that window, every further restart re-announces again, a pending (never proven) link is
// torn down by the drop like an active one, and the `TcpClient` reconnect-wait knob
// defaults to five seconds. Helpers come from the sibling `tests_parts` files included
// into this module.

/// Python sleeps 0.25 s before the registration announce (`RNS/Transport.py:2914`); an
/// announce reaching the shared instance sooner than this went out early. Loopback
/// latency only ever adds to the observed gap, so the bound is safe on a slow host.
/// The 100 ms taken off the nominal delay is a conservative margin only: the clock is
/// read before the registration call, so the timer can never lead it.
const SHARED_INSTANCE_PROBE7_REGISTRATION_FLOOR: Duration =
    super::links::SHARED_INSTANCE_REGISTRATION_ANNOUNCE_DELAY
        .saturating_sub(Duration::from_millis(100));

/// Python sleeps `RECONNECT_WAIT + 2` before `shared_connection_reappeared`
/// (`RNS/Interfaces/LocalInterface.py:178-181`); with the fixture's 100 ms wait a
/// re-announce reaching the shared instance sooner than this fired early. The 300 ms
/// taken off the nominal delay is poll slack: the clock is read only after the server-side
/// accept poll notices the reconnect, so the client's timer can lead it by that much.
const SHARED_INSTANCE_PROBE7_REANNOUNCE_FLOOR: Duration = SHARED_INSTANCE_RECONNECT_WAIT
    .saturating_add(Duration::from_secs(2))
    .saturating_sub(Duration::from_millis(300));

/// Long enough for a re-announce that should not be sent to have arrived if it were going
/// to: the bound the reconnect fixture asserts, plus a margin.
const SHARED_INSTANCE_PROBE7_REANNOUNCE_SILENCE: Duration =
    SHARED_INSTANCE_RECONNECT_REANNOUNCE_BOUND.saturating_add(Duration::from_secs(1));

/// Drains `announces` for `window`, returning every path-response announce seen for one of
/// `watched` in that time.
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_probe7_path_responses_within(
    announces: &mut broadcast::Receiver<AnnounceEvent>,
    watched: &[AddressHash],
    window: Duration,
) -> Vec<AddressHash> {
    let mut seen = Vec::new();
    let deadline = Instant::now() + window;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return seen;
        }
        match timeout(remaining, announces.recv()).await {
            Ok(Ok(event)) => {
                let address_hash = event.destination.lock().await.desc.address_hash;
                if event.is_path_response && watched.contains(&address_hash) {
                    seen.push(address_hash);
                }
            }
            Ok(Err(RecvError::Lagged(_))) => continue,
            Ok(Err(RecvError::Closed)) => panic!("announce channel closed"),
            Err(_) => return seen,
        }
    }
}

/// `RNS/Transport.py:2911-2916`: the registration announce is scheduled 0.25 s after the
/// destination is registered, not sent inline. Nothing reaches the shared instance inside
/// the floor; the path-response announce then arrives.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_probe7_registration_announce_waits_the_quarter_second() {
    let server = shared_instance_server().await;
    let client = shared_instance_client("shared-instance-probe7-delay", &server).await;
    server.await_accepted_connections(1).await;
    shared_instance_assert_attached(&client.transport, client.iface).await;
    let mut server_announces = server.transport.recv_announces().await;

    let identity = PrivateIdentity::new_from_rand(OsRng);
    let registered_at = Instant::now();
    let destination = client
        .transport
        .add_destination(identity, DestinationName::new("shared-instance-probe7", "delay"))
        .await;
    let address_hash = destination.lock().await.desc.address_hash;

    let early = timeout(SHARED_INSTANCE_PROBE7_REGISTRATION_FLOOR, server_announces.recv()).await;
    assert!(
        early.is_err(),
        "the registration announce went out inside {SHARED_INSTANCE_PROBE7_REGISTRATION_FLOOR:?}"
    );
    let event = shared_instance_parity_await_announce(&mut server_announces, &address_hash).await;
    let elapsed = registered_at.elapsed();
    assert!(event.is_path_response, "the registration announce is a path response");
    assert!(
        elapsed >= SHARED_INSTANCE_PROBE7_REGISTRATION_FLOOR,
        "the registration announce arrived after {elapsed:?}"
    );
    let latest = super::links::SHARED_INSTANCE_REGISTRATION_ANNOUNCE_DELAY + Duration::from_secs(2);
    assert!(
        elapsed <= latest,
        "the registration announce arrived after {elapsed:?}, past the {latest:?} bound"
    );

    timeout(STOP_TIMEOUT, client.transport.stop()).await.expect("client stop completes");
    timeout(STOP_TIMEOUT, server.transport.stop()).await.expect("shared instance stop completes");
}

/// `RNS/Interfaces/LocalInterface.py:178-181`: the re-announce fires `reconnect_wait + 2 s`
/// after the reconnect, not on the reconnect itself. The first re-announce arrives no
/// sooner than the floor after the re-accept, and the last one inside the reconnect
/// fixture's bound. The reconnect's own tunnel re-synthesis is not an announce and does
/// not count.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_probe7_reannounce_waits_the_reconnect_wait_plus_two_seconds() {
    let server = shared_instance_server().await;
    let mut server_announces = server.transport.recv_announces().await;
    let client = shared_instance_reconnect_client("shared-instance-probe7-floor", &server).await;
    server.await_accepted_connections(1).await;
    shared_instance_assert_attached(&client.transport, client.iface).await;
    let first = shared_instance_reconnect_register(&client, &mut server_announces, "first").await;
    let second = shared_instance_reconnect_register(&client, &mut server_announces, "second").await;

    let (restarted, mut restarted_announces, reaccepted_at) =
        shared_instance_reconnect_restart(server).await;

    // The two re-announces go out in the destination map's order, so the first to arrive
    // is whichever of the two that is: both are awaited together and the earliest arrival
    // kept, rather than awaiting one and then the other (the helper discards what it is
    // not waiting for).
    let (first_reannounce_at, last_reannounce_at) = timeout(LOOPBACK_TIMEOUT, async {
        let mut expected = vec![first, second];
        let mut earliest = None;
        while !expected.is_empty() {
            match restarted_announces.recv().await {
                Ok(event) => {
                    let address_hash = event.destination.lock().await.desc.address_hash;
                    if event.is_path_response && expected.contains(&address_hash) {
                        earliest.get_or_insert_with(Instant::now);
                        expected.retain(|hash| *hash != address_hash);
                    }
                }
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => panic!("announce channel closed"),
            }
        }
        (earliest.expect("at least one re-announce arrived"), Instant::now())
    })
    .await
    .expect("timed out waiting for the re-announces");
    let first_elapsed = first_reannounce_at.saturating_duration_since(reaccepted_at);
    assert!(
        first_elapsed >= SHARED_INSTANCE_PROBE7_REANNOUNCE_FLOOR,
        "the re-announce fired {first_elapsed:?} after the re-accept, before the \
         {SHARED_INSTANCE_PROBE7_REANNOUNCE_FLOOR:?} floor"
    );
    let last_elapsed = last_reannounce_at.saturating_duration_since(reaccepted_at);
    assert!(
        last_elapsed <= SHARED_INSTANCE_RECONNECT_REANNOUNCE_BOUND,
        "the last re-announce arrived {last_elapsed:?} after the re-accept"
    );

    timeout(STOP_TIMEOUT, client.transport.stop()).await.expect("client stop completes");
    timeout(STOP_TIMEOUT, restarted.transport.stop())
        .await
        .expect("restarted shared instance stop completes");
}

/// `RNS/Transport.py:3705-3706`: `shared_connection_reappeared` announces only while
/// `is_connected_to_shared_instance` still holds when the timer fires. A client whose
/// connected posture is dropped right after the re-accept, with its interface still up,
/// sends no re-announce.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_probe7_posture_dropped_after_the_reconnect_reannounces_nothing() {
    let server = shared_instance_server().await;
    let mut server_announces = server.transport.recv_announces().await;
    let client = shared_instance_reconnect_client("shared-instance-probe7-gate", &server).await;
    server.await_accepted_connections(1).await;
    shared_instance_assert_attached(&client.transport, client.iface).await;
    let first = shared_instance_reconnect_register(&client, &mut server_announces, "first").await;

    let (restarted, mut restarted_announces, _) = shared_instance_reconnect_restart(server).await;
    client.transport.set_connected_to_shared_instance(false).await;
    assert_eq!(client.transport.shared_instance(), Some(client.iface), "the slot keeps the iface");
    assert!(
        client.transport.interface_hashes().await.contains(&client.iface),
        "the client's interface is still up"
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

/// `RNS/Transport.py:3705-3706` together with the fork's twin of the `not self.detached`
/// check (`RNS/Interfaces/LocalInterface.py:259`): the re-announce also needs the interface
/// to still be the one flagged as the shared instance when the timer fires. A client whose
/// interface is unflagged right after the re-accept, with the connected posture kept and
/// the interface still up, sends no re-announce.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_probe7_interface_unflagged_after_the_reconnect_reannounces_nothing() {
    let server = shared_instance_server().await;
    let mut server_announces = server.transport.recv_announces().await;
    let client = shared_instance_reconnect_client("shared-instance-probe7-unflag", &server).await;
    server.await_accepted_connections(1).await;
    shared_instance_assert_attached(&client.transport, client.iface).await;
    let first = shared_instance_reconnect_register(&client, &mut server_announces, "first").await;

    let (restarted, mut restarted_announces, _) = shared_instance_reconnect_restart(server).await;
    assert!(client.transport.iface_manager().lock().await.set_shared_instance(client.iface, false));
    assert_eq!(client.transport.shared_instance(), Some(client.iface), "the slot keeps the iface");
    assert!(client.transport.connected_to_shared_instance().await, "the connected posture is kept");
    assert!(
        client.transport.interface_hashes().await.contains(&client.iface),
        "the client's interface is still up"
    );

    let seen = shared_instance_probe7_path_responses_within(
        &mut restarted_announces,
        &[first],
        SHARED_INSTANCE_PROBE7_REANNOUNCE_SILENCE,
    )
    .await;
    assert!(seen.is_empty(), "a client whose interface was unflagged re-announced: {seen:?}");

    timeout(STOP_TIMEOUT, client.transport.stop()).await.expect("client stop completes");
    timeout(STOP_TIMEOUT, restarted.transport.stop())
        .await
        .expect("restarted shared instance stop completes");
}

/// `RNS/Interfaces/LocalInterface.py:153-181` runs on every reconnect: a second restart of
/// the shared instance re-announces every registered destination again, within the same
/// bound of the second re-accept.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_probe7_every_restart_reannounces_again() {
    let server = shared_instance_server().await;
    let mut server_announces = server.transport.recv_announces().await;
    let client = shared_instance_reconnect_client("shared-instance-probe7-twice", &server).await;
    server.await_accepted_connections(1).await;
    shared_instance_assert_attached(&client.transport, client.iface).await;
    let first = shared_instance_reconnect_register(&client, &mut server_announces, "first").await;
    let second = shared_instance_reconnect_register(&client, &mut server_announces, "second").await;

    let mut server = server;
    for round in 1..=2 {
        let (restarted, mut restarted_announces, reaccepted_at) =
            shared_instance_reconnect_restart(server).await;
        let last_reannounce_at = shared_instance_reconnect_await_reannounces(
            &mut restarted_announces,
            vec![first, second],
        )
        .await;
        let elapsed = last_reannounce_at.saturating_duration_since(reaccepted_at);
        assert!(
            elapsed <= SHARED_INSTANCE_RECONNECT_REANNOUNCE_BOUND,
            "restart {round}: the re-announces arrived {elapsed:?} after the re-accept"
        );
        restarted.await_tunneled_destinations(vec![first, second]).await;
        assert!(
            client.transport.connected_to_shared_instance().await,
            "restart {round}: the connected posture survives"
        );
        server = restarted;
    }

    timeout(STOP_TIMEOUT, client.transport.stop()).await.expect("client stop completes");
    timeout(STOP_TIMEOUT, server.transport.stop()).await.expect("shared instance stop completes");
}

/// `RNS/Transport.py:3690-3696` tears down the pending links as well as the active ones.
/// A link whose request the shared instance never answers (its destination was
/// deregistered after the client learned the path) is closed by the drop with the
/// initiator-side reason, and the path it was opened on is flushed.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_probe7_drop_tears_down_a_pending_link() {
    let server = shared_instance_server().await;
    let mut client =
        shared_instance_reconnect_client("shared-instance-probe7-pending", &server).await;
    server.await_accepted_connections(1).await;
    shared_instance_assert_attached(&client.transport, client.iface).await;

    let (_destination, desc) =
        register_in_destination(&server.transport, "shared-instance-probe7", "pending").await;
    let (learned, _) =
        shared_instance_learn_announce(&mut client.announces, &desc.address_hash).await;
    assert!(
        server.transport.deregister_destination(&desc.address_hash).await,
        "the shared instance forgets its destination so the link request goes unanswered"
    );
    let mut client_out_events = client.transport.out_link_events();
    let link = client.transport.link(learned).await.expect("link request sent");
    let link_id = *link.lock().await.id();
    let early = timeout(Duration::from_millis(500), async {
        await_event(&mut client_out_events, |event| event.id == link_id).await
    })
    .await;
    assert!(early.is_err(), "the link saw an event before the drop");

    let (restarted, _restarted_announces, _) = shared_instance_reconnect_restart(server).await;

    let closed = await_event(&mut client_out_events, |event| {
        event.id == link_id && matches!(event.event, LinkEvent::Closed)
    })
    .await;
    assert_eq!(
        closed.close_reason,
        Some(LinkCloseReason::InitiatorClosed),
        "the pending link is torn down with the initiator-side reason"
    );
    shared_instance_reconnect_await_path_gone(&client.transport, &desc.address_hash).await;

    timeout(STOP_TIMEOUT, client.transport.stop()).await.expect("client stop completes");
    timeout(STOP_TIMEOUT, restarted.transport.stop())
        .await
        .expect("restarted shared instance stop completes");
}

/// The `TcpClient` reconnect-wait knob that `attach_shared_instance` drives: five seconds
/// unless set, and exactly what was set otherwise.
#[test]
fn shared_instance_probe7_tcp_client_reconnect_wait_defaults_to_five_seconds() {
    assert_eq!(TcpClient::DEFAULT_RECONNECT_WAIT, Duration::from_secs(5));
    let client = TcpClient::new("127.0.0.1:4242");
    assert_eq!(client.reconnect_wait(), TcpClient::DEFAULT_RECONNECT_WAIT);
    let short = TcpClient::new("127.0.0.1:4242").with_reconnect_wait(Duration::from_millis(100));
    assert_eq!(short.reconnect_wait(), Duration::from_millis(100));
}
