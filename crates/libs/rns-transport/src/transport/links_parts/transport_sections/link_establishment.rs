// Kept separate from `close_channel.rs` so that section stays below the
// repository's 500-line limit. `include!`d into the same module.

impl Transport {
    /// How long an outbound link to `destination` may stay unestablished,
    /// sized as `RNS.Link.__init__` sizes it (`RNS/Link.py:282-283`): the
    /// first hop's own timeout, then
    /// [`crate::destination::link::ESTABLISHMENT_TIMEOUT_PER_HOP`] for every
    /// hop to the destination. The first hop is sized with the fixed 500-byte
    /// Reticulum MTU (`RNS/Transport.py:3194-3197`, `RNS/Reticulum.py:93`),
    /// not the next hop interface's hardware MTU: a shared-instance client
    /// interface carries a 262144-byte MTU, which would stretch a 1-hop
    /// timeout at 62500 bps from about 12 s to about 45 s. The bitrate is
    /// the local next-hop interface's; a Python shared-instance client asks
    /// the shared instance for the first-hop figure instead
    /// (`RNS/Reticulum.py:1728-1732`), a difference of milliseconds at the
    /// 500-byte MTU.
    async fn establishment_timeout_for(&self, destination: &AddressHash, hops: u8) -> Duration {
        let per_hop = crate::destination::link::ESTABLISHMENT_TIMEOUT_PER_HOP;
        let first_hop_timeout = match self.next_hop_metrics(destination).await {
            Some(metrics) => {
                metrics.first_hop_timeout(super::next_hop_metrics::RETICULUM_MTU, per_hop)
            }
            None => per_hop,
        };
        first_hop_timeout + per_hop * u32::from(hops.max(1))
    }
}
