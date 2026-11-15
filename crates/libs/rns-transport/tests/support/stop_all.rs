//! Stops a set of transports in order, each within a fixed deadline; shared by the
//! integration tests that tear down several nodes at the end.

use std::time::Duration;

use rns_transport::transport::Transport;
use tokio::time::timeout;

const WAIT: Duration = Duration::from_secs(15);

/// Stops each transport in turn; panics when one does not stop within 15 s.
#[deny(clippy::await_holding_invalid_type)]
pub async fn stop_all(transports: Vec<Transport>) {
    for transport in transports {
        timeout(WAIT, transport.stop()).await.expect("stop completes");
    }
}
