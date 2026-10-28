// Usage-pattern coverage for `Transport::destination_desc` with more than one announced
// peer, the shape a real consumer sees: each address resolves to its own entry, the two
// accessors that share the lookup agree on the identity (not only on presence) for every
// address, and calling them from parallel tasks is safe and consistent. Helpers come from
// the sibling `tests_parts` files included into this module.

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn destination_desc_resolves_each_of_two_announced_peers_to_its_own_entry() {
    let listener = listener("loopback-listener", TcpClient::DEFAULT_MTU).await;
    // Two announces follow in quick succession, so the connector's ingress control is off.
    let mut connector = connector_with(
        "loopback-connector",
        listener.port,
        TcpClient::DEFAULT_MTU,
        Some(crate::iface::InterfaceSharedConfig {
            ingress_control: Some(false),
            ..Default::default()
        }),
    )
    .await;
    listener.await_accepted_connections(1).await;

    // Announce two distinct destinations one after the other so neither announce is
    // consumed while waiting for the other.
    let (_first_destination, first) =
        register_in_destination(&listener.transport, "loopback", "destination-desc-first").await;
    learn_announce(&mut connector.announces, &first.address_hash).await;
    let (_second_destination, second) =
        register_in_destination(&listener.transport, "loopback", "destination-desc-second").await;
    learn_announce(&mut connector.announces, &second.address_hash).await;
    assert_ne!(first.address_hash, second.address_hash);

    for announced in [&first, &second] {
        let desc = connector
            .transport
            .destination_desc(&announced.address_hash)
            .await
            .expect("the connector learned both destinations");
        let identity = connector
            .transport
            .destination_identity(&announced.address_hash)
            .await
            .expect("destination_identity is Some whenever destination_desc is Some");

        // The entry is the one keyed by the asked-for address, not another peer's.
        assert_eq!(desc.address_hash, announced.address_hash);
        assert_eq!(desc.identity.public_key_bytes(), announced.identity.public_key_bytes());
        assert_eq!(desc.name.as_name_hash_slice(), announced.name.as_name_hash_slice());

        // Both accessors read the same map entry, so they agree on the identity itself.
        assert_eq!(identity.address_hash, desc.identity.address_hash);
        assert_eq!(identity.public_key_bytes(), desc.identity.public_key_bytes());
        assert_eq!(identity.verifying_key_bytes(), desc.identity.verifying_key_bytes());
    }

    // Two peers announced under different names never collapse onto one entry.
    let first_desc =
        connector.transport.destination_desc(&first.address_hash).await.expect("first");
    let second_desc =
        connector.transport.destination_desc(&second.address_hash).await.expect("second");
    assert_ne!(first_desc.identity.public_key_bytes(), second_desc.identity.public_key_bytes());
    assert_ne!(first_desc.name.as_name_hash_slice(), second_desc.name.as_name_hash_slice());

    // Pruning one peer leaves the other, and the accessors keep agreeing on both.
    assert!(connector.transport.deregister_destination(&first.address_hash).await);
    assert!(connector.transport.destination_desc(&first.address_hash).await.is_none());
    assert!(connector.transport.destination_identity(&first.address_hash).await.is_none());
    assert!(connector.transport.destination_desc(&second.address_hash).await.is_some());
    assert!(connector.transport.destination_identity(&second.address_hash).await.is_some());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[deny(clippy::await_holding_invalid_type)]
async fn destination_desc_and_destination_identity_agree_when_called_concurrently() {
    let listener = listener("loopback-listener", TcpClient::DEFAULT_MTU).await;
    let mut connector =
        connector("loopback-connector", listener.port, TcpClient::DEFAULT_MTU).await;
    listener.await_accepted_connections(1).await;

    let (_destination, announced) =
        register_in_destination(&listener.transport, "loopback", "destination-desc-concurrent")
            .await;
    learn_announce(&mut connector.announces, &announced.address_hash).await;
    let known = announced.address_hash;
    let unknown = AddressHash::new_from_rand(OsRng);

    // Both accessors take the transport handler lock and then the entry's own lock; a
    // batch of callers on parallel tasks, spread over two worker threads so the calls
    // overlap, must neither deadlock nor disagree. The timeout turns a deadlock into a
    // failure instead of a hang.
    let LoopbackConnector { transport, .. } = connector;
    let transport = Arc::new(transport);
    let tasks: Vec<_> = (0..8)
        .map(|_| {
            let transport = Arc::clone(&transport);
            tokio::spawn(async move {
                let (desc, identity, missing_desc, missing_identity) = tokio::join!(
                    transport.destination_desc(&known),
                    transport.destination_identity(&known),
                    transport.destination_desc(&unknown),
                    transport.destination_identity(&unknown),
                );
                assert!(missing_desc.is_none(), "an unknown address has no desc");
                assert!(missing_identity.is_none(), "an unknown address has no identity");
                (
                    desc.expect("the connector learned the destination"),
                    identity.expect("destination_identity agrees with destination_desc"),
                )
            })
        })
        .collect();
    let batch = async {
        let mut results = Vec::new();
        for task in tasks {
            results.push(task.await.expect("accessor task completed"));
        }
        results
    };
    let results = timeout(LOOPBACK_TIMEOUT, batch)
        .await
        .expect("parallel accessor calls completed without deadlocking");

    for (desc, identity) in &results {
        assert_eq!(desc.address_hash, known);
        assert_eq!(identity.public_key_bytes(), desc.identity.public_key_bytes());
        assert_eq!(identity.verifying_key_bytes(), desc.identity.verifying_key_bytes());
        assert_eq!(desc.name.as_name_hash_slice(), announced.name.as_name_hash_slice());
    }
}
