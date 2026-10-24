use std::net::SocketAddr;
use std::time::Duration;

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
}

impl Default for SharedInstanceOptions {
    fn default() -> Self {
        Self {
            reconnect_wait: Duration::from_secs(8),
            allow_remote_endpoint: false,
            mtu: None,
            forced_bitrate_bps: None,
            interface_mode: InterfaceMode::Full,
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
}
