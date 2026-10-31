// The destination lookup that waits for a path, `include!`d into
// `links_parts/transport.rs` beside `close_channel.rs` so that section
// stays below the repository's 500-line limit; a file boundary, not a
// privacy one.

impl Transport {
    /// Returns the description of `address`, asking the network for it when
    /// it is not yet known: [`Self::destination_desc`] and
    /// [`Self::await_path`] in one call.
    ///
    /// A destination this transport already knows answers at once and no
    /// path request goes out. With a zero `timeout` the call is a cached
    /// lookup alone and returns `None` for an unknown destination without
    /// sending anything. Otherwise exactly one path request is sent, a
    /// `Broadcast` on every outgoing interface as [`Self::request_path`]
    /// sends it, and the call polls every 50 ms until the destination is
    /// learned or `timeout` passes, returning `None` after it. The entry
    /// read is the one `destination_desc` and `destination_identity` read,
    /// filled when an announce is accepted or a cached path table is
    /// restored. A destination [`Self::deregister_destination`] removed
    /// while its path table entry stands is the one exception: `await_path`
    /// takes the known path as its answer, so the call returns `None` at
    /// once without a request.
    ///
    /// Fork-only convenience: a Python application does the same by hand
    /// with `RNS.Transport.request_path` (`RNS/Transport.py:3279`) and a
    /// `has_path` poll (`RNS/Transport.py:3126`) before `RNS.Identity.recall`.
    #[deny(clippy::await_holding_invalid_type)]
    pub async fn resolve_destination(
        &self,
        address: &AddressHash,
        timeout: Duration,
    ) -> Option<DestinationDesc> {
        if let Some(desc) = self.destination_desc(address).await {
            return Some(desc);
        }
        if timeout.is_zero() {
            return None;
        }
        self.await_path(address, timeout, None).await;
        self.destination_desc(address).await
    }
}
