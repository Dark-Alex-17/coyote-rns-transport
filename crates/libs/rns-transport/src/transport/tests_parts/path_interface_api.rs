// Loopback coverage for `Transport::path_interface`: the record agrees with
// `path_status`, `hops_to` and `path_table` on the same destination, `learned_at`
// lies within a second of the announce, `name` follows the interface manager's
// display name, and an unknown or expired destination answers `None`. The two
// restore cases follow: a disk restore keeps the persisted row's own timestamp,
// a tunnel restore stamps the restore moment. Helpers come from the sibling
// `tests_parts` files included into this module.

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn path_interface_reports_the_learned_interface_name_time_and_hops() {
    let listener = listener("loopback-listener", TcpClient::DEFAULT_MTU).await;
    let mut connector =
        connector("loopback-connector", listener.port, TcpClient::DEFAULT_MTU).await;
    listener.await_accepted_connections(1).await;

    let before = std::time::SystemTime::now();
    let (_destination, desc) =
        register_in_destination(&listener.transport, "loopback", "path-interface").await;
    await_announce_for(&mut connector.announces, &desc.address_hash).await;
    let after = std::time::SystemTime::now();
    let hash = desc.address_hash;

    let path = connector.transport.path_interface(&hash).await.expect("the path is on file");
    let status = connector.transport.path_status(&hash).await;
    assert_eq!(path.iface, status.interface.expect("path_status reports the interface"));
    assert_eq!(path.hops, connector.transport.hops_to(&hash).await);
    assert_eq!(path.hops, 1, "the announce came straight from the listener");
    let second = Duration::from_secs(1);
    assert!(
        before - second <= path.learned_at && path.learned_at <= after + second,
        "learned_at {:?} lies within a second of the announce bracket {before:?}..={after:?}",
        path.learned_at
    );
    assert_eq!(path.name, None, "the TcpClient has no display name");

    assert!(connector
        .transport
        .iface_manager()
        .lock()
        .await
        .set_display_name(path.iface, Some("uplink".to_owned())));
    tokio::time::sleep(Duration::from_millis(50)).await;
    let named = connector.transport.path_interface(&hash).await.expect("the path is still on file");
    assert_eq!(named.name.as_deref(), Some("uplink"));
    assert_eq!(named.iface, path.iface);
    // Two reads of one entry convert the same `Instant` through two clock samples,
    // so they agree to the sampling imprecision, not exactly; the sleep makes a
    // relearn visible as a shift of its own size.
    let drift = named
        .learned_at
        .duration_since(path.learned_at)
        .unwrap_or_else(|earlier| earlier.duration());
    assert!(
        drift < Duration::from_millis(25),
        "two reads of one entry agree to the clock-sampling imprecision; a relearn would move learned_at by the 50 ms sleep: the reads differ by {drift:?}"
    );

    let rows = connector.transport.path_table(None).await.expect("path table");
    let row = rows.iter().find(|row| row.destination == hash).expect("the table lists the path");
    assert_eq!(row.interface, named.iface);
    assert_eq!(row.interface_name, named.name);
    assert_eq!(row.hops, named.hops);

    assert_eq!(
        connector.transport.path_interface(&AddressHash::new_from_rand(OsRng)).await,
        None,
        "no path table entry, no interface"
    );
    assert!(connector.transport.expire_path(&hash).await, "the path table entry is removed");
    assert_eq!(connector.transport.path_interface(&hash).await, None);
}

fn within_a_second(actual: std::time::SystemTime, expected: std::time::SystemTime) -> bool {
    let second = Duration::from_secs(1);
    expected - second <= actual && actual <= expected + second
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn path_interface_learned_at_keeps_a_disk_restored_rows_own_timestamp() {
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let transport =
        Transport::new(TransportConfig::new("path-interface-restore", &identity, true));
    let destination = AddressHash::new_from_hash(&Hash::new_from_slice(b"destination"));
    let received_from = AddressHash::new_from_hash(&Hash::new_from_slice(b"next-hop"));
    let iface = *transport.iface_manager().lock().await.new_channel(16).address();
    let now_unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("the clock is past the epoch")
        .as_secs_f64();
    transport.get_handler().lock().await.path_table.restore_python_entry(
        super::path_table::PythonPathEntry {
            destination,
            timestamp_secs: now_unix - 3600.0,
            received_from,
            hops: 2,
            expires_secs: now_unix + 3600.0,
            random_blobs: vec![],
            iface,
            interface_hash: Hash::new_from_slice(b"iface"),
            packet_hash: Hash::new_from_slice(b"packet"),
        },
        std::time::Instant::now(),
        now_unix,
    );

    let path = transport.path_interface(&destination).await.expect("the restored path is on file");
    assert_eq!(path.hops, 2);
    assert_eq!(path.iface, iface);
    let an_hour_ago = std::time::SystemTime::now() - Duration::from_secs(3600);
    assert!(
        within_a_second(path.learned_at, an_hour_ago),
        "learned_at {:?} is the persisted row's hour-old timestamp, about {an_hour_ago:?}",
        path.learned_at
    );
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn path_interface_learned_at_is_the_restore_moment_for_a_tunnel_restore() {
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let transport =
        Transport::new(TransportConfig::new("path-interface-restore", &identity, true));
    let destination = AddressHash::new_from_hash(&Hash::new_from_slice(b"destination"));
    let next_hop = AddressHash::new_from_hash(&Hash::new_from_slice(b"next-hop"));
    let iface = *transport.iface_manager().lock().await.new_channel(16).address();
    assert!(transport.get_handler().lock().await.path_table.restore_tunnel_path(
        destination,
        next_hop,
        1,
        iface,
        Hash::new_from_slice(b"packet"),
        std::time::Instant::now(),
    ));

    let path = transport.path_interface(&destination).await.expect("the restored path is on file");
    assert_eq!(path.hops, 1);
    assert_eq!(path.iface, iface);
    let now = std::time::SystemTime::now();
    assert!(
        within_a_second(path.learned_at, now),
        "learned_at {:?} is the restore moment, about {now:?}",
        path.learned_at
    );
}
