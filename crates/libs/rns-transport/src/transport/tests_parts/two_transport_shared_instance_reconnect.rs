// Coverage for what a shared-instance client does when its connection drops and comes
// back: the drop tears down its links and flushes its tables, the reconnect re-synthesizes
// the tunnel and re-announces every registered destination as a path response after the
// reconnect wait plus two seconds, and a client that is detached, has dropped its connected
// posture or whose interface is no longer flagged as the shared instance flushes nothing.
// Helpers come from the sibling `tests_parts` files included into this module.

use crate::destination::link::LinkCloseReason;

const SHARED_INSTANCE_RECONNECT_WAIT: Duration = Duration::from_millis(100);

/// The re-announce is due `reconnect_wait + 2 s` after the reconnect; the rest is
/// scheduling margin. Only this upper bound is asserted.
const SHARED_INSTANCE_RECONNECT_REANNOUNCE_BOUND: Duration =
    Duration::from_millis(100 + 2_000 + 3_000);

/// Attaches with the short reconnect wait so the restart below is observed quickly.
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_reconnect_client(
    name: &str,
    server: &SharedInstanceServer,
) -> SharedInstanceClient {
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let transport = Transport::new(TransportConfig::new(name, &identity, false));
    let announces = transport.recv_announces().await;
    let iface = transport
        .attach_shared_instance(
            server.endpoint(),
            SharedInstanceOptions::default().with_reconnect_wait(SHARED_INSTANCE_RECONNECT_WAIT),
        )
        .await
        .expect("attach to the shared instance");
    SharedInstanceClient { transport, announces, iface }
}

/// Registers a destination on the client without announcing it from the test and waits
/// for the client's own registration announce to reach the shared instance, so that the
/// announces observed later are the reconnect's alone.
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_reconnect_register(
    client: &SharedInstanceClient,
    server_announces: &mut broadcast::Receiver<AnnounceEvent>,
    aspect: &str,
) -> AddressHash {
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let destination = client
        .transport
        .add_destination(identity, DestinationName::new("shared-instance-reconnect", aspect))
        .await;
    let address_hash = destination.lock().await.desc.address_hash;
    let event = shared_instance_parity_await_announce(server_announces, &address_hash).await;
    assert!(event.is_path_response, "the registration announce is a path response");
    address_hash
}

/// The shared instance registers and announces a destination, the client learns the path
/// and opens a link to it that both ends see activated. Returns the destination and the
/// link id.
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_reconnect_open_link(
    server: &SharedInstanceServer,
    client: &mut SharedInstanceClient,
) -> (AddressHash, AddressHash) {
    let mut server_in_events = server.transport.in_link_events();
    let (_destination, desc) =
        register_in_destination(&server.transport, "shared-instance-reconnect", "server").await;
    let (learned, _) =
        shared_instance_learn_announce(&mut client.announces, &desc.address_hash).await;
    assert!(client.transport.has_path(&desc.address_hash).await, "the client learned the path");
    let (_link, link_id) = open_link(&client.transport, learned).await;
    let activated = await_event(&mut server_in_events, |event| {
        event.id == link_id && matches!(event.event, LinkEvent::Activated)
    })
    .await;
    assert_eq!(activated.id, link_id, "both ends share one link id");
    (desc.address_hash, link_id)
}

/// Stops `server` and brings a fresh shared instance up on the same port; returns it with
/// its announce receiver (subscribed before any client can reconnect) and the instant its
/// listener accepted the client again.
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_reconnect_restart(
    server: SharedInstanceServer,
) -> (SharedInstanceServer, broadcast::Receiver<AnnounceEvent>, Instant) {
    let port = server.port;
    timeout(STOP_TIMEOUT, server.transport.stop()).await.expect("shared instance stop completes");
    let restarted = shared_instance_server_on(port).await;
    let announces = restarted.transport.recv_announces().await;
    restarted.await_accepted_connections(1).await;
    (restarted, announces, Instant::now())
}

