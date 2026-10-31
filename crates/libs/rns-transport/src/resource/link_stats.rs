/// Exponentially weighted moving average: alpha = 7/8.
pub(crate) fn ewma(old: Duration, sample: Duration) -> Duration {
    let micros =
        (old.as_micros() as u64).saturating_mul(7).saturating_add(sample.as_micros() as u64) / 8;
    Duration::from_micros(micros)
}

/// Per-link adaptive statistics used to decide when to re-request lost fragments.
#[derive(Debug, Clone, Copy)]
pub(crate) struct LinkStats {
    /// EWMA of round-trip time (request to part arrival).
    pub(crate) rtt: Duration,
    /// EWMA of inter-arrival interval between consecutive received parts.
    pub(crate) arrival_interval: Duration,
    pub(crate) last_arrival: Option<Instant>,
    /// The window the most recent inbound resource on this link finished on.
    ///
    /// The reference keeps the same thing on the Link itself
    /// (`Link.resource_concluded` stores `resource.window`, and a new inbound
    /// Resource restores it: `previous_window = resource.link
    /// .get_last_resource_window()`, `RNS/Resource.py`). Without it every
    /// resource re-learns a link the previous one just measured, and a file
    /// large enough to be split is many resources, not one. A 46 MB transfer
    /// arrives as 45 segments, so the ladder was restarting from `WINDOW`
    /// forty-five times and spending most of each segment climbing.
    pub(crate) last_window: Option<usize>,
    /// Advertisements this link sent that were dropped for exceeding an
    /// inbound cap. Drives the warn cadence in
    /// `ResourceManager::record_rejected_advertisement`.
    pub(crate) rejected_advertisements: u32,
    /// Request advertisements this link sent that were refused for exceeding
    /// the destination's `max_request_size`. Drives the warn cadence in
    /// `ResourceManager::record_rejected_request_advertisement`.
    pub(crate) rejected_request_advertisements: u32,
}

impl LinkStats {
    pub(crate) fn new() -> Self {
        Self {
            rtt: Duration::from_millis(500),
            arrival_interval: Duration::from_millis(100),
            last_arrival: None,
            last_window: None,
            rejected_advertisements: 0,
            rejected_request_advertisements: 0,
        }
    }

    pub(crate) fn update_rtt(&mut self, sample: Duration) {
        self.rtt = ewma(self.rtt, sample);
    }

    pub(crate) fn record_arrival(&mut self, now: Instant) {
        if let Some(last) = self.last_arrival {
            self.arrival_interval = ewma(self.arrival_interval, now.duration_since(last));
        }
        self.last_arrival = Some(now);
    }
}
