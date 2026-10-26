// Loopback coverage for `Transport::destination_desc`: the description the connector
// hands back for an announced destination carries the announce's truncated name hash,
// is absent for an unknown address, and outlives `expire_path`, which touches only the
// path table. Helpers come from the sibling `tests_parts` files included into this module.

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn destination_desc_tracks_the_announced_destination_and_survives_expire_path() {
    let listener = listener("loopback-listener", TcpClient::DEFAULT_MTU).await;
    let mut connector =
        connector("loopback-connector", listener.port, TcpClient::DEFAULT_MTU).await;
    listener.await_accepted_connections(1).await;

    let (_destination, announced) =
        register_in_destination(&listener.transport, "loopback", "destination-desc").await;
    learn_announce(&mut connector.announces, &announced.address_hash).await;

    let desc = connector
        .transport
        .destination_desc(&announced.address_hash)
        .await
        .expect("the connector learned the destination");
    assert_eq!(desc.address_hash, announced.address_hash);
    assert_eq!(desc.identity.address_hash, announced.identity.address_hash);
    assert_eq!(desc.identity.public_key_bytes(), announced.identity.public_key_bytes());
    assert_eq!(desc.identity.verifying_key_bytes(), announced.identity.verifying_key_bytes());
    assert_eq!(desc.name.as_name_hash_slice(), announced.name.as_name_hash_slice());
    assert_ne!(
        desc.name.hash.as_slice(),
        announced.name.hash.as_slice(),
        "the receiver stores the truncated name hash zero-padded, not the full hash"
    );

    let unknown = AddressHash::new_from_rand(OsRng);
    assert!(connector.transport.destination_desc(&unknown).await.is_none());
    assert!(connector.transport.destination_identity(&unknown).await.is_none());

    assert!(connector.transport.expire_path(&announced.address_hash).await, "the path existed");
    assert!(!connector.transport.has_path(&announced.address_hash).await);
    assert!(
        connector.transport.destination_desc(&announced.address_hash).await.is_some(),
        "only deregister_destination prunes the map"
    );
    assert!(
        connector.transport.destination_identity(&announced.address_hash).await.is_some(),
        "only deregister_destination prunes the map"
    );
}
