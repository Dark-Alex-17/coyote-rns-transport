// Pins the two release claims about the shared-instance client API that no other fixture
// covers: `remove_interface` is named beside `stop_interface` as a call that leaves the slot
// held until `detach_shared_instance`, and an interface spawned after a stop's interface
// sweep but before that stop takes the slot is detached by a second stop. Helpers come from
// the sibling `tests_parts` files included into this module.

/// Removing the attached interface through the generic `remove_interface` leaves the slot
/// and the posture set, exactly like `stop_interface`: a new attach is refused until
/// `detach_shared_instance` clears the stale slot, after which a fresh attach succeeds.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_removed_through_remove_interface_keeps_the_slot_until_detach() {
    let server = shared_instance_server().await;
    let client = shared_instance_client("shared-instance-remove-iface", &server).await;
    server.await_accepted_connections(1).await;
    shared_instance_assert_attached(&client.transport, client.iface).await;

    assert!(
        timeout(STOP_TIMEOUT, client.transport.remove_interface(client.iface))
            .await
            .expect("remove_interface completes"),
        "remove_interface finds the attached interface"
    );
    assert!(
        !client.transport.interface_hashes().await.contains(&client.iface),
        "the interface is gone"
    );
    assert_eq!(
        client.transport.shared_instance(),
        Some(client.iface),
        "remove_interface leaves the slot set"
    );
    assert!(
        client.transport.connected_to_shared_instance().await,
        "remove_interface leaves the client posture set"
    );
    let refused = client
        .transport
        .attach_shared_instance(server.endpoint(), SharedInstanceOptions::default())
        .await;
    assert!(
        matches!(refused, Err(RnsError::InvalidArgument)),
        "the stale slot still counts as held: {refused:?}"
    );
    assert_eq!(
        client.transport.interface_hashes().await.len(),
        0,
        "a refused attach spawns nothing"
    );

    let detached = timeout(STOP_TIMEOUT, client.transport.detach_shared_instance())
        .await
        .expect("detach completes on a stale slot");
    assert!(detached, "detach reports the stale slot");
    shared_instance_assert_released(&client.transport, client.iface).await;
    assert!(
        !timeout(STOP_TIMEOUT, client.transport.detach_shared_instance())
            .await
            .expect("second detach completes"),
        "nothing is left to detach"
    );

    let fresh = client
        .transport
        .attach_shared_instance(server.endpoint(), SharedInstanceOptions::default())
        .await
        .expect("a fresh attach succeeds once the stale slot is cleared");
    assert_ne!(fresh, client.iface, "the fresh attach spawns a new interface");
    server.await_accepted_connections(2).await;
    shared_instance_assert_attached(&client.transport, fresh).await;

    timeout(STOP_TIMEOUT, client.transport.stop()).await.expect("client stop completes");
    shared_instance_assert_released(&client.transport, fresh).await;
    timeout(STOP_TIMEOUT, server.transport.stop()).await.expect("shared instance stop completes");
}

/// The documented stop window, then the documented remedy. A stop sweeps the interfaces
/// first and takes the slot last; an attach whose first section runs in between leaves its
/// interface registered while the stop empties the slot and clears the posture. The handler
/// is held on a blocking thread until the attach has filled the slot, so the stop's slot
/// take runs against the fresh slot and the sweep has already passed. A second stop must
/// then detach that interface: afterwards the slot is empty, the posture is off, the
/// interface is gone and no longer flagged, and the shared instance saw exactly one client.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_second_stop_detaches_an_interface_spawned_inside_the_first_stop_window() {
    let server = shared_instance_server().await;
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let transport =
        Transport::new(TransportConfig::new("shared-instance-second-stop", &identity, false));
    let before = transport.interface_hashes().await.len();
    let (stall, release) = stall_transport_until_released(&transport).await;

    let ((), attached, slot_at_release) = tokio::join!(
        async {
            // stop's own 5 s budget can be consumed by the handler wait; let it elapse before
            // this guard fires.
            timeout(STOP_TIMEOUT + Duration::from_secs(3), transport.stop())
                .await
                .expect("the first stop returns within its budget")
        },
        async {
            // Let the stop run its interface sweep before the attach spawns anything.
            tokio::task::yield_now().await;
            timeout(
                STOP_TIMEOUT,
                transport
                    .attach_shared_instance(server.endpoint(), SharedInstanceOptions::default()),
            )
            .await
            .expect("the attach returns within its budget")
        },
        async {
            let slot = shared_instance_await_slot(&transport, true).await;
            release.send(()).expect("the stall thread is waiting for the release");
            slot
        },
    );
    stall.await.expect("the stall thread finishes once released");

    let iface = attached.expect("the attach spawned its interface and filled the slot");
    assert_eq!(slot_at_release, Some(iface), "the attach's first section filled the slot");
    assert_eq!(transport.shared_instance(), None, "the first stop emptied the slot");
    assert!(!transport.connected_to_shared_instance().await, "the first stop cleared the posture");
    assert!(
        transport.interface_hashes().await.contains(&iface),
        "the documented window: the interface spawned after the sweep stays registered"
    );
    server.await_accepted_connections(1).await;

    timeout(STOP_TIMEOUT, transport.stop()).await.expect("the second stop completes");

    assert_eq!(transport.shared_instance(), None, "the second stop leaves the slot empty");
    assert!(
        !transport.connected_to_shared_instance().await,
        "the second stop leaves the posture off"
    );
    assert!(
        !transport.interface_hashes().await.contains(&iface),
        "the second stop detached the interface left behind by the first"
    );
    assert!(!transport.is_shared_instance(&iface).await, "the detached iface is no longer flagged");
    assert_eq!(transport.interface_hashes().await.len(), before, "no interface is left behind");
    assert!(
        !timeout(STOP_TIMEOUT, transport.detach_shared_instance())
            .await
            .expect("detach completes after two stops"),
        "nothing is left to detach after the second stop"
    );
    assert_eq!(
        server.status.to_json()["accepted_connections"].as_u64(),
        Some(1),
        "the shared instance saw exactly the one client the window spawned"
    );
    timeout(STOP_TIMEOUT, server.transport.stop()).await.expect("shared instance stop completes");
}
