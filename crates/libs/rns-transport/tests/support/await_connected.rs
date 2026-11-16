//! Reads a shared-instance state stream until `Connected`; shared by the integration tests
//! that follow a local client through the shared instance going away and coming back.

use std::time::{Duration, Instant};

use rns_transport::transport::SharedInstanceEvent;
use tokio::sync::mpsc;
use tokio::time::timeout;

/// Reads `events` until `Connected`, skipping `Connecting` and `DialFailed`: a client that
/// redials at once while the port is still closed reports those first, which is the stream
/// doing its job, not a failure of the reconnect. Panics naming `what` on any other event,
/// on the stream ending, or after `within`.
#[deny(clippy::await_holding_invalid_type)]
pub async fn await_connected(
    what: &str,
    events: &mut mpsc::Receiver<SharedInstanceEvent>,
    within: Duration,
) {
    let deadline = Instant::now() + within;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        match timeout(remaining, events.recv()).await {
            Ok(Some(SharedInstanceEvent::Connected)) => return,
            Ok(Some(SharedInstanceEvent::Connecting | SharedInstanceEvent::DialFailed { .. })) => {}
            Ok(Some(other)) => panic!("{what}: unexpected {other:?} before `Connected`"),
            Ok(None) => panic!("{what}: the state stream ended before `Connected`"),
            Err(_) => panic!("{what}: `Connected` timed out"),
        }
    }
}
