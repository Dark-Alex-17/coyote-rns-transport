//! A run of dial failures read off a client's dial-failure channel; shared by the
//! integration tests that drive a client against an endpoint nothing listens on.

use std::time::Duration;

use rns_transport::hash::AddressHash;
use rns_transport::transport::{DialFailure, DialFailureKind};
use tokio::sync::mpsc;
use tokio::time::timeout;

/// `count` failures of `kind` on `iface`, one per attempt.
#[deny(clippy::await_holding_invalid_type)]
pub async fn recv_failures(
    rx: &mut mpsc::Receiver<DialFailure>,
    iface: AddressHash,
    kind: DialFailureKind,
    count: usize,
) {
    for attempt in 1..=count {
        let failure = timeout(Duration::from_secs(3), rx.recv())
            .await
            .unwrap_or_else(|_| panic!("dial failure {attempt} timed out"))
            .unwrap_or_else(|| panic!("dial failure {attempt} channel closed"));
        assert_eq!(failure.iface(), iface, "the failure names the interface");
        assert_eq!(failure.kind(), kind, "attempt {attempt}");
    }
}