/// Polls until the client no longer has a path to `destination`.
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_reconnect_await_path_gone(
    transport: &Transport,
    destination: &AddressHash,
) {
    let deadline = Instant::now() + LOOPBACK_TIMEOUT;
    while transport.has_path(destination).await {
        assert!(Instant::now() < deadline, "timed out waiting for the path to be flushed");
        tokio::time::sleep(LOOPBACK_POLL).await;
    }
}

/// Consumes announces until a path-response announce has arrived for every entry of
/// `expected`; returns the instant the last one arrived.
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_reconnect_await_reannounces(
    announces: &mut broadcast::Receiver<AnnounceEvent>,
    mut expected: Vec<AddressHash>,
) -> Instant {
    timeout(LOOPBACK_TIMEOUT, async {
        while !expected.is_empty() {
            match announces.recv().await {
                Ok(event) => {
                    let address_hash = event.destination.lock().await.desc.address_hash;
                    if event.is_path_response {
                        expected.retain(|hash| *hash != address_hash);
                    }
                }
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => panic!("announce channel closed"),
            }
        }
        Instant::now()
    })
    .await
    .expect("timed out waiting for the re-announces")
}

/// Asserts that no `Closed` event for `link_id` is waiting on `events`.
fn shared_instance_reconnect_assert_not_closed(
    events: &mut broadcast::Receiver<LinkEventData>,
    link_id: AddressHash,
) {
    while let Ok(event) = events.try_recv() {
        assert!(
            !(event.id == link_id && matches!(event.event, LinkEvent::Closed)),
            "the link was closed: {:?}",
            event.close_reason
        );
    }
}

/// The shared instance goes away and comes back on the same port. The drop flushes the
/// client's path to the shared instance's destination and closes the client's link with
/// the initiator-side reason while the attach stays in place; the reconnect re-announces
/// both client destinations as path responses within the bound and lands them in the new
/// shared instance's tunnel table.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_reconnect_flushes_on_drop_and_reannounces_on_return() {
    let server = shared_instance_server().await;
    let mut server_announces = server.transport.recv_announces().await;
    let mut client = shared_instance_reconnect_client("shared-instance-reconnect", &server).await;
    server.await_accepted_connections(1).await;
    shared_instance_assert_attached(&client.transport, client.iface).await;

    let first = shared_instance_reconnect_register(&client, &mut server_announces, "first").await;
    let second = shared_instance_reconnect_register(&client, &mut server_announces, "second").await;
    let (server_destination, link_id) =
        shared_instance_reconnect_open_link(&server, &mut client).await;
    let mut client_out_events = client.transport.out_link_events();

    let (restarted, mut restarted_announces, reaccepted_at) =
        shared_instance_reconnect_restart(server).await;

    shared_instance_reconnect_await_path_gone(&client.transport, &server_destination).await;
    let closed = await_event(&mut client_out_events, |event| {
        event.id == link_id && matches!(event.event, LinkEvent::Closed)
    })
    .await;
    assert_eq!(
        closed.close_reason,
        Some(LinkCloseReason::InitiatorClosed),
        "the initiator's link is torn down with the initiator-side reason"
    );
    assert!(
        client.transport.connected_to_shared_instance().await,
        "the connected posture survives the drop"
    );
    assert_eq!(client.transport.shared_instance(), Some(client.iface), "the slot keeps the iface");

    let last_reannounce_at =
        shared_instance_reconnect_await_reannounces(&mut restarted_announces, vec![first, second])
            .await;
    let elapsed = last_reannounce_at.saturating_duration_since(reaccepted_at);
    assert!(
        elapsed <= SHARED_INSTANCE_RECONNECT_REANNOUNCE_BOUND,
        "both re-announces arrive within {SHARED_INSTANCE_RECONNECT_REANNOUNCE_BOUND:?} of the re-accept, took {elapsed:?}"
    );
    restarted.await_tunneled_destinations(vec![first, second]).await;

    timeout(STOP_TIMEOUT, client.transport.stop()).await.expect("client stop completes");
    timeout(STOP_TIMEOUT, restarted.transport.stop())
        .await
        .expect("restarted shared instance stop completes");
}

