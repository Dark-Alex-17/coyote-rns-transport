// Usage-pattern probes for the shared-instance client API, third set: the window between
// the two sections of `attach_shared_instance` hit deterministically rather than by a
// scheduler race (a detach, then a stop, already queued on the handler when the attach
// writes its slot), and a run of attach/detach cycles that must leave the client transport
// with exactly the interfaces it started with. Helpers come from the sibling `tests_parts`
// files included into this module.

/// How many attach/detach rounds the cycle probe runs.
const SHARED_INSTANCE_PROBE3_CYCLES: u64 = 3;

/// Polls the slot without touching the handler until it is held (`held == true`) or empty,
/// and returns what it held last.
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_await_slot(transport: &Transport, held: bool) -> Option<AddressHash> {
    let deadline = Instant::now() + LOOPBACK_TIMEOUT;
    loop {
        let slot = transport.shared_instance();
        if slot.is_some() == held {
            return slot;
        }
        assert!(Instant::now() < deadline, "timed out waiting for the slot to be held={held}");
        tokio::time::sleep(LOOPBACK_POLL).await;
    }
}

/// A detach that is already waiting on the handler when an attach writes its slot: the
/// attach's first section spawns the interface and fills the slot, the detach then takes
/// that fresh slot, clears the posture and removes the interface, and the attach's second
/// section must leave the posture off because the slot no longer names its interface. The
/// handler is held on a blocking thread so the detach queues ahead of the attach's second
/// section; the attach's first section only needs the interface manager.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_detach_queued_before_the_attach_posture_write_wins_the_slot() {
    let server = shared_instance_server().await;
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let transport =
        Transport::new(TransportConfig::new("shared-instance-window", &identity, false));
    let before = transport.interface_hashes().await.len();
    let (stall, release) = stall_transport_until_released(&transport).await;

    let (detached, attached, slot_at_release) = tokio::join!(
        async {
            timeout(STOP_TIMEOUT, transport.detach_shared_instance())
                .await
                .expect("the detach returns within its budget")
        },
        async {
            // Let the detach register as the first waiter on the handler before the attach
            // runs its first section.
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
    assert!(detached, "the detach consumed the slot the attach had just written");
    assert_eq!(transport.shared_instance(), None, "the detach emptied the slot");
    assert!(
        !transport.connected_to_shared_instance().await,
        "the attach's second section must not set the posture once the slot is gone"
    );
    assert!(
        !transport.interface_hashes().await.contains(&iface),
        "the detach removed the interface the attach had spawned"
    );
    assert!(!transport.is_shared_instance(&iface).await);
    assert_eq!(transport.interface_hashes().await.len(), before, "no interface is left behind");

    let fresh = transport
        .attach_shared_instance(server.endpoint(), SharedInstanceOptions::default())
        .await
        .expect("a fresh attach succeeds after the lost race");
    assert_ne!(fresh, iface);
    shared_instance_assert_attached(&transport, fresh).await;
    assert!(timeout(STOP_TIMEOUT, transport.detach_shared_instance()).await.expect("detach"));
    shared_instance_assert_released(&transport, fresh).await;
    timeout(STOP_TIMEOUT, server.transport.stop()).await.expect("shared instance stop completes");
}

/// A stop that is already under way when an attach writes its slot: whatever the stop
/// manages to tear down, the slot and the posture must agree afterwards and a held slot must
/// name a registered interface. The handler is held on a blocking thread until the attach's
/// first section has filled the slot, so the stop's slot take runs against the fresh slot.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_stop_racing_an_attach_keeps_slot_and_posture_consistent() {
    let server = shared_instance_server().await;
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let transport =
        Transport::new(TransportConfig::new("shared-instance-stop-window", &identity, false));
    let (stall, release) = stall_transport_until_released(&transport).await;

    let ((), attached, slot_at_release) = tokio::join!(
        async {
            // stop's own 5 s budget can be consumed by the handler wait; let it elapse before
            // this guard fires.
            timeout(STOP_TIMEOUT + Duration::from_secs(3), transport.stop())
                .await
                .expect("stop returns within its budget")
        },
        async {
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
    let held = transport.shared_instance();
    assert_eq!(
        held.is_some(),
        transport.connected_to_shared_instance().await,
        "the slot and the posture disagree after stop raced the attach, slot={held:?}"
    );
    if let Some(held) = held {
        assert!(
            transport.interface_hashes().await.contains(&held),
            "a held slot names an interface that is gone"
        );
    }
    timeout(STOP_TIMEOUT, server.transport.stop()).await.expect("shared instance stop completes");
}

/// Repeated attach/detach rounds against one shared instance: every round is accepted by
/// the shared instance, flagged and released like the first, and the client transport ends
/// each round with exactly the interfaces it started with.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_attach_detach_cycles_leave_no_interface_behind() {
    let server = shared_instance_server().await;
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let transport =
        Transport::new(TransportConfig::new("shared-instance-cycles", &identity, false));
    let before = transport.interface_hashes().await.len();
    let mut previous = None;

    for round in 1..=SHARED_INSTANCE_PROBE3_CYCLES {
        let iface = transport
            .attach_shared_instance(server.endpoint(), SharedInstanceOptions::default())
            .await
            .unwrap_or_else(|err| panic!("round {round}: attach succeeds: {err:?}"));
        assert_ne!(Some(iface), previous, "round {round}: a fresh interface is spawned");
        server.await_accepted_connections(round).await;
        shared_instance_assert_attached(&transport, iface).await;
        assert_eq!(
            transport.interface_hashes().await.len(),
            before + 1,
            "round {round}: exactly one interface was added"
        );

        let detached = timeout(STOP_TIMEOUT, transport.detach_shared_instance())
            .await
            .unwrap_or_else(|_| panic!("round {round}: detach completes"));
        assert!(detached, "round {round}: detach reports the attached shared instance");
        shared_instance_assert_released(&transport, iface).await;
        assert_eq!(
            transport.interface_hashes().await.len(),
            before,
            "round {round}: the client is back to its starting interfaces"
        );
        previous = Some(iface);
    }

    timeout(STOP_TIMEOUT, transport.stop()).await.expect("client stop completes");
    assert_eq!(transport.shared_instance(), None);
    assert!(!transport.connected_to_shared_instance().await);
    timeout(STOP_TIMEOUT, server.transport.stop()).await.expect("shared instance stop completes");
}
