impl Transport {
    /// Fork-only: one broadcast stream of every stream client interface's
    /// connection state on this transport, each event tagged with its
    /// interface address: `Connecting` per dial attempt, `Connected` per
    /// established stream, `Disconnected { reason }` when an established
    /// stream ends for a reason the client reports, `DialFailed { reason }`
    /// per failed attempt. Published by a dialing `TcpClient` or
    /// `LocalUnixClient` spawned through this transport's interface
    /// manager, the client an `attach_shared_instance` spawns included. A
    /// client built from a pre-supplied stream (`TcpClient::new_from_stream`,
    /// `LocalUnixClient::new_from_stream`; how `TcpServer` and
    /// `LocalUnixServer` run each accepted peer) publishes nothing for that
    /// stream, and every other interface kind publishes nothing yet. The
    /// per-client `with_*_events` senders and the shared-instance state
    /// stream keep working beside it. The stream has no replay: subscribe
    /// before spawning the client whose events you want. Capacity is the
    /// transport's event capacity (1024); a receiver that falls behind reads
    /// `RecvError::Lagged(n)` for the n events it missed and never slows a
    /// client, and a dropped receiver costs the clients nothing. Python has
    /// no such stream (`RNS/Interfaces/LocalInterface.py:159-167`, `:255-266`).
    pub fn interface_events(&self) -> broadcast::Receiver<InterfaceEvent> {
        self.interface_events_tx.subscribe()
    }
}
