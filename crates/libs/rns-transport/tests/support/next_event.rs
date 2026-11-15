//! Reads the next shared-instance state event within a caller-chosen deadline; shared by
//! the integration tests that follow an attach's state stream.

use std::time::Duration;

use rns_transport::transport::SharedInstanceEvent;
use tokio::sync::mpsc;
use tokio::time::timeout;

/// The next event on `events`, or `None` once the stream has ended; panics naming `what`
/// after `within`.
#[deny(clippy::await_holding_invalid_type)]
pub async fn next_event(
    what: &str,
    events: &mut mpsc::Receiver<SharedInstanceEvent>,
    within: Duration,
) -> Option<SharedInstanceEvent> {
    timeout(within, events.recv()).await.unwrap_or_else(|_| panic!("{what} timed out"))
}
