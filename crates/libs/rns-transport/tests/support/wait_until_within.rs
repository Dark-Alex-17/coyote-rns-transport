//! Polls a condition until it holds or a caller-chosen deadline passes; shared by the
//! integration tests whose waits outlast the fixed deadline of `wait_until.rs`.

use std::time::{Duration, Instant};

const POLL: Duration = Duration::from_millis(20);

/// Polls `condition` every 20 ms until it holds; panics naming `what` after `within`.
#[deny(clippy::await_holding_invalid_type)]
pub async fn wait_until_within(what: &str, within: Duration, mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + within;
    while !condition() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        tokio::time::sleep(POLL).await;
    }
}
