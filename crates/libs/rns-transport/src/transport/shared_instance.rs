use std::net::SocketAddr;
use std::time::Duration;

use tokio::sync::mpsc;

#[cfg(unix)]
use crate::iface::local::LocalUnixEndpoint;
use crate::iface::InterfaceMode;

/// Where the shared instance listens. Python's `LocalClientInterface` is the
/// TCP framing on 127.0.0.1 (`RNS/Interfaces/LocalInterface.py:109`) at the
/// configured `shared_instance_port`, 37428 by default
/// (`RNS/Reticulum.py:295`), or an abstract `AF_UNIX` socket on Linux
/// (`RNS/Reticulum.py:725-736`).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SharedInstanceEndpoint {
    /// TCP shared instance (`RNS/Interfaces/LocalInterface.py:109`,
    /// `RNS/Reticulum.py:295`). A non-loopback address is refused by
    /// [`Transport::attach_shared_instance`](crate::transport::Transport::attach_shared_instance)
    /// unless [`SharedInstanceOptions::allow_remote_endpoint`] is set.
    Tcp(SocketAddr),
    /// Unix-domain shared instance (`RNS/Reticulum.py:725-736`). Unix
    /// targets only.
    #[cfg(unix)]
    Unix(LocalUnixEndpoint),
}

/// Fork-only: the connection state of the client
/// [`Transport::attach_shared_instance`](crate::transport::Transport::attach_shared_instance)
/// spawned, delivered on the sender given to
/// [`SharedInstanceOptions::with_state_events`]. Python's
/// `LocalClientInterface` exposes no such stream; its reconnect loop is
/// `RNS/Interfaces/LocalInterface.py:155-181`.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SharedInstanceEvent {
    /// Fork-only: the attach is in place and the client is dialing; sent
    /// once by the attach itself and again after every [`Self::Disconnected`].
    /// A client attached while nothing listens stays here until the socket
    /// exists.
    Connecting,
    /// Fork-only: a stream to the shared instance is established, the first
    /// included; the tunnel is synthesized on it next.
    Connected,
    /// Fork-only: an established stream ended while attached
    /// (`RNS/Interfaces/LocalInterface.py:255-261` is the Python drop path);
    /// the flush, which runs only while the transport is still attached,
    /// has run; [`Self::Connecting`] follows at once. A detach or
    /// [`Transport::stop`](crate::transport::Transport::stop) ends the
    /// stream without one: the sender is dropped instead, though a drop
    /// that was already queued when the detach landed is still served and
    /// read before the stream ends.
    Disconnected,
}

/// Options for
/// [`Transport::attach_shared_instance`](crate::transport::Transport::attach_shared_instance).
///
/// `reconnect_wait` defaults to `LocalClientInterface.RECONNECT_WAIT`, 8 s
/// (`RNS/Interfaces/LocalInterface.py:53-54`). A non-loopback
/// [`SharedInstanceEndpoint::Tcp`] is refused unless
/// [`Self::allow_remote_endpoint`] is set, where loopback is recognised in
/// any notation (`127.0.0.1`, `[::1]`, `[::ffff:127.0.0.1]`: the address is
/// checked after `to_canonical()`); the MTU, forced bitrate and interface
/// mode default to the client's own.
#[derive(Debug, Clone)]
pub struct SharedInstanceOptions {
    reconnect_wait: Duration,
    allow_remote_endpoint: bool,
    mtu: Option<usize>,
    forced_bitrate_bps: Option<u64>,
    interface_mode: InterfaceMode,
    state_events: Option<mpsc::Sender<SharedInstanceEvent>>,
}

impl Default for SharedInstanceOptions {
    fn default() -> Self {
        Self {
            reconnect_wait: Duration::from_secs(8),
            allow_remote_endpoint: false,
            mtu: None,
            forced_bitrate_bps: None,
            interface_mode: InterfaceMode::Full,
            state_events: None,
        }
    }
}

impl SharedInstanceOptions {
    /// Waits `wait` after each failed connect to the endpoint, including the
    /// retries after the shared instance drops the connection, instead of
    /// `LocalClientInterface.RECONNECT_WAIT`
    /// (`RNS/Interfaces/LocalInterface.py:53-54`). The wait is clamped to at
    /// least 1 ms here, as the [`SharedInstanceEndpoint::Tcp`] client and
    /// the unix-domain client clamp it again; both honour it. The TCP client
    /// redials at once after a drop and waits `wait` from the second
    /// consecutive short-lived connection on, one that lasted under
    /// `MIN_STABLE_CONNECTION` (2 s); a connection that outlived the window
    /// resets the streak. The unix client waits `wait` before every retry,
    /// as Python does (`:160`).
    #[must_use]
    pub fn with_reconnect_wait(mut self, wait: Duration) -> Self {
        self.reconnect_wait = wait.max(Duration::from_millis(1));
        self
    }

