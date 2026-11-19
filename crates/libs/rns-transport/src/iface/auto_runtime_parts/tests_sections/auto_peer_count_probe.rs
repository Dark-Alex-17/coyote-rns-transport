// Spawn-path peer counter behaviour beyond `auto_peer_count_pins.rs`: a spawn given a
// transport bridge but no status handle still installs the discovery state's own counter
// on the bridge's host channel (the `None => state.peer_counter()` arm of
// `wire_peer_counter`); a runtime with no adopted devices stays at zero; and the daemon
// path takes the count back down to `Some(0)` through the real peer job once the
// admitted peer times out, through the spawned runtime rather than a hand-driven
// `expire_stale_peers`. Reuses `loopback_runtime_plan` from `core_tests.rs` and
// `send_loopback_peering_token` and `wait_for_peer_count` from `auto_peer_count_pins.rs`.

#[cfg(unix)]
async fn wait_for_manager_peer_count(
    manager: &Arc<tokio::sync::Mutex<InterfaceManager>>,
    host: &AddressHash,
    expected: Option<u64>,
    deadline: core::time::Duration,
) {
    let until = Instant::now() + deadline;
    loop {
        let observed = manager.lock().await.peer_count(host);
        if observed == expected {
            return;
        }
        assert!(
            Instant::now() < until,
            "timed out waiting for manager.peer_count(host) == {expected:?}: last read {observed:?}"
        );
        tokio::time::sleep(core::time::Duration::from_millis(50)).await;
    }
}

/// With a bridge and no handle, the state's own counter is the one installed on the
/// host: `Some(0)` right after the spawn, `Some(1)` once a loopback peer is admitted.
#[cfg(unix)]
#[tokio::test]
async fn spawn_without_a_handle_installs_the_state_counter_on_the_bridge_host() {
    let (plan, discovery_port) = loopback_runtime_plan();
    let manager = Arc::new(tokio::sync::Mutex::new(InterfaceManager::new(8)));
    let channel = manager.lock().await.new_channel_with_role(8, IfaceRole::Multicast);
    let host = channel.address;
    let transport_runtime =
        AutoInterfaceTransportRuntime::from_channel(channel, Arc::clone(&manager));
    assert_eq!(manager.lock().await.peer_count(&host), None);

    let runtime = plan
        .spawn_discovery_runtime_with_native_scope_ids_and_transport(Some(transport_runtime), None)
        .await
        .expect("start loopback AutoInterface runtime with a bridge and no handle");
    assert_eq!(manager.lock().await.peer_count(&host), Some(0), "installed at spawn");

    send_loopback_peering_token(&plan, discovery_port).await;
    wait_for_manager_peer_count(&manager, &host, Some(1), core::time::Duration::from_secs(5)).await;

    runtime.stop().await;
}

/// A spawned runtime whose plan adopts no devices announces nothing, so nothing arrives
/// on its own discovery listener and the handle stays at 0 across one announce
/// interval.
#[cfg(unix)]
#[tokio::test]
async fn spawned_runtime_alone_reads_zero_through_an_announce_interval() {
    let (plan, _discovery_port) = loopback_runtime_plan();
    let handle = AutoRuntimeStatusHandle::from_startup_plan(&plan.startup_plan);
    let runtime = plan
        .spawn_discovery_runtime_with_native_scope_ids_and_transport(None, Some(handle.clone()))
        .await
        .expect("start loopback AutoInterface runtime");

    let timing = AutoInterfaceTiming::for_platform(plan.platform);
    let until = Instant::now() + timing.announce_interval + core::time::Duration::from_millis(500);
    while Instant::now() < until {
        assert_eq!(handle.peer_count(), 0, "a runtime with no adopted devices admits nobody");
        tokio::time::sleep(core::time::Duration::from_millis(100)).await;
    }

    runtime.stop().await;
}

/// The daemon path's expiry: after one admitted loopback peer stops announcing, the
/// peer job (every 4 s) expires it at the platform peering timeout (22 s), and both the
/// handle and the bridge's host channel read 0 again. Slow by construction (about 30 s);
/// the peering timeout is platform timing, not a plan field.
#[cfg(unix)]
#[tokio::test]
#[ignore = "slow: waits out the 22 s platform peering timeout"]
async fn spawned_runtime_counts_down_when_the_admitted_peer_times_out() {
    let (plan, discovery_port) = loopback_runtime_plan();
    let manager = Arc::new(tokio::sync::Mutex::new(InterfaceManager::new(8)));
    let channel = manager.lock().await.new_channel_with_role(8, IfaceRole::Multicast);
    let host = channel.address;
    let transport_runtime =
        AutoInterfaceTransportRuntime::from_channel(channel, Arc::clone(&manager));
    let handle = AutoRuntimeStatusHandle::from_startup_plan(&plan.startup_plan);
    let runtime = plan
        .spawn_discovery_runtime_with_native_scope_ids_and_transport(
            Some(transport_runtime),
            Some(handle.clone()),
        )
        .await
        .expect("start loopback AutoInterface runtime with a bridge");

    send_loopback_peering_token(&plan, discovery_port).await;
    wait_for_peer_count(&handle, 1).await;
    assert_eq!(manager.lock().await.peer_count(&host), Some(1));

    let timing = AutoInterfaceTiming::for_platform(plan.platform);
    let budget =
        timing.peering_timeout + timing.peer_job_interval * 2 + core::time::Duration::from_secs(5);
    wait_for_manager_peer_count(&manager, &host, Some(0), budget).await;
    assert_eq!(handle.peer_count(), 0, "the handle reads the same counter");

    runtime.stop().await;
}