/// A detach ends the client's stream without a disconnect event, so the path learned
/// through the shared instance and the link over it stay, both right after the detach and
/// once the shared instance has seen the stream end, when the client worker has exited and
/// its event channels have closed: a closed channel is not taken for a drop either.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_reconnect_detach_does_not_flush() {
    let server = shared_instance_server().await;
    let mut client =
        shared_instance_reconnect_client("shared-instance-reconnect-detach", &server).await;
    server.await_accepted_connections(1).await;
    let (server_destination, link_id) =
        shared_instance_reconnect_open_link(&server, &mut client).await;
    let mut client_out_events = client.transport.out_link_events();
    let server_ifaces_with_client = server.transport.interface_hashes().await.len();

    let detached = timeout(STOP_TIMEOUT, client.transport.detach_shared_instance())
        .await
        .expect("detach completes");
    assert!(detached, "detach reports the attached shared instance");
    assert!(client.transport.has_path(&server_destination).await, "the path survives the detach");
    shared_instance_reconnect_assert_not_closed(&mut client_out_events, link_id);

    let deadline = Instant::now() + LOOPBACK_TIMEOUT;
    while server.transport.interface_hashes().await.len() == server_ifaces_with_client {
        assert!(
            Instant::now() < deadline,
            "timed out waiting for the shared instance to drop the detached client's interface"
        );
        tokio::time::sleep(LOOPBACK_POLL).await;
    }
    timeout(STOP_TIMEOUT, client.transport.stop()).await.expect("client stop completes");
    assert!(
        client.transport.has_path(&server_destination).await,
        "the path survives the client's stream ending"
    );
    shared_instance_reconnect_assert_not_closed(&mut client_out_events, link_id);
    timeout(STOP_TIMEOUT, server.transport.stop()).await.expect("shared instance stop completes");
}

/// Python flushes only while `is_connected_to_shared_instance` holds
/// (`RNS/Interfaces/LocalInterface.py:259`): a client whose connected posture was dropped
/// keeps its path and link across a shared-instance restart.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_reconnect_dropped_posture_does_not_flush() {
    let server = shared_instance_server().await;
    let mut client =
        shared_instance_reconnect_client("shared-instance-reconnect-posture", &server).await;
    server.await_accepted_connections(1).await;
    let (server_destination, link_id) =
        shared_instance_reconnect_open_link(&server, &mut client).await;
    let mut client_out_events = client.transport.out_link_events();
    client.transport.set_connected_to_shared_instance(false).await;

    let (restarted, _restarted_announces, _) = shared_instance_reconnect_restart(server).await;

    assert!(
        client.transport.has_path(&server_destination).await,
        "the path survives the restart once the posture is dropped"
    );
    shared_instance_reconnect_assert_not_closed(&mut client_out_events, link_id);

    timeout(STOP_TIMEOUT, client.transport.stop()).await.expect("client stop completes");
    timeout(STOP_TIMEOUT, restarted.transport.stop())
        .await
        .expect("restarted shared instance stop completes");
}

/// The fork's `detached` (`RNS/Interfaces/LocalInterface.py:259`) is the interface no
/// longer being flagged as the shared instance: with the flag cleared, a restart flushes
/// nothing even though the connected posture is still set.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_reconnect_unflagged_iface_does_not_flush() {
    let server = shared_instance_server().await;
    let mut client =
        shared_instance_reconnect_client("shared-instance-reconnect-unflagged", &server).await;
    server.await_accepted_connections(1).await;
    let (server_destination, link_id) =
        shared_instance_reconnect_open_link(&server, &mut client).await;
    let mut client_out_events = client.transport.out_link_events();
    assert!(
        client.transport.iface_manager().lock().await.set_shared_instance(client.iface, false),
        "the attached iface is unflagged"
    );

    let (restarted, _restarted_announces, _) = shared_instance_reconnect_restart(server).await;

    assert!(
        client.transport.has_path(&server_destination).await,
        "the path survives the restart once the iface is unflagged"
    );
    shared_instance_reconnect_assert_not_closed(&mut client_out_events, link_id);

    timeout(STOP_TIMEOUT, client.transport.stop()).await.expect("client stop completes");
    timeout(STOP_TIMEOUT, restarted.transport.stop())
        .await
        .expect("restarted shared instance stop completes");
}

