impl Transport {
    /// Fork-only: one broadcast stream of every stream client interface's
    /// connection state on this transport, each event tagged with its
    /// interface address: `Connecting` per dial attempt, `Connected` per
    /// established stream, `Disconnected { reason }` when an established
    /// stream ends for a reason the client reports (a stream the client
    /// ended itself publishes nothing), `DialFailed { reason }` per failed
    /// attempt. Published by a dialing `TcpClient` or `LocalUnixClient`
    /// spawned through this transport's interface manager, the client an
    /// `attach_shared_instance` spawns included. A client built from a
    /// pre-supplied stream (`TcpClient::new_from_stream`,
    /// `LocalUnixClient::new_from_stream`; how `TcpServer` and
    /// `LocalUnixServer` run each accepted peer) publishes nothing for that
    /// stream, and every other interface kind publishes nothing yet. A
    /// client that stops, or gives up after `max_reconnect_tries`, publishes
    /// no further event: its last event is the `Connected` of the stream it
    /// was on, or the `Connecting` or `DialFailed` of the attempt it
    /// abandoned, or the `Disconnected { reason }` of a stream that ended
    /// just before it stopped or gave up. The per-client `with_*_events`
    /// senders and the shared-instance state stream keep working beside it.
    /// The stream has no replay: subscribe before spawning the client whose
    /// events you want. Capacity is the transport's event capacity (1024); a
    /// receiver that falls behind reads `RecvError::Lagged(n)` for the n
    /// events it missed and never slows a client, and a dropped receiver
    /// costs the clients nothing. Python has no such stream
    /// (`RNS/Interfaces/LocalInterface.py:159-167`, `:255-266`).
    pub fn interface_events(&self) -> broadcast::Receiver<InterfaceEvent> {
        self.interface_events_tx.subscribe()
    }

    /// Fork-only: how many streams the stream client on `iface` has
    /// established since the interface was spawned, every reconnect
    /// included. The count is incremented before the client publishes the
    /// matching `Connected` on `interface_events()`, so a reader that saw
    /// that event reads at least that count, and a reader that compares
    /// two reads taken either side of a `RecvError::Lagged` learns whether
    /// a reconnect happened in between even though the events themselves
    /// were dropped. A client built from a pre-supplied stream counts that
    /// stream too, though it publishes no `Connected` for it. Any other
    /// interface the manager spawned (listeners, datagram, serial and KISS
    /// carriers, the I2P interface) reads `Some(0)` for as long as it
    /// lives; only `TcpClient` and `LocalUnixClient` move the count. The
    /// shared instance's interface is `Transport::shared_instance()`.
    /// `None` once no record in this transport's interface manager carries
    /// the address: never minted, built with `InterfaceChannel::new`, as
    /// tests do, or already stopped and pruned, so the read goes `Some(n)`
    /// to `None` after `stop_interface`, `detach_shared_instance` or
    /// `stop`. A client that gave up after `max_reconnect_tries` keeps its
    /// last `Some(n)` until the manager prunes its record, which needs no
    /// call: giving up cancels the interface's stop token, and the
    /// manager's next cleanup pass (every send, and the announce job's
    /// one-second tick) drops cancelled records, so the read goes `None`
    /// within about a second. Python has no equivalent: its reconnect loop
    /// keeps only a per-burst local `attempts` and no count of established
    /// streams (`RNS/Interfaces/TCPInterface.py:270-298`, `:274-277`).
    #[deny(clippy::await_holding_invalid_type)]
    pub async fn interface_connect_count(&self, iface: &AddressHash) -> Option<u64> {
        self.iface_manager.lock().await.connected_streams(iface)
    }
}
