impl Transport {
    pub async fn stop_interface(&self, address: AddressHash) -> bool {
        let routing = { self.handler.lock().await.multicast_peer_routings.remove(&address) };

        let virtual_ifaces = match routing {
            Some(routing) => {
                let virtual_ifaces = routing.lock().await.hashes();
                self.handler
                    .lock()
                    .await
                    .unicast_udp_ifaces
                    .retain(|_, (iface, _)| !virtual_ifaces.contains(iface));
                Some(virtual_ifaces)
            }
            None => None,
        };

        let Some(virtual_ifaces) = virtual_ifaces else {
            let peer_routings = {
                let mut handler = self.handler.lock().await;
                let stale_peer = handler
                    .unicast_udp_ifaces
                    .iter()
                    .find_map(|(peer, (iface, _))| (*iface == address).then_some(*peer));
                stale_peer.map(|peer| {
                    handler.unicast_udp_ifaces.remove(&peer);
                    handler.multicast_peer_routings.values().cloned().collect::<Vec<_>>()
                })
            };
            let Some(peer_routings) = peer_routings else {
                return self.iface_manager.lock().await.stop_interface(address);
            };
            for routing in peer_routings {
                let _ = routing.lock().await.remove_by_hash(&address);
            }
            return self.iface_manager.lock().await.stop_interface(address);
        };

        let mut manager = self.iface_manager.lock().await;
        let mut stopped = manager.stop_interface(address);
        for iface in virtual_ifaces {
            stopped |= manager.stop_interface(iface);
        }
        stopped
    }
}