/// Each table's `clear` empties every collection it holds and reports how many entries
/// went, leaving the table usable.
#[test]
fn shared_instance_reconnect_table_clear_empties_every_collection() {
    let now = std::time::Instant::now();
    let destination = AddressHash::new_from_rand(OsRng);
    let other = AddressHash::new_from_rand(OsRng);
    let received_from = AddressHash::new_from_rand(OsRng);
    let iface = AddressHash::new_from_rand(OsRng);

    let mut path_table = super::path_table::PathTable::new();
    path_table.restore_python_entry(
        super::path_table::PythonPathEntry {
            destination,
            timestamp_secs: 100.0,
            received_from,
            hops: 1,
            expires_secs: 200.0,
            random_blobs: vec![tunnel_restore_random_blob(1)],
            iface,
            interface_hash: Hash::new_from_slice(b"iface"),
            packet_hash: Hash::new_from_slice(b"path-packet"),
        },
        now,
        100.0,
    );
    assert!(path_table.get(&destination).is_some());
    assert_eq!(path_table.clear(), 1);
    assert!(path_table.get(&destination).is_none());
    assert_eq!(path_table.clear(), 0);

    let mut announce_table = AnnounceTable::new(16, 1);
    let announce = Packet { destination, ..Packet::default() };
    announce_table.add(&announce, destination, received_from);
    announce_table.add_cached(
        &Packet { destination: other, ..Packet::default() },
        other,
        received_from,
    );
    assert!(announce_table.add_response(destination, iface, 1), "the queued announce answers");
    assert_eq!(announce_table.tier_sizes(), (1, 1));
    assert!(announce_table.pending_response_for_destination(&destination).is_some());
    assert_eq!(announce_table.clear(), 3, "queue, cache and response entries are all counted");
    assert_eq!(announce_table.tier_sizes(), (0, 0));
    assert!(announce_table.pending_response_for_destination(&destination).is_none());
    assert!(announce_table.cached_packet_for_destination(&other).is_none());
    announce_table.add(&announce, destination, received_from);
    assert_eq!(announce_table.tier_sizes(), (1, 0), "the table takes entries again");

    let mut link_table = LinkTable::new(Duration::from_secs(5), Duration::from_secs(30));
    link_table.add(
        &Packet { destination, ..Packet::default() },
        destination,
        received_from,
        other,
        iface,
    );
    assert_eq!(link_table.len(), 1);
    assert_eq!(link_table.clear(), 1);
    assert_eq!(link_table.len(), 0);

    let mut tunnel_table = super::tunnels::TunnelTable::new();
    let restored = tunnel_table.restore_python_entries(
        vec![super::tunnels::PythonTunnelEntry {
            tunnel_id: Hash::new_from_slice(b"tunnel"),
            interface_hash: None,
            paths: vec![super::tunnels::PythonTunnelPathEntry {
                destination,
                timestamp_secs: 100.0,
                received_from,
                hops: 1,
                expires_secs: 200.0,
                random_blobs: vec![tunnel_restore_random_blob(2)],
                interface_hash: None,
                packet_hash: Hash::new_from_slice(b"tunnel-packet"),
            }],
            expires_secs: 200.0,
        }],
        now,
        100.0,
    );
    assert_eq!(restored, 1);
    assert_eq!(tunnel_table.export_python_entries(now, 100.0, |_| None).len(), 1);
    assert_eq!(tunnel_table.clear(), 1);
    assert!(tunnel_table.export_python_entries(now, 100.0, |_| None).is_empty());
}
