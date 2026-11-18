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
    /// `stop`. A `TcpClient` that gave up after `max_reconnect_tries`
    /// keeps its last `Some(n)` until the manager prunes its record, which
    /// needs no call: giving up cancels the interface's stop token, and the
    /// manager's next cleanup pass (every send, and the announce job's
    /// 1 s tick) drops cancelled records, so the read goes `None` within
    /// about a second. Python has no equivalent: its reconnect loop keeps
    /// only a per-burst local `attempts` and no count of established
    /// streams (`RNS/Interfaces/TCPInterface.py:270-298`, `:274-277`).
    #[deny(clippy::await_holding_invalid_type)]
    pub async fn interface_connect_count(&self, iface: &AddressHash) -> Option<u64> {
        self.iface_manager.lock().await.connected_streams(iface)
    }

    /// Fork-only: how many peers the AutoInterface on `iface` currently
    /// holds in its discovery peer table, what Python reports as
    /// `len(interface.peers)` (`RNS/Reticulum.py:1501-1503`, over the dict
    /// at `RNS/Interfaces/AutoInterface.py:126`) and rnstatus prints
    /// (`RNS/Utilities/rnstatus.py:555-556`). The count rises as peers
    /// authenticate and falls as the peer job expires them or an adopted
    /// device is removed with its peers, so two reads may differ with no
    /// event in between. A virtual child registered on the AutoInterface's
    /// host channel answers its host's count. `None` collapses two cases:
    /// no record in this transport's interface manager carries the address
    /// (never minted, built with `InterfaceChannel::new` as tests do, or
    /// already stopped and pruned), and the record carries no counter,
    /// which is every interface kind other than an AutoInterface host and
    /// its virtual children (TCP, UDP, serial, KISS, RNode, pipe and the
    /// rest), the shared instance's interface, and a channel minted here
    /// that no `InterfaceManager::set_peer_counter` call reached. Python
    /// exposes the count only through rnstatus and its RPC; the fork also
    /// exposes it in-process here. Nothing publishes 0 when the AutoInterface
    /// runtime stops: the counter keeps its last published value, so the read
    /// keeps its last `Some(n)` until the manager prunes the host record
    /// (`stop_interface`, or the manager's next cleanup pass once the
    /// interface's stop token is cancelled), after which it reads `None`; a
    /// status handle sharing the counter keeps reading `n` for as long as it
    /// lives.
    #[deny(clippy::await_holding_invalid_type)]
    pub async fn interface_peer_count(&self, iface: &AddressHash) -> Option<u64> {
        self.iface_manager.lock().await.peer_count(iface)
    }
}
