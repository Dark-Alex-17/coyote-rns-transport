/// Mirrors RNS/Resource.py:1374, which raises on an advertised transfer size
/// above three times `MAX_EFFICIENT_SIZE`. Here the advertisement is dropped
/// and logged so the link stays usable.
pub(crate) const MAX_REFERENCE_ADVERTISED_TRANSFER_SIZE: u64 = 3 * (MAX_EFFICIENT_SIZE as u64);

/// Enforces the inbound advertisement caps before any receiver state is
/// created (issue #514): a hostile peer must not get a `ResourceReceiver`
/// (and the part-tracking allocations that come with it) for a resource
/// that exceeds the reference transfer-size cap or the part-count cap.
///
/// Returns `true` when the advertisement must be rejected. Each rejection
/// logs the exact limit, the advertised value, the resource hash, and the
/// peer link. `ResourceReceiver::new_with_mtu` re-checks the inbound bounds
/// as defense in depth; these guards reject early with actionable context.
fn advertisement_exceeds_inbound_limits(
    advertisement: &ResourceAdvertisement,
    link_id: &AddressHash,
) -> bool {
    if advertisement.transfer_size > MAX_REFERENCE_ADVERTISED_TRANSFER_SIZE {
        log::debug!(
            "[resource-diag] reject_advertisement invalid_transfer_size link={} hash={} transfer_size={} limit={}",
            link_id,
            advertisement.hash,
            advertisement.transfer_size,
            MAX_REFERENCE_ADVERTISED_TRANSFER_SIZE
        );
        return true;
    }
    if u64::from(advertisement.parts) > MAX_INBOUND_RESOURCE_PARTS {
        log::debug!(
            "[resource-diag] reject_advertisement parts_limit link={} hash={} parts={} limit={}",
            link_id,
            advertisement.hash,
            advertisement.parts,
            MAX_INBOUND_RESOURCE_PARTS
        );
        return true;
    }
    false
}

impl ResourceManager {
    /// Counts the drop against the link and warns on the first one and on
    /// every 64th after that, so a peer that keeps offering over-cap
    /// resources stays visible without one warn per advertisement. Covers
    /// both the manager's own guards and the bounds the receiver constructor
    /// re-checks, so every dropped advertisement shares one count.
    fn record_rejected_advertisement(
        &mut self,
        advertisement: &ResourceAdvertisement,
        link_id: &AddressHash,
    ) {
        let stats = self.link_stats.entry(*link_id).or_insert_with(LinkStats::new);
        stats.rejected_advertisements = stats.rejected_advertisements.saturating_add(1);
        let count = stats.rejected_advertisements;
        if count == 1 || count.is_multiple_of(64) {
            log::warn!(
                "rejecting resource advertisement over inbound limits \
                 link={} hash={} transfer_size={} data_size={} parts={} rejected={}",
                link_id,
                advertisement.hash,
                advertisement.transfer_size,
                advertisement.data_size,
                advertisement.parts,
                count
            );
        }
    }

    /// Counts a request advertisement refused for exceeding the destination's
    /// `max_request_size` (the size check mirrors `RNS/Link.py:1036-1046`)
    /// against the link and warns on the first one and on every 64th after
    /// that, so a peer that keeps sending oversized requests stays visible
    /// without one warn per advertisement. The reference logs every refusal;
    /// the cadence is fork-only.
    pub(crate) fn record_rejected_request_advertisement(
        &mut self,
        advertisement: &ResourceAdvertisement,
        link_id: &AddressHash,
        limit: usize,
    ) {
        let stats = self.link_stats.entry(*link_id).or_insert_with(LinkStats::new);
        stats.rejected_request_advertisements =
            stats.rejected_request_advertisements.saturating_add(1);
        let count = stats.rejected_request_advertisements;
        let level = if count == 1 || count.is_multiple_of(64) {
            log::Level::Warn
        } else {
            log::Level::Debug
        };
        log::log!(
            level,
            "[resource-diag] reject_request_advertisement link={} hash={} data_size={} transfer_size={} limit={} rejected={}",
            link_id,
            advertisement.hash,
            advertisement.data_size,
            advertisement.transfer_size,
            limit,
            count
        );
    }
}
