// Coverage for the rationale behind the default of `SharedInstanceOptions`: two transports
// that attach to one shared instance under one identity. The tunnel id is the hash of the
// transport public key and the interface hash, and the interface hash comes from a
// per-transport counter, so two opted-in attaches under one identity that are each their
// transport's first interface synthesize one tunnel id and the shared instance holds one
// tunnel for both (`RNS/Transport.py:2820-2868`, the reappeared branch at `:2829`). Under
// the default neither synthesizes a tunnel, so the shared instance keeps a path per
// connection and a third client's links reach the transport that registered the
// destination. Helpers come from the sibling `tests_parts` files included into this
// module.

/// A client of the shared instance built under `identity` rather than a fresh one, its
/// announce receiver subscribed before the attach.
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_identity_client(
    name: &str,
    identity: &PrivateIdentity,
    server: &SharedInstanceServer,
    options: SharedInstanceOptions,
) -> SharedInstanceClient {
    let transport = Transport::new(TransportConfig::new(name, identity, false));
    let announces = transport.recv_announces().await;
    let iface = transport
        .attach_shared_instance(server.endpoint(), options)
        .await
        .expect("attach to the shared instance");
    SharedInstanceClient { transport, announces, iface }
}

/// The interface-hash counter is per transport, so two transports whose shared-instance
/// attach is their first interface carry the same full hash; with one identity that is what
/// makes the tunnel ids collide.
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_assert_same_interface_hash(
    a: &SharedInstanceClient,
    b: &SharedInstanceClient,
) {
    let a_hash = {
        let manager = a.transport.iface_manager();
        let manager = manager.lock().await;
        manager.full_hash(&a.iface)
    };
    let b_hash = {
        let manager = b.transport.iface_manager();
        let manager = manager.lock().await;
        manager.full_hash(&b.iface)
    };
    assert!(a_hash.is_some(), "a's attached interface is registered");
    assert_eq!(
        a_hash, b_hash,
        "both attaches are each transport's first interface, so the interface-hash counter agrees"
    );
}

/// Two default-option transports under one identity each register a destination; a third
/// client learns both at one hop and opens a link to each, and each link activates on the
/// transport that registered the destination, not on the newest connection. The shared
/// instance holds no tunnel throughout.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_tunnel_identity_default_keeps_two_transports_on_one_identity_apart() {
    let server = shared_instance_server().await;
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let a = shared_instance_identity_client(
        "shared-instance-tunnel-identity-a",
        &identity,
        &server,
        SharedInstanceOptions::default(),
    )
    .await;
    let mut c = shared_instance_client("shared-instance-tunnel-identity-c", &server).await;
    server.await_accepted_connections(2).await;
    shared_instance_assert_attached(&a.transport, a.iface).await;
    shared_instance_assert_attached(&c.transport, c.iface).await;
    let mut a_in_events = a.transport.in_link_events();

    let (_a_destination, a_desc) =
        register_in_destination(&a.transport, "shared-instance-tunnel-identity", "a").await;
    let (a_learned, hops) =
        shared_instance_learn_announce(&mut c.announces, &a_desc.address_hash).await;
    assert_eq!(hops, 1, "c learns a's destination at one hop");

    // The second transport under the same identity attaches after a's path is filed.
    let b = shared_instance_identity_client(
        "shared-instance-tunnel-identity-b",
        &identity,
        &server,
        SharedInstanceOptions::default(),
    )
    .await;
    server.await_accepted_connections(3).await;
    shared_instance_assert_attached(&b.transport, b.iface).await;
    shared_instance_assert_same_interface_hash(&a, &b).await;
    let mut b_in_events = b.transport.in_link_events();
    let (_b_destination, b_desc) =
        register_in_destination(&b.transport, "shared-instance-tunnel-identity", "b").await;
    assert_ne!(
        a_desc.address_hash, b_desc.address_hash,
        "the two transports hold distinct destinations"
    );
    let (b_learned, hops) =
        shared_instance_learn_announce(&mut c.announces, &b_desc.address_hash).await;
    assert_eq!(hops, 1, "c learns b's destination at one hop");
    assert_eq!(server.tunnel_count().await, 0, "neither default attach synthesized a tunnel");

    let (_link_to_a, link_id) = open_link(&c.transport, a_learned).await;
    let activated =
        await_event(&mut a_in_events, |event| matches!(event.event, LinkEvent::Activated)).await;
    assert_eq!(activated.id, link_id, "the link to a's destination activates on a");
    let (_link_to_b, link_id) = open_link(&c.transport, b_learned).await;
    let activated =
        await_event(&mut b_in_events, |event| matches!(event.event, LinkEvent::Activated)).await;
    assert_eq!(activated.id, link_id, "the link to b's destination activates on b");
    assert_eq!(server.tunnel_count().await, 0, "the links landed no tunnel either");

    timeout(STOP_TIMEOUT, a.transport.stop()).await.expect("client a stop completes");
    timeout(STOP_TIMEOUT, b.transport.stop()).await.expect("client b stop completes");
    timeout(STOP_TIMEOUT, c.transport.stop()).await.expect("client c stop completes");
    timeout(STOP_TIMEOUT, server.transport.stop()).await.expect("shared instance stop completes");
}

/// Two opted-in transports under one identity collide on the tunnel id: once both have
/// announced, the shared instance files both destinations under tunnels yet holds exactly
/// one tunnel, where two fresh identities would hold two.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_tunnel_identity_opted_in_transports_on_one_identity_share_one_tunnel() {
    let server = shared_instance_server().await;
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let mut a = shared_instance_identity_client(
        "shared-instance-tunnel-identity-on-a",
        &identity,
        &server,
        SharedInstanceOptions::default().with_tunnel_synthesis(true),
    )
    .await;
    let mut b = shared_instance_identity_client(
        "shared-instance-tunnel-identity-on-b",
        &identity,
        &server,
        SharedInstanceOptions::default().with_tunnel_synthesis(true),
    )
    .await;
    server.await_accepted_connections(2).await;
    shared_instance_assert_attached(&a.transport, a.iface).await;
    shared_instance_assert_attached(&b.transport, b.iface).await;
    shared_instance_assert_same_interface_hash(&a, &b).await;

    let (_a_destination, a_desc) =
        register_in_destination(&a.transport, "shared-instance-tunnel-identity-on", "a").await;
    let (_b_destination, b_desc) =
        register_in_destination(&b.transport, "shared-instance-tunnel-identity-on", "b").await;
    assert_ne!(
        a_desc.address_hash, b_desc.address_hash,
        "the two transports hold distinct destinations"
    );
    let (_, hops) = shared_instance_learn_announce(&mut b.announces, &a_desc.address_hash).await;
    assert_eq!(hops, 1, "b learns a's destination at one hop");
    let (_, hops) = shared_instance_learn_announce(&mut a.announces, &b_desc.address_hash).await;
    assert_eq!(hops, 1, "a learns b's destination at one hop");
    server.await_tunneled_destinations(vec![a_desc.address_hash, b_desc.address_hash]).await;
    assert_eq!(
        server.tunnel_count().await,
        1,
        "one identity and one per-transport interface counter make one tunnel id for both"
    );

    timeout(STOP_TIMEOUT, a.transport.stop()).await.expect("client a stop completes");
    timeout(STOP_TIMEOUT, b.transport.stop()).await.expect("client b stop completes");
    timeout(STOP_TIMEOUT, server.transport.stop()).await.expect("shared instance stop completes");
}
