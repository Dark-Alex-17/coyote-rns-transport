// Pins the two release claims about the shared-instance client API that no other fixture
// covers: `remove_interface` and `stop_interface` both leave the slot held until
// `detach_shared_instance`, and an interface spawned after a stop's interface sweep but
// before that stop cancels the transport is detached by a second stop. Helpers come from
// the sibling `tests_parts` files included into this module.

/// An interface whose worker runs until the test releases it, so a stop's interface sweep
/// can be parked on the worker wait deterministically.
struct SharedInstanceHeldWorkerInterface;

impl crate::iface::Interface for SharedInstanceHeldWorkerInterface {
    fn mtu() -> usize {
        TcpClient::DEFAULT_MTU
    }
}

/// The generic interface call a test removes the attached interface through.
#[derive(Clone, Copy)]
enum SharedInstanceRemoval {
    RemoveInterface,
    StopInterface,
}

impl SharedInstanceRemoval {
    fn name(self) -> &'static str {
        match self {
            Self::RemoveInterface => "remove_interface",
            Self::StopInterface => "stop_interface",
        }
    }

    #[deny(clippy::await_holding_invalid_type)]
    async fn apply(self, transport: &Transport, iface: AddressHash) -> bool {
        match self {
            Self::RemoveInterface => transport.remove_interface(iface).await,
            Self::StopInterface => transport.stop_interface(iface).await,
        }
    }
}

/// Removing the attached interface through `removal` leaves the slot and the posture set:
/// a new attach is refused and spawns nothing until `detach_shared_instance` clears the
/// stale slot, after which a fresh attach succeeds.
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_removed_keeps_the_slot_until_detach(
    name: &str,
    removal: SharedInstanceRemoval,
) {
    let call = removal.name();
    let server = shared_instance_server().await;
    let client = shared_instance_client(name, &server).await;
    server.await_accepted_connections(1).await;
    shared_instance_assert_attached(&client.transport, client.iface).await;

    assert!(
        timeout(STOP_TIMEOUT, removal.apply(&client.transport, client.iface))
            .await
            .unwrap_or_else(|_| panic!("{call} completes")),
        "{call} finds the attached interface"
    );
    assert!(
        !client.transport.interface_hashes().await.contains(&client.iface),
        "the interface is gone"
    );
    assert_eq!(
        client.transport.shared_instance(),
        Some(client.iface),
        "{call} leaves the slot set"
    );
    assert!(
        client.transport.connected_to_shared_instance().await,
        "{call} leaves the client posture set"
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

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_removed_through_remove_interface_keeps_the_slot_until_detach() {
    shared_instance_removed_keeps_the_slot_until_detach(
        "shared-instance-remove-iface",
        SharedInstanceRemoval::RemoveInterface,
    )
    .await;
}

/// The documented stop window, then the documented remedy. A stop sweeps the interfaces
/// first, waits for their workers, then cancels the transport and takes the slot last; an
/// attach that runs while the stop waits for a worker is not yet refused and leaves its
/// interface registered while the stop empties the slot and clears the posture. The stop
/// is parked on a worker the test holds until the attach has returned, so the attach runs
/// in the window for certain. A second stop must then detach that interface: afterwards
/// the slot is empty, the posture is off, the interface is gone and no longer flagged, and
/// the shared instance saw exactly one client.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_second_stop_detaches_an_interface_spawned_inside_the_first_stop_window() {
    let server = shared_instance_server().await;
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let transport =
        Transport::new(TransportConfig::new("shared-instance-second-stop", &identity, false));
    let before = transport.interface_hashes().await.len();
    let (release, held) = tokio::sync::oneshot::channel::<()>();
    let held_worker = {
        transport.iface_manager().lock().await.spawn(
            SharedInstanceHeldWorkerInterface,
            |context| async move {
                held.await.expect("the test releases the held worker");
                drop(context);
            },
        )
    };

    let (attached, slot_at_release) =
        shared_instance_attach_inside_a_stop(&transport, &server, release).await;

    let iface = attached.expect("the attach inside the window spawned its interface");
    assert_eq!(slot_at_release, Some(iface), "the attach filled the slot before the stop took it");
    assert!(
        !transport.interface_hashes().await.contains(&held_worker),
        "the first stop's sweep detached the held worker's interface"
    );
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
