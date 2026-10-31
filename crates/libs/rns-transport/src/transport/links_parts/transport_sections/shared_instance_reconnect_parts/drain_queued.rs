// The post-exit drain of the shared-instance reconnect consumer, `include!`d
// into `transport_sections/shared_instance_reconnect.rs` so that section
// stays below the repository's 500-line limit; a file boundary, not a
// privacy one.

impl SharedInstanceReconnectConsumer {
    /// Serves what the exited worker left queued, every disconnect, then
    /// every connect, then every dial failure, the order the live arms
    /// keep; see [`Self::run`] for why the reconnects are left, and logs
    /// the counts at debug when it served anything.
    #[deny(clippy::await_holding_invalid_type)]
    async fn drain_queued(
        &self,
        disconnects: &mut mpsc::Receiver<AddressHash>,
        connects: &mut mpsc::Receiver<AddressHash>,
        dial_failures: &mut mpsc::Receiver<DialFailure>,
        state_events: Option<&mpsc::Sender<SharedInstanceEvent>>,
        first_connect: &mut Option<oneshot::Sender<()>>,
    ) {
        let mut disconnects_served = 0usize;
        while disconnects.try_recv().is_ok() {
            self.serve_disconnected(state_events).await;
            disconnects_served += 1;
        }
        let mut connects_served = 0usize;
        while connects.try_recv().is_ok() {
            self.serve_connected(state_events, first_connect);
            connects_served += 1;
        }
        let mut dial_failures_served = 0usize;
        while let Ok(failed) = dial_failures.try_recv() {
            self.serve_dial_failed(state_events, failed);
            dial_failures_served += 1;
        }
        if disconnects_served + connects_served + dial_failures_served > 0 {
            log::debug!(
                "tp({}): shared instance iface={} drained {} disconnect(s), {} connect(s), {} dial failure(s) after the worker exited",
                self.name,
                self.iface,
                disconnects_served,
                connects_served,
                dial_failures_served
            );
        }
    }
}
