use std::net::SocketAddr;

#[cfg(unix)]
use crate::iface::local::LocalUnixClient;
use crate::iface::tcp_client::TcpClient;

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
    /// With [`SharedInstanceOptions::with_state_events`] set, the attach
    /// sends [`SharedInstanceEvent::Connecting`] once and that task sends
    /// the events of every connection cycle after it; the stream ends when
    /// the task does.
    /// Python also forces transport, remote management and probes off when it
    /// attaches (`RNS/Reticulum.py:438-440`); this method leaves
    /// `transport_enabled` as configured, since the crate supports a
    /// transport-enabled node attached to a shared instance (see
    /// `path_parts/unknown_owner_local_link_request.rs`). A `Tcp` endpoint
    /// whose address is not loopback (in either notation: `[::ffff:127.0.0.1]`
    /// counts as loopback) is refused with
    /// [`RnsError::SharedInstanceEndpointNotLoopback`] before any lock is
    /// taken, with one warning, unless
    /// [`SharedInstanceOptions::allow_remote_endpoint`] is set, in which case
    /// the attach proceeds and warns once per accepted attach. Python's
    /// `LocalClientInterface` connects to `127.0.0.1` only
    /// (`RNS/Interfaces/LocalInterface.py:109`). An attach refused by the
    /// slot or by a stop warns nothing.
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
        if let Some(addr) = remote {
            if !options.remote_endpoint_allowed() {
                log::warn!(
                    "tp({}): shared instance endpoint={} is not a loopback address and allow_remote_endpoint is not set, attach refused; Python's LocalClientInterface connects to 127.0.0.1 only (RNS/Interfaces/LocalInterface.py:109)",
                    self.name,
                    addr
                );
                return Err(RnsError::SharedInstanceEndpointNotLoopback);
            }
        }
        let (reconnect_tx, reconnect_rx) = tokio::sync::mpsc::channel(8);
        let (disconnect_tx, disconnect_rx) = tokio::sync::mpsc::channel(8);
        let (connect_tx, connect_rx) = tokio::sync::mpsc::channel(8);
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
                SharedInstanceEndpoint::Tcp(addr) => {
                    let mut client = TcpClient::new(addr.to_string())
                        .with_reconnect_wait(options.reconnect_wait())
                        .with_connect_events(connect_tx)
                        .with_reconnect_events(reconnect_tx)
                        .with_disconnect_events(disconnect_tx);
                    if let Some(mtu) = options.mtu() {
                        client = client.with_mtu(mtu);
                    }
                    if let Some(bitrate_bps) = options.forced_bitrate_bps() {
                        client = client.with_forced_bitrate(bitrate_bps);
                    }
                    manager.spawn_as_with_mode(
                        client,
                        TcpClient::spawn,
                        IfaceRole::Unicast,
                        options.interface_mode(),
                    )
                }
                #[cfg(unix)]
                SharedInstanceEndpoint::Unix(endpoint) => {
                    let mut client = LocalUnixClient::new_connect(endpoint)
                        .with_reconnect_wait(options.reconnect_wait())
                        .with_connect_events(connect_tx)
                        .with_reconnect_events(reconnect_tx)
                        .with_disconnect_events(disconnect_tx);
                    if let Some(mtu) = options.mtu() {
                        client = client.with_mtu(mtu);
                    }
                    if let Some(bitrate_bps) = options.forced_bitrate_bps() {
                        client = client.with_forced_bitrate(bitrate_bps);
                    }
                    manager.spawn_as_with_mode(
                        client,
                        LocalUnixClient::spawn,
                        IfaceRole::Unicast,
                        options.interface_mode(),
                    )
                }
            };
            manager.set_shared_instance(iface, true);
            *slot = Some(iface);
            iface
        };
        if let Some(addr) = remote {
            log::warn!(
                "tp({}): shared instance iface={} endpoint={} is not a loopback address, attached under allow_remote_endpoint; Python's LocalClientInterface connects to 127.0.0.1 only (RNS/Interfaces/LocalInterface.py:109)",
                self.name,
                iface,
                addr
            );
        }
        // The consumer is the only sender of `Connected` and is spawned
        // below, so this `Connecting` precedes it even when the client has
        // already connected and queued its event.
        send_shared_instance_state_event(
            &self.name,
            iface,
            options.state_events(),
            SharedInstanceEvent::Connecting,
        );
        self.spawn_shared_instance_reconnect_consumer(
            iface,
            options.reconnect_wait(),
            reconnect_rx,
            connect_rx,
            disconnect_rx,
            options.state_events().cloned(),
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
            "tp({}): attached shared instance iface={} kind={} reconnect_wait={:?}",
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

    /// A shared-instance task [`Self::stop`] joined ended by panic or was
    /// aborted by a holder of its handle; the in-crate spawner drops the
    /// handle, so in-crate it is the panic case. The tasks still running at
    /// the budget are aborted and dropped, never joined again, so stop stays
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
