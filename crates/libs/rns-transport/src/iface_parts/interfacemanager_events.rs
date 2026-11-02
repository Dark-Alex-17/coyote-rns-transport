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
    /// host's count. Any other interface spawned here (listeners, datagram
    /// and serial carriers, the I2P interface) reads `Some(0)` for as long
    /// as it lives; only `TcpClient` and `LocalUnixClient` move the count.
    /// `None` once no record here carries the address: never minted, built
    /// with `InterfaceChannel::new`, or already stopped and pruned.
    pub fn connected_streams(&self, address: &AddressHash) -> Option<u64> {
        self.ifaces
            .iter()
            .find(|i| i.address == *address)
            .map(|i| i.connected_streams.load(Ordering::Acquire))
    }
}
