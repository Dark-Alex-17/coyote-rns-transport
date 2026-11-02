// The path wait, `include!`d into `links_parts/transport.rs` beside
// `resolve_destination.rs`, which keeps the wait next to the lookup it
// serves; a file boundary, not a privacy one.

impl Transport {
    /// Waits until a path to `destination` is known or `timeout` passes,
    /// returning whether one stands at the end. A path already in the table
    /// answers `true` at once and nothing is sent; otherwise exactly one path
    /// request is attempted through [`Self::request_path`], on `on_iface` or,
    /// with `None`, on every outgoing interface. The table is then polled
    /// every 50 ms or the time left, whichever is shorter, so a short timeout
    /// returns at its deadline rather than one poll later. A timeout the clock
    /// cannot hold, `Duration::MAX` included, is treated as waiting
    /// indefinitely instead of panicking.
    ///
    /// Fork-only convenience over `RNS.Transport.request_path`
    /// (`RNS/Transport.py:3279`) and a `has_path` poll
    /// (`RNS/Transport.py:3126`).
    #[deny(clippy::await_holding_invalid_type)]
    pub async fn await_path(
        &self,
        destination: &AddressHash,
        timeout: Duration,
        on_iface: Option<AddressHash>,
    ) -> bool {
        if self.has_path(destination).await {
            return true;
        }
        self.request_path(destination, on_iface, None).await;
        let now = tokio::time::Instant::now();
        // Thirty years, as tokio's own far future: a century overflows the
        // clock on some platforms.
        let deadline = now
            .checked_add(timeout)
            .unwrap_or_else(|| now + Duration::from_secs(30 * 365 * 24 * 60 * 60));
        let poll = Duration::from_millis(50);
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                break;
            }
            tokio::time::sleep(remaining.min(poll)).await;
            if self.has_path(destination).await {
                return true;
            }
        }
        self.has_path(destination).await
    }
}
