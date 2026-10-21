// Pins each table flush of a shared-instance drop (`RNS/Transport.py:3697-3702`) on its
// own: before the drop the path table, announce table, link table, tunnel table, held
// announces and the packet cache's reverse routes each provably hold something, and after
// it each is empty, with the duplicate-filter hashes kept. The data the shared instance
// sends over a link before the drop leaves the reverse route; the rest is seeded straight
// into the client's handler, since a client with transport off never fills those tables
// from the wire. Helpers come from the sibling `tests_parts` files included into this
// module.

/// Three announces inside one sample window trip the burst detector, so the fourth
/// unknown destination is held rather than admitted.
fn shared_instance_flush_rate_limit() -> AnnounceRateLimit {
    AnnounceRateLimit {
        incoming_freq_samples: 3,
        max_held_announces: 8,
        new_time: Duration::from_secs(3600),
        burst_freq_new: 100.0,
        burst_freq: 100.0,
        burst_hold: Duration::from_secs(60),
        burst_penalty: Duration::from_secs(60),
        held_release_interval: Duration::from_secs(60),
    }
}

fn shared_instance_flush_announce(destination: AddressHash) -> Packet {
    Packet {
        header: Header { packet_type: PacketType::Announce, hops: 1, ..Default::default() },
        destination,
        ..Default::default()
    }
}

/// The hashes of the cached packets that remember the interface they arrived on.
fn shared_instance_flush_noted_hashes(cache: &super::packet_cache::PacketCache) -> Vec<Hash> {
    cache.hashes().into_iter().filter(|hash| cache.source_iface_for_hash(hash).is_some()).collect()
}

/// Polls the client's packet cache until a Data packet received on `iface` has left its
/// source note, and returns the hashes of the packets that carry one.
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_flush_await_reverse_route(
    transport: &Transport,
    iface: AddressHash,
) -> Vec<Hash> {
    let packet_cache = transport.get_handler().lock().await.packet_cache.clone();
    let deadline = Instant::now() + LOOPBACK_TIMEOUT;
    loop {
        let noted = {
            let cache = packet_cache.lock().await;
            let noted = shared_instance_flush_noted_hashes(&cache);
            noted
                .iter()
                .any(|hash| cache.source_iface_for_hash(hash) == Some(iface))
                .then_some(noted)
        };
        if let Some(noted) = noted {
            return noted;
        }
        assert!(Instant::now() < deadline, "timed out waiting for the reverse route");
        tokio::time::sleep(LOOPBACK_POLL).await;
    }
}

