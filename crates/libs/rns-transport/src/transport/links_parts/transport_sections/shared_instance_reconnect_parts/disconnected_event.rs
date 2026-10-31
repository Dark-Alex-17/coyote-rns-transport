// The disconnect event an attach reads, `include!`d into
// `transport_sections/shared_instance_reconnect.rs` so that section stays
// below the repository's 500-line limit; a file boundary, not a privacy one.

impl SharedInstanceReconnectConsumer {
    /// The event a served disconnect sends before its `Connecting`:
    /// `DisconnectedWithReason` for an attach that opted in with
    /// `SharedInstanceOptions::with_disconnect_reasons`, else the plain
    /// `Disconnected` and `reason` is dropped.
    fn disconnected_event(&self, reason: DisconnectReason) -> SharedInstanceEvent {
        if self.disconnect_reasons {
            SharedInstanceEvent::DisconnectedWithReason { reason }
        } else {
            SharedInstanceEvent::Disconnected
        }
    }
}
