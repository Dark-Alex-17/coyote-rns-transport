//! Polls `Transport::interface_connect_count` until it reads an expected value; shared by
//! the integration tests that count a stream client's connects.

use std::time::{Duration, Instant};

use rns_transport::hash::AddressHash;
use rns_transport::transport::Transport;

const WAIT: Duration = Duration::from_secs(5);
const POLL: Duration = Duration::from_millis(20);

/// Polls `transport.interface_connect_count(iface)` until it reads `expected`.
#[deny(clippy::await_holding_invalid_type)]
pub async fn wait_for_count(what: &str, transport: &Transport, iface: AddressHash, expected: u64) {
    let deadline = Instant::now() + WAIT;
    loop {
        let count = transport.interface_connect_count(&iface).await;
        if count == Some(expected) {
            return;
        }
        assert!(Instant::now() < deadline, "timed out waiting for {what}: read {count:?}");
        tokio::time::sleep(POLL).await;
    }
}
