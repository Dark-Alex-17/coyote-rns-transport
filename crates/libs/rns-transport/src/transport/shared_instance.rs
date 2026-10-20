use std::net::SocketAddr;
use std::time::Duration;

use crate::iface::local::LocalUnixEndpoint;

/// Where the shared instance listens. Python's `LocalInterface` is the TCP
/// framing on 127.0.0.1 (`RNS/Interfaces/LocalInterface.py:62`) at the
/// configured `shared_instance_port`, 37428 by default
/// (`RNS/Reticulum.py:295`), or an abstract `AF_UNIX` socket on Linux
/// (`RNS/Reticulum.py:725-736`).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SharedInstanceEndpoint {
    /// TCP shared instance (`RNS/Interfaces/LocalInterface.py:62`,
    /// `RNS/Reticulum.py:295`).
    Tcp(SocketAddr),
    /// Unix-domain shared instance (`RNS/Reticulum.py:725-736`).
    Unix(LocalUnixEndpoint),
}

/// Options for
/// [`Transport::attach_shared_instance`](crate::transport::Transport::attach_shared_instance).
///
/// `reconnect_wait` defaults to `LocalInterface.RECONNECT_WAIT`, 8 s
/// (`RNS/Interfaces/LocalInterface.py:54`).
#[derive(Debug, Clone)]
pub struct SharedInstanceOptions {
    reconnect_wait: Duration,
}

impl Default for SharedInstanceOptions {
    fn default() -> Self {
        Self { reconnect_wait: Duration::from_secs(8) }
    }
}

impl SharedInstanceOptions {
    /// Waits `wait` between reconnect attempts after the shared instance
    /// drops the connection, instead of `LocalInterface.RECONNECT_WAIT`
    /// (`RNS/Interfaces/LocalInterface.py:54`). Applies to
    /// [`SharedInstanceEndpoint::Unix`] only; a
    /// [`SharedInstanceEndpoint::Tcp`] client retries on its own fixed 5 s
    /// interval and ignores this option.
    pub fn with_reconnect_wait(mut self, wait: Duration) -> Self {
        self.reconnect_wait = wait;
        self
    }

    /// Fork-only: the reconnect wait in force.
    pub(crate) fn reconnect_wait(&self) -> Duration {
        self.reconnect_wait
    }
}
