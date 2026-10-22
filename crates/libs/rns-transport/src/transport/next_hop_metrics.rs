use std::time::Duration;

use crate::hash::AddressHash;

pub(super) fn medium_path_timeout_for_bitrate(bitrate: Option<u64>) -> Duration {
    const RETICULUM_MTU_BYTES: f64 = 500.0;
    const MINIMUM_BITRATE: u64 = 5;
    const DEFAULT_PER_HOP_TIMEOUT_SECS: f64 = 6.0;

    let Some(bitrate) = bitrate else { return Duration::ZERO };
    Duration::from_secs_f64(
        2.0 * (RETICULUM_MTU_BYTES * 8.0 / bitrate.max(MINIMUM_BITRATE) as f64)
            + DEFAULT_PER_HOP_TIMEOUT_SECS,
    )
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NextHopMetrics {
    pub interface: AddressHash,
    pub bitrate: u64,
    pub hardware_mtu: Option<usize>,
    pub per_bit_latency: Option<f64>,
}

impl NextHopMetrics {
    pub fn per_byte_latency(self) -> Option<f64> {
        self.per_bit_latency.map(|latency| latency * 8.0)
    }

    pub fn first_hop_timeout(self, mtu: usize, default_timeout: Duration) -> Duration {
        self.per_byte_latency()
            .map(|latency| default_timeout + Duration::from_secs_f64(mtu as f64 * latency))
            .unwrap_or(default_timeout)
    }

    pub fn extra_link_proof_timeout(self, mtu: usize) -> Duration {
        self.per_byte_latency()
            .map(|latency| Duration::from_secs_f64(mtu as f64 * latency))
            .unwrap_or_default()
    }
}
