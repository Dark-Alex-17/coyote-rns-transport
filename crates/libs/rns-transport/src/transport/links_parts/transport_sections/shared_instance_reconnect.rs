use tokio::sync::mpsc;

/// Python's shared-instance client re-announces `RECONNECT_WAIT + 2` seconds
/// after it reconnects (`RNS/Interfaces/LocalInterface.py:177-180`); this
/// is the 2, added to the attach's `reconnect_wait`.
const SHARED_INSTANCE_REAPPEARED_ANNOUNCE_GRACE: Duration = Duration::from_secs(2);

impl Transport {
    /// Spawns the task that reacts to the attached client's reconnect
    /// events; see [`SharedInstanceReconnectConsumer::run`].
    fn spawn_shared_instance_reconnect_consumer(
        &self,
        iface: AddressHash,
        reconnect_wait: Duration,
        reconnects: mpsc::Receiver<AddressHash>,
    ) {
        let consumer = SharedInstanceReconnectConsumer {
            handler: self.handler.clone(),
            iface_manager: self.iface_manager.clone(),
            cancel: self.cancel.clone(),
            name: self.name.clone(),
            iface,
            reconnect_wait,
        };
        tokio::spawn(consumer.run(reconnects));
    }
}

/// Resynthesizes the tunnel on `iface` and returns whether the interface
/// was still registered. The identity is copied out under the handler
/// guard and the interface hash read under the iface_manager guard, one
/// after the other, before the send takes the handler again.
#[deny(clippy::await_holding_invalid_type)]
pub(crate) async fn synthesize_tunnel_on_iface(
    handler: &Arc<Mutex<TransportHandler>>,
    iface: AddressHash,
) -> bool {
    let (identity, iface_manager) = {
        let handler = handler.lock().await;
        (handler.config.identity.clone(), handler.iface_manager.clone())
    };
    let Some(interface_hash) = iface_manager.lock().await.full_hash(&iface) else {
        return false;
    };
    let packet = super::tunnels::synthesize_tunnel_packet(&identity, interface_hash);
    super::request_dispatch::send_on_iface(handler, iface, packet).await;
    true
}

/// What one attach's consumer task carries: the task outlives the attach
/// call, so it holds clones of the transport's parts rather than a borrow.
#[derive(Clone)]
struct SharedInstanceReconnectConsumer {
    handler: Arc<Mutex<TransportHandler>>,
    iface_manager: Arc<Mutex<InterfaceManager>>,
    cancel: CancellationToken,
    name: String,
    iface: AddressHash,
    reconnect_wait: Duration,
}

impl SharedInstanceReconnectConsumer {
    /// Serves the client's events until the transport is cancelled or the
    /// client worker exits. The sender belongs to that one worker and drops
    /// with it, after a detach, `stop_interface` or `stop`, which closes the
    /// channel and ends the task.
    #[deny(clippy::await_holding_invalid_type)]
    async fn run(self, mut reconnects: mpsc::Receiver<AddressHash>) {
        loop {
            tokio::select! {
                _ = self.cancel.cancelled() => break,
                reconnected = reconnects.recv() => {
                    if reconnected.is_none() {
                        break;
                    }
                    self.on_reconnected().await;
                }
            }
        }
    }

    /// Python's `is_connected_to_shared_instance` guard
    /// (`RNS/Transport.py:3706`) together with the fork's equivalent of its
    /// `not self.detached` (`RNS/Interfaces/LocalInterface.py:259`): the
    /// interface is still the one flagged as the shared instance. The two
    /// guards are taken one after the other, never together.
    #[deny(clippy::await_holding_invalid_type)]
    async fn attached(&self) -> bool {
        let connected = self.handler.lock().await.config.connected_to_shared_instance;
        connected && self.iface_manager.lock().await.is_shared_instance(&self.iface)
    }

    /// The tunnel goes out at once, as reticulumd does for its own clients;
    /// the re-announce waits like Python's reconnect job
    /// (`RNS/Interfaces/LocalInterface.py:177-180`).
    #[deny(clippy::await_holding_invalid_type)]
    async fn on_reconnected(&self) {
        if synthesize_tunnel_on_iface(&self.handler, self.iface).await {
            log::debug!(
                "tp({}): shared instance iface={} reconnected, tunnel re-synthesized",
                self.name,
                self.iface
            );
        } else {
            log::debug!(
                "tp({}): shared instance iface={} reconnected but no longer registered, tunnel not re-synthesized",
                self.name,
                self.iface
            );
        }
        let consumer = self.clone();
        tokio::spawn(async move {
            tokio::select! {
                _ = consumer.cancel.cancelled() => {}
                _ = tokio::time::sleep(
                    consumer.reconnect_wait + SHARED_INSTANCE_REAPPEARED_ANNOUNCE_GRACE
                ) => consumer.announce_reappeared().await,
            }
        });
    }

    /// `shared_connection_reappeared` (`RNS/Transport.py:3705-3709`): every
    /// registered single destination is announced as a path response, if
    /// the transport is still attached when the delay has elapsed.
    #[deny(clippy::await_holding_invalid_type)]
    async fn announce_reappeared(&self) {
        if !self.attached().await {
            log::debug!(
                "tp({}): shared instance iface={} detached before the reappearance delay elapsed, no re-announce",
                self.name,
                self.iface
            );
            return;
        }
        let announced = announce_all_destinations_as_path_response(&self.handler).await;
        log::debug!(
            "tp({}): shared instance iface={} reappeared, {} destination(s) re-announced as path responses",
            self.name,
            self.iface,
            announced
        );
    }
}
