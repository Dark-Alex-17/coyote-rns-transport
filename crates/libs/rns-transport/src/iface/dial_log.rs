//! Fork-only: the log cadence for a stream client's failed connect
//! attempts. A peer that stays down would otherwise warn once per
//! `reconnect_wait`, so the TCP and local unix clients count their failed
//! dials here and warn on the first, on a change of failure kind and on
//! every `DIAL_WARN_STRIDE`th, logging the attempts between at debug. The
//! `DialFailed` events and the dial-failure channels still fire on every
//! attempt; only the log line's level moves. Python logs every failed
//! attempt at debug (`RNS/Interfaces/TCPInterface.py:288`,
//! `RNS/Interfaces/LocalInterface.py:159-167`), so this is a divergence
//! recorded under the README's known divergences.

use super::events::DialFailureKind;

/// A failed dial warns on the first attempt, whenever the failure kind
/// changes, and on every multiple of this count; the attempts between log
/// at debug. At a 2 s `reconnect_wait` a peer that is down warns at once
/// and then once a minute; at the clients' 5 s default, every 150 s.
pub(crate) const DIAL_WARN_STRIDE: u64 = 30;

/// The failed `connect()` attempts since the last established stream, and
/// the kind the last one failed with. The count is cleared at the success
/// arm, before the TCP client's `MIN_STABLE_CONNECTION` has proven the
/// stream, so it is not `TcpRuntimeStatus::reconnect_attempts`: the
/// short-stream path bumps that without a failed connect, and a flapping
/// peer re-arms this cadence at every connect.
#[derive(Debug, Default)]
pub(crate) struct DialAttempts {
    failed: u64,
    last_kind: Option<DialFailureKind>,
}

impl DialAttempts {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Counts one failed attempt and picks its log level: `Warn` on the
    /// first failure since the last established stream, whenever the kind
    /// differs from the previous failure's, and on every
    /// `DIAL_WARN_STRIDE`th failure; `Debug` otherwise.
    pub(crate) fn record_failure(&mut self, kind: DialFailureKind) -> log::Level {
        self.failed = self.failed.saturating_add(1);
        let kind_changed = self.last_kind != Some(kind);
        self.last_kind = Some(kind);
        if self.failed == 1 || kind_changed || self.failed.is_multiple_of(DIAL_WARN_STRIDE) {
            log::Level::Warn
        } else {
            log::Level::Debug
        }
    }

    /// Clears the count at an established stream and returns the failures
    /// it took to get there (0 when the first attempt connected).
    pub(crate) fn connected(&mut self) -> u64 {
        self.last_kind = None;
        core::mem::take(&mut self.failed)
    }

    /// The failed attempts since the last established stream (the value
    /// the `(attempt N)` suffix prints).
    pub(crate) fn failed(&self) -> u64 {
        self.failed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use log::Level::{Debug, Warn};

    fn levels(attempts: &mut DialAttempts, kinds: &[DialFailureKind]) -> Vec<log::Level> {
        kinds.iter().map(|kind| attempts.record_failure(*kind)).collect()
    }

    #[test]
    fn the_first_failure_warns_and_the_same_kind_then_debugs() {
        let mut attempts = DialAttempts::new();
        assert_eq!(attempts.record_failure(DialFailureKind::Refused), Warn);
        assert_eq!(attempts.failed(), 1);
        for attempt in 2..DIAL_WARN_STRIDE {
            assert_eq!(
                attempts.record_failure(DialFailureKind::Refused),
                Debug,
                "attempt {attempt}"
            );
            assert_eq!(attempts.failed(), attempt);
        }
    }

    #[test]
    fn a_change_of_kind_warns_at_once_and_the_new_kind_then_debugs() {
        let mut attempts = DialAttempts::new();
        let seen = levels(
            &mut attempts,
            &[
                DialFailureKind::Refused,
                DialFailureKind::Refused,
                DialFailureKind::NotFound,
                DialFailureKind::NotFound,
            ],
        );
        assert_eq!(seen, [Warn, Debug, Warn, Debug]);
    }

    #[test]
    fn every_stride_th_failure_warns() {
        let mut attempts = DialAttempts::new();
        let stride = DIAL_WARN_STRIDE;
        for attempt in 1..=(2 * stride) {
            let level = attempts.record_failure(DialFailureKind::Timeout);
            let expected = if attempt == 1 || attempt == stride || attempt == 2 * stride {
                Warn
            } else {
                Debug
            };
            assert_eq!(level, expected, "attempt {attempt}");
        }
        assert_eq!(attempts.failed(), 2 * stride);
    }

    #[test]
    fn connected_returns_the_cleared_count_and_rearms_the_first_warn() {
        let mut attempts = DialAttempts::new();
        assert_eq!(attempts.connected(), 0, "no failures before the first connect");
        levels(
            &mut attempts,
            &[DialFailureKind::Refused, DialFailureKind::Refused, DialFailureKind::Refused],
        );
        assert_eq!(attempts.connected(), 3);
        assert_eq!(attempts.failed(), 0);
        assert_eq!(attempts.record_failure(DialFailureKind::Refused), Warn, "re-armed");
        assert_eq!(attempts.failed(), 1);
    }
}
