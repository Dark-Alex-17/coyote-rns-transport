impl TransportHandler {
    /// Drop virtual unicast ifaces that haven't seen a fresh announce
    /// from their peer in `UNICAST_IFACE_IDLE_TIMEOUT`. Also clears
    /// the corresponding entry from the host multicast iface's
    /// `PeerRouting`, so future packets from that peer are reattributed
    /// to the multicast iface (re-triggering a fresh virtual iface
    /// registration if the peer reappears). Called from
    /// `handle_cleanup`.
    pub(super) async fn gc_unicast_ifaces(&mut self) {
        let now = Instant::now();
        let stale: Vec<std::net::SocketAddr> = self
            .unicast_udp_ifaces
            .iter()
            .filter(|(_, (_, last_seen))| {
                now.duration_since(*last_seen) > UNICAST_IFACE_IDLE_TIMEOUT
            })
            .map(|(peer, _)| *peer)
            .collect();

        if stale.is_empty() {
            return;
        }

        for peer in stale {
            if let Some((iface_hash, _)) = self.unicast_udp_ifaces.remove(&peer) {
                let mut removed_from_routing = false;
                for routing in self.multicast_peer_routings.values() {
                    if routing.lock().await.remove_by_hash(&iface_hash).is_some() {
                        removed_from_routing = true;
                        break;
                    }
                }
                if !removed_from_routing {
                    log::debug!(
                        "tp({}): idle virtual UDP iface {} was absent from peer routing tables",
                        self.config.name,
                        iface_hash
                    );
                }
                if !self.iface_manager.lock().await.stop_interface(iface_hash) {
                    log::debug!(
                        "tp({}): idle virtual UDP iface {} was already stopped",
                        self.config.name,
                        iface_hash
                    );
                }
                log::debug!(
                    "tp({}): GC'd idle virtual UDP iface {} for peer {}",
                    self.config.name,
                    iface_hash,
                    peer,
                );
            }
        }
    }
}
