//! Polls a condition until it holds or a fixed deadline passes; shared by the integration
//! tests that poll a condition, be it a status handle or a captured log.

use std::time::{Duration, Instant};

const WAIT: Duration = Duration::from_secs(5);
const POLL: Duration = Duration::from_millis(20);

/// Polls `condition` every 20 ms until it holds; panics naming `what` after 5 s.
#[deny(clippy::await_holding_invalid_type)]
pub async fn wait_until(what: &str, mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + WAIT;
    while !condition() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        tokio::time::sleep(POLL).await;
    }
}
