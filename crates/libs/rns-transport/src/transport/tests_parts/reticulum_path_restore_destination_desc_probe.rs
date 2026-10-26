// Usage-pattern coverage for the second fill site `Transport::destination_desc` documents:
// a cached path table restored into a fresh transport yields the same `DestinationDesc`
// the announce produced, with the identity the announce carried and the truncated name
// hash the receiver stores, and the accessor agrees with `destination_identity` before and
// after the restore. Helpers come from the sibling `tests_parts` files included into this
// module.

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn destination_desc_is_filled_by_a_restored_path_table_with_the_announced_identity() {
    let temp = tempfile::tempdir().expect("tempdir");
    let local_identity = PrivateIdentity::new_from_rand(OsRng);
    let mut config = TransportConfig::new("test", &local_identity, true);
    config.set_retransmit(true);
    let transport = Transport::new(config);
    let iface = *transport.iface_manager().lock().await.new_channel(16).address();

    let remote_identity = PrivateIdentity::new_from_rand(OsRng);
    let mut remote_destination =
        SingleInputDestination::new(remote_identity, DestinationName::new("lxmf", "restored"));
    let announced = remote_destination.desc;
    let announce = remote_destination.announce(OsRng, None).expect("valid announce packet");
    assert_eq!(announce.destination, announced.address_hash);

    handle_announce(
        &announce,
        transport.get_handler().lock().await,
        iface,
        crate::iface::IfaceSource::None,
    )
    .await;
    let learned = transport
        .destination_desc(&announced.address_hash)
        .await
        .expect("the announce filled the destination map");
    assert_eq!(transport.save_reticulum_path_table(temp.path()).await.expect("save"), 1);

    let mut restored_config = TransportConfig::new("test", &local_identity, true);
    restored_config.set_retransmit(true);
    let restored = Transport::new(restored_config);
    let restored_iface = *restored.iface_manager().lock().await.new_channel(16).address();
    assert_eq!(restored_iface, iface, "test relies on deterministic iface hashes");

    // A fresh transport knows nothing until the cached table is restored; the two
    // accessors agree on that too.
    assert!(restored.destination_desc(&announced.address_hash).await.is_none());
    assert!(restored.destination_identity(&announced.address_hash).await.is_none());

    assert_eq!(restored.restore_reticulum_path_table(temp.path()).await.expect("restore"), 1);
    assert!(restored.has_path(&announced.address_hash).await, "the cached row restored a path");

    let desc = restored
        .destination_desc(&announced.address_hash)
        .await
        .expect("the restored path table filled the destination map");
    let identity = restored
        .destination_identity(&announced.address_hash)
        .await
        .expect("destination_identity is Some whenever destination_desc is Some");
    assert_eq!(desc.address_hash, announced.address_hash);
    assert_eq!(desc.identity.address_hash, announced.identity.address_hash);
    assert_eq!(desc.identity.public_key_bytes(), announced.identity.public_key_bytes());
    assert_eq!(desc.identity.verifying_key_bytes(), announced.identity.verifying_key_bytes());
    assert_eq!(identity.public_key_bytes(), desc.identity.public_key_bytes());
    assert_eq!(
        desc.name.as_name_hash_slice(),
        announced.name.as_name_hash_slice(),
        "the restored name carries the announce's truncated name hash"
    );
    assert_eq!(
        desc.name.hash.as_slice(),
        learned.name.hash.as_slice(),
        "the restored desc stores the same zero-padded name hash the announce did"
    );
    assert_ne!(
        desc.name.hash.as_slice(),
        announced.name.hash.as_slice(),
        "the stored name is the truncated hash zero-padded, not the full hash"
    );

    // The restored entry follows the same lifetime contract as a learned one: it outlives
    // `expire_path` and goes with `deregister_destination`.
    assert!(restored.expire_path(&announced.address_hash).await, "the restored path existed");
    assert!(restored.destination_desc(&announced.address_hash).await.is_some());
    assert!(restored.deregister_destination(&announced.address_hash).await);
    assert!(restored.destination_desc(&announced.address_hash).await.is_none());
    assert!(restored.destination_identity(&announced.address_hash).await.is_none());
}

/// The desc the accessor returns is the argument `Transport::link` takes, but the link
/// still needs a path: once `expire_path` has dropped the path the desc survives, and
/// without broadcast `link` refuses it at once with `NoPath` rather than letting the
/// pending link time out.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn destination_desc_outliving_expire_path_still_needs_a_path_to_link() {
    let local_identity = PrivateIdentity::new_from_rand(OsRng);
    let transport = Transport::new(TransportConfig::new("desc-no-path", &local_identity, false));
    let mut channel =
        transport.iface_manager().lock().await.new_channel_with_role(8, IfaceRole::Unicast);
    let iface = *channel.address();

    let remote_identity = PrivateIdentity::new_from_rand(OsRng);
    let mut remote_destination =
        SingleInputDestination::new(remote_identity, DestinationName::new("lxmf", "no-path"));
    let announce = remote_destination.announce(OsRng, None).expect("valid announce packet");
    handle_announce(
        &announce,
        transport.get_handler().lock().await,
        iface,
        crate::iface::IfaceSource::None,
    )
    .await;
    // Drain whatever the announce handling put on the wire so the link check below sees
    // only the link request, if any.
    while channel.tx_channel.try_recv().is_ok() {}

    assert!(transport.expire_path(&announce.destination).await, "the announce set a path");
    let desc = transport
        .destination_desc(&announce.destination)
        .await
        .expect("the desc outlives expire_path");
    assert_eq!(desc.address_hash, remote_destination.desc.address_hash);

    let started = std::time::Instant::now();
    let result = transport.link(desc).await;
    assert!(matches!(result, Err(RnsError::NoPath)), "a desc without a path is refused");
    assert!(started.elapsed() < Duration::from_millis(500), "took {:?}", started.elapsed());
    assert!(
        matches!(channel.tx_channel.try_recv(), Err(tokio::sync::mpsc::error::TryRecvError::Empty)),
        "no link request on the wire"
    );
}