    /// Fork-only: attaches to a [`SharedInstanceEndpoint::Tcp`] whose
    /// address is not loopback, which the attach otherwise refuses with
    /// [`RnsError::SharedInstanceEndpointNotLoopback`](crate::error::RnsError::SharedInstanceEndpointNotLoopback).
    /// Python has no remote option at all: its `LocalClientInterface`
    /// connects to `127.0.0.1` only (`RNS/Interfaces/LocalInterface.py:109`).
    /// Setting this grants the remote host the trust a local shared instance
    /// gets: it receives dedup delegation, since this transport stops
    /// filtering duplicates while attached (`RNS/Transport.py:1618-1627`),
    /// and its stream drop flushes every table and tears down every link
    /// (`RNS/Transport.py:3690-3702`). Each accepted remote attach is logged
    /// at warn.
    #[must_use]
    pub fn allow_remote_endpoint(mut self, allow: bool) -> Self {
        self.allow_remote_endpoint = allow;
        self
    }

    /// Fork-only: the MTU of the spawned client, `TcpClient::with_mtu` or
    /// `LocalUnixClient::with_mtu`, in place of the client's default; the
    /// client clamps it to at least 256.
    #[must_use]
    pub fn with_mtu(mut self, mtu: usize) -> Self {
        self.mtu = Some(mtu);
        self
    }

    /// Fork-only: the forced bitrate of the spawned client,
    /// `TcpClient::with_forced_bitrate` or `LocalUnixClient::with_forced_bitrate`;
    /// the client treats zero as unset.
    #[must_use]
    pub fn with_forced_bitrate(mut self, bitrate_bps: u64) -> Self {
        self.forced_bitrate_bps = Some(bitrate_bps);
        self
    }

    /// Fork-only: the [`InterfaceMode`] the spawned client is registered
    /// under, [`InterfaceMode::Full`] by default.
    #[must_use]
    pub fn with_interface_mode(mut self, mode: InterfaceMode) -> Self {
        self.interface_mode = mode;
        self
    }

    /// Fork-only: delivers the client's [`SharedInstanceEvent`]s on `events`.
    /// Each cycle is `Disconnected`, `Connecting`, `Connected` in that order,
    /// after the attach's own `Connecting` and the first `Connected`. Every
    /// event is sent with `try_send`: a full channel drops the event at
    /// debug and never blocks the consumer, so size the channel for the
    /// burst expected. Under a reconnect storm the cycles can interleave:
    /// queued disconnects are served before connects, so two `Disconnected`
    /// can precede their `Connected`s; the per-cycle order holds, the
    /// cycles may nest. The attach's clone of the sender is dropped on
    /// [`Transport::detach_shared_instance`](crate::transport::Transport::detach_shared_instance),
    /// [`Transport::stop`](crate::transport::Transport::stop) or when the
    /// client worker exits; the stream ends once no other clone, a
    /// retained `SharedInstanceOptions` clone included, is alive. A detach
    /// or a stop sends no `Disconnected`, but a drop that was already
    /// queued when the detach landed is still served. The posture flag
    /// [`Transport::connected_to_shared_instance`](crate::transport::Transport::connected_to_shared_instance)
    /// is unchanged by these events: it stays set across a `Disconnected`
    /// until detach or stop. Python's `LocalClientInterface` exposes no
    /// such stream (`RNS/Interfaces/LocalInterface.py:155-181`).
    #[must_use]
    pub fn with_state_events(mut self, events: mpsc::Sender<SharedInstanceEvent>) -> Self {
        self.state_events = Some(events);
        self
    }

    /// Fork-only: the reconnect wait in force.
    pub(crate) fn reconnect_wait(&self) -> Duration {
        self.reconnect_wait
    }

    /// Fork-only: whether a non-loopback TCP endpoint may be attached.
    pub(crate) fn remote_endpoint_allowed(&self) -> bool {
        self.allow_remote_endpoint
    }

    /// Fork-only: the MTU set with [`Self::with_mtu`], if any.
    #[must_use]
    pub fn mtu(&self) -> Option<usize> {
        self.mtu
    }

    /// Fork-only: the forced bitrate set with [`Self::with_forced_bitrate`], if any.
    #[must_use]
    pub fn forced_bitrate_bps(&self) -> Option<u64> {
        self.forced_bitrate_bps
    }

    /// Fork-only: the interface mode the client is registered under.
    pub(crate) fn interface_mode(&self) -> InterfaceMode {
        self.interface_mode
    }

    /// Fork-only: the state-event sender set with [`Self::with_state_events`], if any.
    pub(crate) fn state_events(&self) -> Option<&mpsc::Sender<SharedInstanceEvent>> {
        self.state_events.as_ref()
    }
}
