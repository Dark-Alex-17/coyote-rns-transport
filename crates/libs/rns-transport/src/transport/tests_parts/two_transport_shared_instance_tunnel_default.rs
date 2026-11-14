// Coverage for the default of `SharedInstanceOptions`: a client that does not opt in with
// `with_tunnel_synthesis` synthesizes no tunnel, as Python's local client does not
// (`RNS/Interfaces/LocalInterface.py` has no tunnel code). Two such clients attached to one
// TCP shared instance still learn each other's announces at one hop and open links both
// ways, the shared instance's tunnel table stays empty before and after a restart, and the
// reconnect records that the synthesis is off. Helpers come from the sibling `tests_parts`
// files included into this module.

/// The number of tunnels, path-less ones included, the shared instance holds; `clear`
/// reports the pre-clear count, so an empty table reads zero and is left as it was. The
/// handler guard is held for the call alone.
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_tunnel_default_count(server: &SharedInstanceServer) -> usize {
    let handler = server.transport.get_handler();
    let mut handler = handler.lock().await;
    handler.tunnel_table.clear()
}

/// Two default-option clients exchange announces at one hop and open links both ways with
/// no tunnel in the shared instance's table; a restart of the shared instance is followed
/// by a reconnect that records the synthesis as off, re-announces both destinations and
/// lands no tunnel in the restarted instance's table either.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_tunnel_default_synthesizes_no_tunnel_at_attach_or_reconnect() {
    install_test_logger();
    let name_a = "shared-instance-tunnel-default-a";
    let name_b = "shared-instance-tunnel-default-b";
    let server = shared_instance_server().await;
    let mut a = shared_instance_reconnect_client(name_a, &server).await;
    let mut b = shared_instance_reconnect_client(name_b, &server).await;
    server.await_accepted_connections(2).await;
    shared_instance_assert_attached(&a.transport, a.iface).await;
    shared_instance_assert_attached(&b.transport, b.iface).await;
    let mut a_in_events = a.transport.in_link_events();
    let mut b_in_events = b.transport.in_link_events();

    let (_a_destination, a_desc) =
        register_in_destination(&a.transport, "shared-instance-tunnel-default", "a").await;
    let (_b_destination, b_desc) =
        register_in_destination(&b.transport, "shared-instance-tunnel-default", "b").await;
    let (a_learned, hops) =
        shared_instance_learn_announce(&mut b.announces, &a_desc.address_hash).await;
    assert_eq!(hops, 1, "b learns a's destination at one hop");
    let (b_learned, hops) =
        shared_instance_learn_announce(&mut a.announces, &b_desc.address_hash).await;
    assert_eq!(hops, 1, "a learns b's destination at one hop");
    assert_eq!(
        shared_instance_tunnel_default_count(&server).await,
        0,
        "neither attach synthesized a tunnel"
    );

    let (_link_to_a, link_id) = open_link(&b.transport, a_learned).await;
    let activated =
        await_event(&mut a_in_events, |event| matches!(event.event, LinkEvent::Activated)).await;
    assert_eq!(activated.id, link_id, "both ends share one link id");
    let (_link_to_b, link_id) = open_link(&a.transport, b_learned).await;
    let activated =
        await_event(&mut b_in_events, |event| matches!(event.event, LinkEvent::Activated)).await;
    assert_eq!(activated.id, link_id, "the reverse link shares one id too");

    // The reconnect helper waits for exactly one re-accept, which two clients redialing at
    // once can skip past inside one poll, so the restart is spelled out here.
    let port = server.port;
    timeout(STOP_TIMEOUT, server.transport.stop()).await.expect("shared instance stop completes");
    let restarted = shared_instance_server_on(port).await;
    let mut restarted_announces = restarted.transport.recv_announces().await;
    restarted.await_accepted_connections(2).await;

    for name in [name_a, name_b] {
        let record = shared_instance_probe10_await_debug_record(
            name,
            "reconnected, tunnel synthesis is off",
        )
        .await;
        assert!(record.contains("shared instance iface="), "the record names the iface: {record}");
        assert!(
            shared_instance_debug_records_containing(name, "tunnel re-synthesized").is_empty(),
            "a default-option reconnect re-synthesizes nothing: {:?}",
            shared_instance_records_for(name)
        );
    }
    shared_instance_reconnect_await_reannounces(
        &mut restarted_announces,
        vec![a_desc.address_hash, b_desc.address_hash],
    )
    .await;
    assert_eq!(
        shared_instance_tunnel_default_count(&restarted).await,
        0,
        "neither reconnect synthesized a tunnel"
    );

    timeout(STOP_TIMEOUT, a.transport.stop()).await.expect("client a stop completes");
    timeout(STOP_TIMEOUT, b.transport.stop()).await.expect("client b stop completes");
    timeout(STOP_TIMEOUT, restarted.transport.stop())
        .await
        .expect("restarted shared instance stop completes");
}
