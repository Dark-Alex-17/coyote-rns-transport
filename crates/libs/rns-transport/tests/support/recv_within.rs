//! The next item of a channel, or a panic naming what never arrived; shared by the
//! integration tests that await a client's events.

use std::time::Duration;

use tokio::sync::mpsc;
use tokio::time::timeout;

/// The next item of `rx` within a 2 s deadline.
#[deny(clippy::await_holding_invalid_type)]
pub async fn recv_within<T>(what: &str, rx: &mut mpsc::Receiver<T>) -> T {
    timeout(Duration::from_secs(2), rx.recv())
        .await
        .unwrap_or_else(|_| panic!("{what} timed out"))
        .unwrap_or_else(|| panic!("{what} channel closed"))
}
