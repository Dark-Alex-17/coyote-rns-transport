impl InterfaceManager {
    /// Fork-only: installs the broadcast sender every channel this manager
    /// mints from now on publishes its `InterfaceEvent`s through; the
    /// transport calls it once, before the manager is shared, so no channel
    /// predates the sender. A channel minted before the first call keeps
    /// publishing nothing; a later call replaces the sender only for
    /// channels minted from then on.
    pub fn set_interface_events(
        &mut self,
        events: tokio::sync::broadcast::Sender<events::InterfaceEvent>,
    ) {
        self.interface_events = Some(events);
    }

    /// Fork-only: the number of streams the client on `address` has
    /// established since it was spawned; a virtual interface shares its
    /// host's count. Any other interface spawned here (listeners, datagram,
    /// serial and KISS carriers, the I2P interface) reads `Some(0)` for as
    /// long as it lives; only `TcpClient` and `LocalUnixClient` move the
    /// count. `None` once no record here carries the address: never minted,
    /// built with `InterfaceChannel::new`, or already stopped and pruned.
    pub fn connected_streams(&self, address: &AddressHash) -> Option<u64> {
        self.ifaces
            .iter()
            .find(|i| i.address == *address)
            .map(|i| i.connected_streams.load(Ordering::Acquire))
    }

    /// Fork-only: installs the peer counter `peer_count` reads for `host`
    /// and for every virtual child already registered on it; a child
    /// registered later copies it from the host. The AutoInterface runtime
    /// installs its discovery peer table's counter on its host channel; a
    /// caller may install one on a channel it minted here. Returns `false`,
    /// installing nothing, when no record carries `host`.
    pub fn set_peer_counter(&mut self, host: &AddressHash, counter: Arc<AtomicU64>) -> bool {
        let mut updated = false;
        for iface in &mut self.ifaces {
            if iface.address == *host || iface.parent == Some(*host) {
                iface.peers = Some(counter.clone());
                updated |= iface.address == *host;
            }
        }
        updated
    }

    /// Fork-only: the AutoInterface discovery peer table length behind
    /// `address`; a virtual child shares its host's counter. `None` both when
    /// no record here carries the address and when the record carries no
    /// counter, which is every interface kind other than an AutoInterface
    /// host and its children unless `set_peer_counter` installed one.
    pub fn peer_count(&self, address: &AddressHash) -> Option<u64> {
        self.ifaces
            .iter()
            .find(|i| i.address == *address)
            .and_then(|i| i.peers.as_ref().map(|peers| peers.load(Ordering::Acquire)))
    }
}
