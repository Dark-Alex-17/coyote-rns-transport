use std::time::Duration;

use crate::hash::AddressHash;

/// `RNS.Reticulum.MTU` (`RNS/Reticulum.py:93`): the fixed MTU Python sizes
/// per-hop timeouts with, whatever the interface's hardware MTU is. The same
/// 500 bytes the link layer derives from the packet layout.
pub(super) const RETICULUM_MTU: usize = 500;
const _: () =
    assert!(RETICULUM_MTU == crate::destination::link::LEGACY_RETICULUM_MTU, "one Reticulum MTU");
const RETICULUM_MTU_BYTES: f64 = RETICULUM_MTU as f64;

pub(super) fn medium_path_timeout_for_bitrate(bitrate: Option<u64>) -> Duration {
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

    /// `Transport.first_hop_timeout` (`RNS/Transport.py:3194-3197`): Python
    /// passes the fixed `Reticulum.MTU` of 500 bytes as `mtu`, never the
    /// interface's hardware MTU, which `hardware_mtu` only reports.
    pub fn first_hop_timeout(self, mtu: usize, default_timeout: Duration) -> Duration {
        self.per_byte_latency()
            .map(|latency| default_timeout + Duration::from_secs_f64(mtu as f64 * latency))
            .unwrap_or(default_timeout)
    }

    /// `Transport.extra_link_proof_timeout` (`RNS/Transport.py:3200-3202`):
    /// Python passes the fixed `Reticulum.MTU` of 500 bytes as `mtu` here too.
    pub fn extra_link_proof_timeout(self, mtu: usize) -> Duration {
        self.per_byte_latency()
            .map(|latency| Duration::from_secs_f64(mtu as f64 * latency))
            .unwrap_or_default()
    }
}