/// The shared instance sends data over the link before it goes away, so the client's
/// packet cache routes a proof for that packet back over the shared-instance interface;
/// the other tables are seeded. The drop's debug record is the gate: it is written after
/// the last clear. Each table is then asserted empty on its own, and the cached hashes,
/// Python's `packet_hashlist`, are still there (the reconnect's tunnel packet adds to
/// them, so what was cached before the drop is asserted kept, not the exact set).
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_flush_empties_every_table_and_keeps_the_hashlist() {
    install_test_logger();
    let name = "shared-instance-flush";
    let server = shared_instance_server().await;
    let mut client = shared_instance_reconnect_client(name, &server).await;
    server.await_accepted_connections(1).await;
    shared_instance_assert_attached(&client.transport, client.iface).await;
    let (server_destination, link_id) =
        shared_instance_reconnect_open_link(&server, &mut client).await;
    let mut client_out_events = client.transport.out_link_events();

    let report = server
        .transport
        .send_to_in_links_with_report(&server_destination, b"data before the drop")
        .await;
    assert_sent_on_one_link(report, "inbound");
    let inbound = await_event(&mut client_out_events, |event| {
        event.id == link_id && matches!(event.event, LinkEvent::Data(_))
    })
    .await;
    assert_eq!(data_bytes(inbound), b"data before the drop");
    let noted = shared_instance_flush_await_reverse_route(&client.transport, client.iface).await;
    let proof_destinations: Vec<AddressHash> =
        noted.iter().map(AddressHash::new_from_hash).collect();

    let announced = AddressHash::new_from_rand(OsRng);
    let linked = AddressHash::new_from_rand(OsRng);
    let tunneled = AddressHash::new_from_rand(OsRng);
    let received_from = AddressHash::new_from_rand(OsRng);
    let now = std::time::Instant::now();
    let packet_cache = {
        let handler = client.transport.get_handler();
        let mut handler = handler.lock().await;
        handler.announce_table.add(
            &shared_instance_flush_announce(announced),
            announced,
            received_from,
        );
        let (queued, cached) = handler.announce_table.tier_sizes();
        assert_eq!(queued, 1, "the announce table's queue is seeded");
        assert!(cached >= 1, "the shared instance's announce is cached: {cached}");
        handler.link_table.add(
            &Packet { destination: linked, ..Packet::default() },
            linked,
            received_from,
            AddressHash::new_from_rand(OsRng),
            client.iface,
        );
        assert_eq!(handler.link_table.len(), 1, "the link table is seeded");
        let restored = handler.tunnel_table.restore_python_entries(
            vec![super::tunnels::PythonTunnelEntry {
                tunnel_id: Hash::new_from_slice(b"flush-tunnel"),
                interface_hash: None,
                paths: vec![super::tunnels::PythonTunnelPathEntry {
                    destination: tunneled,
                    timestamp_secs: 100.0,
                    received_from,
                    hops: 1,
                    expires_secs: 200.0,
                    random_blobs: vec![tunnel_restore_random_blob(7)],
                    interface_hash: None,
                    packet_hash: Hash::new_from_slice(b"flush-tunnel-packet"),
                }],
                expires_secs: 200.0,
            }],
            now,
            100.0,
        );
        assert_eq!(restored, 1, "the tunnel table is seeded");
        handler.announce_limits =
            AnnounceLimits::with_rate_limit(shared_instance_flush_rate_limit());
        let mut held = false;
        for index in 0..4u8 {
            let destination = AddressHash::new([0xF0 + index; crate::hash::ADDRESS_HASH_SIZE]);
            let action = handler.announce_limits.check(
                client.iface,
                &shared_instance_flush_announce(destination),
                crate::iface::IfaceSource::None,
                false,
            );
            held |= matches!(action, super::announce_limits::AnnounceLimitAction::Hold(_));
        }
        assert!(held, "an announce is held");
        handler.packet_cache.clone()
    };
    let hashes_before = {
        let cache = packet_cache.lock().await;
        for proof_destination in &proof_destinations {
            assert_eq!(
                cache.source_iface_for_proof_destination(proof_destination).map(|(_, iface)| iface),
                Some(client.iface),
                "a proof for the received data routes back over the shared instance"
            );
        }
        cache.hashes()
    };
    assert!(client.transport.has_path(&server_destination).await, "the path is learned");

    let (restarted, _restarted_announces, _) = shared_instance_reconnect_restart(server).await;
    wait_until(
        "the drop to be recorded",
        || !shared_instance_debug_records_containing(name, "disconnected,").is_empty(),
        || format!("{:?}", shared_instance_records_for(name)),
    )
    .await;

    assert!(!client.transport.has_path(&server_destination).await, "the path table is flushed");
    {
        let handler = client.transport.get_handler();
        let handler = handler.lock().await;
        assert_eq!(handler.announce_table.tier_sizes(), (0, 0), "the announce table is flushed");
        assert_eq!(handler.link_table.len(), 0, "the link table is flushed");
        assert!(
            handler.tunnel_table.export_python_entries(now, 100.0, |_| None).is_empty(),
            "the tunnel table is flushed"
        );
    }
    let held_after = {
        let handler = client.transport.get_handler();
        let mut handler = handler.lock().await;
        handler.announce_limits.clear_held_announces()
    };
    assert_eq!(held_after, 0, "the held announces are flushed");
    {
        let cache = packet_cache.lock().await;
        assert!(
            shared_instance_flush_noted_hashes(&cache).is_empty(),
            "no cached packet remembers its interface"
        );
        for proof_destination in &proof_destinations {
            assert_eq!(
                cache.source_iface_for_proof_destination(proof_destination),
                None,
                "the proof route is flushed"
            );
            assert_eq!(
                cache.proof_context_for_destination(proof_destination),
                None,
                "the proof context is flushed"
            );
        }
        let hashes_after = cache.hashes();
        for hash in &hashes_before {
            assert!(hashes_after.contains(hash), "the duplicate-filter hash {hash} is kept");
        }
    }
    let closed = await_event(&mut client_out_events, |event| {
        event.id == link_id && matches!(event.event, LinkEvent::Closed)
    })
    .await;
    assert_eq!(closed.id, link_id, "the link is torn down");
    shared_instance_assert_never_warned(name);

    timeout(STOP_TIMEOUT, client.transport.stop()).await.expect("client stop completes");
    timeout(STOP_TIMEOUT, restarted.transport.stop())
        .await
        .expect("restarted shared instance stop completes");
}
