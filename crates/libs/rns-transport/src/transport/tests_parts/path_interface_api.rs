// Loopback coverage for `Transport::path_interface`: the record agrees with
// `path_status`, `hops_to` and `path_table` on the same destination, `learned_at`
// lies within a second of the announce, `name` follows the interface manager's
// display name, and an unknown or expired destination answers `None`. Helpers come
// from the sibling `tests_parts` files included into this module.

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
    let named = connector.transport.path_interface(&hash).await.expect("the path is still on file");
    assert_eq!(named.name.as_deref(), Some("uplink"));
    assert_eq!(named.iface, path.iface);
    // Two reads of one entry convert the same `Instant` through two clock samples,
    // so they agree to the conversion's sub-millisecond imprecision, not exactly.
    let drift = named
        .learned_at
        .duration_since(path.learned_at)
        .unwrap_or_else(|earlier| earlier.duration());
    assert!(
        drift < Duration::from_millis(1),
        "naming the interface does not relearn the path: the reads differ by {drift:?}"
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
