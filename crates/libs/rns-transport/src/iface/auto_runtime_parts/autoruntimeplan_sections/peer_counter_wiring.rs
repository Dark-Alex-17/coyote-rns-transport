impl AutoRuntimePlan {
    /// Points the status handle (when one is given) and the transport
    /// bridge's host channel at the same peer counter the discovery state
    /// writes, so `AutoRuntimeStatusHandle::peer_count` and
    /// `InterfaceManager::peer_count` read the live peer table length.
    async fn wire_peer_counter(
        &self,
        state: &mut AutoDiscoveryState,
        runtime_status: Option<&AutoRuntimeStatusHandle>,
        transport_bridge: Option<&AutoInterfaceTransportBridge>,
    ) {
        let counter = match runtime_status {
            Some(handle) => handle.peer_counter(),
            None => state.peer_counter(),
        };
        state.set_peer_counter(Arc::clone(&counter));
        let Some(bridge) = transport_bridge else {
            return;
        };
        let installed = {
            let mut manager = bridge.iface_manager.lock().await;
            manager.set_peer_counter(&bridge.host_iface, counter)
        };
        if installed {
            log::debug!("[auto] peer counter installed on host iface={}", bridge.host_iface);
        } else {
            log::warn!(
                "[auto] peer counter not installed: host iface={} missing from manager",
                bridge.host_iface
            );
        }
    }
}
