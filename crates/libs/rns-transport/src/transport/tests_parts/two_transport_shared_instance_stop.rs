// Pins what `stop` does to the shared-instance client API: an attach that starts once the
// transport is stopped is refused and spawns nothing, a stop that takes a held slot records
// the cleared slot at debug naming the interface, and a stop that finds no slot records
// nothing. Helpers come from the sibling `tests_parts` files included into this module.

/// The debug records for `transport_name` about a stop that cleared the slot.
fn shared_instance_stop_cleared_records(transport_name: &str) -> Vec<String> {
    shared_instance_debug_records_containing(transport_name, "stopped with shared instance")
}

/// Fork-only: a stopped transport has cancelled its management task, so an attach that
/// starts afterwards is refused with `InvalidArgument` before it spawns an interface. The
/// slot stays empty, the posture stays off, the manager registers nothing and the shared
/// instance never sees a connection. A stop with no slot to clear records nothing.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_stop_refuses_a_later_attach_and_spawns_nothing() {
    install_test_logger();
    let name = "shared-instance-stop-refuse";
    let server = shared_instance_server().await;
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let transport = Transport::new(TransportConfig::new(name, &identity, false));
    timeout(STOP_TIMEOUT, transport.stop()).await.expect("stop completes");

    let refused = timeout(
        STOP_TIMEOUT,
        transport.attach_shared_instance(server.endpoint(), SharedInstanceOptions::default()),
    )
    .await
    .expect("the refused attach returns within its budget");
    assert!(
        matches!(refused, Err(RnsError::InvalidArgument)),
        "an attach after stop is refused: {refused:?}"
    );
    assert_eq!(transport.shared_instance(), None, "the refused attach leaves the slot empty");
    assert!(
        !transport.connected_to_shared_instance().await,
        "the refused attach leaves the posture off"
    );
    assert!(transport.interface_hashes().await.is_empty(), "a refused attach spawns nothing");
    assert_eq!(
        server.status.to_json()["accepted_connections"].as_u64(),
        Some(0),
        "the shared instance saw no client"
    );
    assert!(
        shared_instance_stop_cleared_records(name).is_empty(),
        "a stop that found no slot records nothing"
    );
    shared_instance_assert_never_warned(name);

    assert!(
        !timeout(STOP_TIMEOUT, transport.detach_shared_instance())
            .await
            .expect("detach completes after the refused attach"),
        "nothing is left to detach"
    );
    timeout(STOP_TIMEOUT, server.transport.stop()).await.expect("shared instance stop completes");
}

/// Fork-only: a stop that takes the slot with the attached interface still in it records
/// that at debug, once, naming the interface and that the slot was cleared; the ordinary
/// detach line is not written, since no detach ran, and nothing is logged at warn.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_stop_logs_the_slot_it_clears_at_debug() {
    install_test_logger();
    let name = "shared-instance-stop-log";
    let server = shared_instance_server().await;
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let transport = Transport::new(TransportConfig::new(name, &identity, false));
    let iface = transport
        .attach_shared_instance(server.endpoint(), SharedInstanceOptions::default())
        .await
        .expect("attach to the shared instance");
    server.await_accepted_connections(1).await;
    shared_instance_assert_attached(&transport, iface).await;
    assert!(
        shared_instance_stop_cleared_records(name).is_empty(),
        "nothing about a cleared slot is recorded before the stop"
    );

    timeout(STOP_TIMEOUT, transport.stop()).await.expect("client stop completes");
    shared_instance_assert_released(&transport, iface).await;

    let cleared = shared_instance_stop_cleared_records(name);
    assert_eq!(cleared.len(), 1, "one cleared-slot line per stop that took the slot: {cleared:?}");
    let line = &cleared[0];
    assert!(line.contains(&format!("iface={iface}")), "names the interface: {line}");
    assert!(line.contains("slot cleared"), "says the slot was cleared: {line}");
    assert!(
        shared_instance_debug_records_containing(name, "detached shared instance").is_empty(),
        "stop writes no detach line"
    );
    shared_instance_assert_never_warned(name);

    timeout(STOP_TIMEOUT, transport.stop()).await.expect("second stop completes");
    assert_eq!(
        shared_instance_stop_cleared_records(name).len(),
        1,
        "a second stop finds the slot empty and records nothing more"
    );
    timeout(STOP_TIMEOUT, server.transport.stop()).await.expect("shared instance stop completes");
}
