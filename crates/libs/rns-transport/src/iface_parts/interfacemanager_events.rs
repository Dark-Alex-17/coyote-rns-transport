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
}
