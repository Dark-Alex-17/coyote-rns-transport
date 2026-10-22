use std::net::SocketAddr;

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
    /// one, and a tunnel is synthesized on it at attach and again on every
    /// reconnect, as reticulumd does for its own clients. Python's
    /// shared-instance client synthesizes no tunnel at all
    /// (`RNS/Interfaces/LocalInterface.py` has no tunnel code; `wants_tunnel`
    /// is set by the network-facing clients such as
    /// `RNS/Interfaces/TCPInterface.py:179`); its reconnect hooks
    /// (`RNS/Transport.py:3690-3709`) are mirrored by a task spawned here:
    /// a dropped connection tears down every link and flushes the tables
    /// while this transport is still attached, and a reconnect re-announces
    /// every registered destination as a path response `reconnect_wait`
    /// plus two seconds later (`RNS/Interfaces/LocalInterface.py:178-181`).
    /// A detach or stop flushes nothing.
    /// Python also forces transport, remote management and probes off when it
    /// attaches (`RNS/Reticulum.py:438-440`); this method leaves
    /// `transport_enabled` as configured, since the crate supports a
    /// transport-enabled node attached to a shared instance (see
    /// `path_parts/unknown_owner_local_link_request.rs`). A `Tcp` endpoint
    /// whose address is not loopback (in either notation: `[::ffff:127.0.0.1]`
    /// counts as loopback) is accepted and warned about once per accepted
    /// attach, since Python's `LocalClientInterface` connects to `127.0.0.1`
    /// only (`RNS/Interfaces/LocalInterface.py:109`) and a remote shared
    /// instance is rarely what the caller meant; a refused attach warns nothing.
    ///
    /// Only one shared instance is held at a time: a second call while one is
    /// attached returns [`RnsError::InvalidArgument`], as does a call after
    /// [`Self::stop`] has cancelled the transport (the crate has no closer
    /// error variant); call [`Self::detach_shared_instance`] first in the
    /// former case. The slot is released only by
    /// [`Self::detach_shared_instance`] or [`Self::stop`]; removing the
    /// interface through [`Self::stop_interface`] or
    /// [`Self::remove_interface`] leaves it set and a later attach refused
    /// until detach. The connected posture is set only while the slot still
    /// holds the new interface, so a detach or stop that runs concurrently
    /// cannot leave the posture set with the slot empty. [`Self::stop`]
    /// sweeps the interfaces before it cancels the transport and takes the
    /// slot, so an attach whose interface is spawned between the sweep and
    /// the cancel leaves that interface registered and connected on the
    /// stopped transport with the slot empty and the posture off;
    /// [`Self::detach_shared_instance`] then finds nothing, and calling
    /// [`Self::stop`] again detaches it. Returns the interface address.
    #[deny(clippy::await_holding_invalid_type)]
    pub async fn attach_shared_instance(
        &self,
        endpoint: SharedInstanceEndpoint,
        options: SharedInstanceOptions,
    ) -> Result<AddressHash, RnsError> {
        let kind: &'static str = match &endpoint {
            SharedInstanceEndpoint::Tcp(_) => "tcp",
            #[cfg(unix)]
            SharedInstanceEndpoint::Unix(_) => "unix",
        };
        let remote: Option<SocketAddr> = match &endpoint {
            SharedInstanceEndpoint::Tcp(addr) if !addr.ip().to_canonical().is_loopback() => {
                Some(*addr)
            }
            _ => None,
        };
        let (reconnect_tx, reconnect_rx) = tokio::sync::mpsc::channel(8);
        let (disconnect_tx, disconnect_rx) = tokio::sync::mpsc::channel(8);
        // Lock order: the std slot is innermost, under the iface_manager here
        // and under the handler below as in detach and stop; the two tokio
        // locks are never nested. The second section re-reads the slot since
        // a detach or stop may have emptied it in between.
        let iface = {
            let mut manager = self.iface_manager.lock().await;
            let mut slot =
                self.shared_instance.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            if slot.is_some() || self.cancel.is_cancelled() {
                return Err(RnsError::InvalidArgument);
            }
            let iface = match endpoint {
                SharedInstanceEndpoint::Tcp(addr) => manager.spawn_as_with_mode(
                    TcpClient::new(addr.to_string())
                        .with_reconnect_wait(options.reconnect_wait())
                        .with_reconnect_events(reconnect_tx)
                        .with_disconnect_events(disconnect_tx),
                    TcpClient::spawn,
                    IfaceRole::Unicast,
                    InterfaceMode::Full,
                ),
                #[cfg(unix)]
                SharedInstanceEndpoint::Unix(endpoint) => manager.spawn_as_with_mode(
                    LocalUnixClient::new_connect(endpoint)
                        .with_reconnect_wait(options.reconnect_wait())
                        .with_reconnect_events(reconnect_tx)
                        .with_disconnect_events(disconnect_tx),
                    LocalUnixClient::spawn,
                    IfaceRole::Unicast,
                    InterfaceMode::Full,
                ),
            };
            manager.set_shared_instance(iface, true);
            *slot = Some(iface);
            iface
        };
        if let Some(addr) = remote {
            log::warn!(
                "tp({}): shared instance iface={} endpoint {} is not a loopback address; Python's LocalClientInterface connects to 127.0.0.1 only (RNS/Interfaces/LocalInterface.py:109), the attach proceeds",
                self.name,
                iface,
                addr
            );
        }
        self.spawn_shared_instance_reconnect_consumer(
            iface,
            options.reconnect_wait(),
            reconnect_rx,
            disconnect_rx,
        );
        {
            let mut handler = self.handler.lock().await;
            let slot = self.shared_instance.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            if *slot == Some(iface) {
                handler.config.set_connected_to_shared_instance(true);
            }
        }
        if !self.synthesize_tunnel_on_interface(iface).await {
            log::debug!(
                "tp({}): shared instance iface={} removed by a detach, stop or stop_interface that raced the attach, tunnel not synthesized",
                self.name,
                iface
            );
        }
        log::debug!(
            "tp({}): attached shared instance iface={} endpoint={} reconnect_wait={:?}",
            self.name,
            iface,
            kind,
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
        let unflagged = { self.iface_manager.lock().await.set_shared_instance(iface, false) };
        let stopped = self.stop_interface(iface).await;
        if !(unflagged && stopped) {
            log::debug!(
                "tp({}): shared instance slot was stale, iface={} already removed (unflagged={} stopped={})",
                self.name,
                iface,
                unflagged,
                stopped
            );
        }
        log::debug!("tp({}): detached shared instance iface={}", self.name, iface);
        true
    }

    /// [`Self::stop`] took the slot with `iface` still in it; logged from
    /// here so the line shares the attach and detach lines' target.
    pub(super) fn log_shared_instance_cleared_by_stop(&self, iface: AddressHash) {
        log::debug!(
            "tp({}): stopped with shared instance iface={} attached, slot cleared",
            self.name,
            iface
        );
    }

    /// A shared-instance task [`Self::stop`] joined ended by panic, the only
    /// `JoinError` a join can observe here: the tasks still running at the
    /// budget are aborted and dropped, never joined again, so stop stays
    /// inside its budget. Logged from here so the line shares the attach
    /// and detach lines' target.
    pub(super) fn log_shared_instance_task_join_failed(&self, err: &tokio::task::JoinError) {
        log::warn!(
            "tp({}): shared instance task did not finish cleanly during stop: {err}",
            self.name
        );
    }

    /// [`Self::stop`] ran out of budget with `running` shared-instance tasks
    /// still going and aborted them; logged from here for the same reason.
    pub(super) fn log_shared_instance_tasks_aborted_by_stop(&self, running: usize) {
        log::warn!(
            "tp({}): {running} shared instance task(s) still running after stop budget, aborted",
            self.name
        );
    }

    /// Fork-only: the interface attached with
    /// [`Self::attach_shared_instance`], if any; see
    /// [`Self::attach_shared_instance`] for when the slot is released.
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
