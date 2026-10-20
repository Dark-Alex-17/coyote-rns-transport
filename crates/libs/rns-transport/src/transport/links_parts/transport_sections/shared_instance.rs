#[cfg(unix)]
use crate::iface::local::LocalUnixClient;
use crate::iface::tcp_client::TcpClient;
use crate::iface::InterfaceMode;

impl Transport {
    /// Attaches this transport as a client of a Reticulum shared instance at
    /// `endpoint`, the posture a Python program takes when another instance
    /// already owns the configured `shared_instance_port`
    /// (`RNS/Reticulum.py:422-441`, `:437` marks it connected): the interface
    /// is marked as the shared instance, the transport is marked connected to
    /// one, and a tunnel is synthesized on it once, at attach, as reticulumd
    /// does for its local client. The tunnel is not re-synthesized when the
    /// client reconnects. Python's shared-instance client synthesizes no
    /// tunnel at all (`RNS/Interfaces/LocalInterface.py` has no tunnel code;
    /// `wants_tunnel` is set by the network-facing clients such as
    /// `RNS/Interfaces/TCPInterface.py:179`); its reconnect hooks re-announce
    /// and flush the tables instead
    /// (`RNS/Transport.py:3690-3709`), which this crate does not yet mirror.
    /// Python also forces transport, remote management and probes off when it
    /// attaches (`RNS/Reticulum.py:438-440`); this method leaves
    /// `transport_enabled` as configured, since the crate supports a
    /// transport-enabled node attached to a shared instance (see
    /// `path_parts/unknown_owner_local_link_request.rs`).
    ///
    /// Only one shared instance is held at a time: a second call while one is
    /// attached returns [`RnsError::InvalidArgument`]; call
    /// [`Self::detach_shared_instance`] first. The slot is released only by
    /// [`Self::detach_shared_instance`] or [`Self::stop`]; removing the
    /// interface through [`Self::stop_interface`] or
    /// [`Self::remove_interface`] leaves it set and a later attach refused
    /// until detach. Returns the interface address.
    #[deny(clippy::await_holding_invalid_type)]
    pub async fn attach_shared_instance(
        &self,
        endpoint: SharedInstanceEndpoint,
        options: SharedInstanceOptions,
    ) -> Result<AddressHash, RnsError> {
        let iface = {
            let mut manager = self.iface_manager.lock().await;
            let mut slot =
                self.shared_instance.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            if slot.is_some() {
                return Err(RnsError::InvalidArgument);
            }
            let iface = match endpoint {
                SharedInstanceEndpoint::Tcp(addr) => manager.spawn_as_with_mode(
                    TcpClient::new(addr.to_string()),
                    TcpClient::spawn,
                    IfaceRole::Unicast,
                    InterfaceMode::Full,
                ),
                #[cfg(unix)]
                SharedInstanceEndpoint::Unix(endpoint) => manager.spawn_as_with_mode(
                    LocalUnixClient::new_connect(endpoint)
                        .with_reconnect_wait(options.reconnect_wait()),
                    LocalUnixClient::spawn,
                    IfaceRole::Unicast,
                    InterfaceMode::Full,
                ),
            };
            manager.set_shared_instance(iface, true);
            *slot = Some(iface);
            iface
        };
        self.set_connected_to_shared_instance(true).await;
        if !self.synthesize_tunnel_on_interface(iface).await {
            log::warn!(
                "tp({}): shared instance iface={} unknown to the interface manager, tunnel not synthesized",
                self.name,
                iface
            );
        }
        log::debug!(
            "tp({}): attached shared instance iface={} reconnect_wait={:?}",
            self.name,
            iface,
            options.reconnect_wait()
        );
        Ok(iface)
    }

    /// Fork-only: stops the shared-instance interface attached with
    /// [`Self::attach_shared_instance`] and marks the transport as no longer
    /// connected to one. Returns `false` when none was attached.
    #[deny(clippy::await_holding_invalid_type)]
    pub async fn detach_shared_instance(&self) -> bool {
        let iface = {
            let mut handler = self.handler.lock().await;
            let mut slot =
                self.shared_instance.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            let iface = slot.take();
            if iface.is_some() {
                handler.config.set_connected_to_shared_instance(false);
            }
            iface
        };
        let Some(iface) = iface else {
            return false;
        };
        {
            self.iface_manager.lock().await.set_shared_instance(iface, false);
        }
        self.stop_interface(iface).await;
        log::debug!("tp({}): detached shared instance iface={}", self.name, iface);
        true
    }

    /// Fork-only: the interface attached with
    /// [`Self::attach_shared_instance`], if any. Released only by
    /// [`Self::detach_shared_instance`] or [`Self::stop`]; removing the
    /// interface through [`Self::stop_interface`] or
    /// [`Self::remove_interface`] leaves it set.
    pub fn shared_instance(&self) -> Option<AddressHash> {
        *self.shared_instance.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Whether this transport runs as a shared-instance client
    /// (`RNS/Reticulum.py:422-441`, `:437`), whether set by
    /// [`Self::attach_shared_instance`] or
    /// [`Self::set_connected_to_shared_instance`].
    #[deny(clippy::await_holding_invalid_type)]
    pub async fn connected_to_shared_instance(&self) -> bool {
        self.handler.lock().await.config.connected_to_shared_instance
    }
}
