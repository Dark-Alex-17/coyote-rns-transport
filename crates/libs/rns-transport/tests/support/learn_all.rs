//! Drains an announce receiver until a set of destinations has been learned; shared by the
//! integration tests that watch a burst of announces land.

use std::time::{Duration, Instant};

use rns_transport::hash::AddressHash;
use rns_transport::transport::AnnounceEvent;
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;
use tokio::time::timeout;

/// Waits up to `within` for the announces of every hash in `expected`, in whatever order they
/// arrive (a transport announces its destination map in map order, not registration order);
/// returns the hashes still missing when the window passed. A lagged receiver is skipped
/// because an announce stream may legitimately lag.
#[deny(clippy::await_holding_invalid_type)]
pub async fn learn_all(
    announces: &mut broadcast::Receiver<AnnounceEvent>,
    expected: &[AddressHash],
    within: Duration,
) -> Vec<AddressHash> {
    let mut missing: Vec<AddressHash> = expected.to_vec();
    let deadline = Instant::now() + within;
    while !missing.is_empty() {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        match timeout(remaining, announces.recv()).await {
            Ok(Ok(event)) => {
                let address_hash = event.destination.lock().await.desc.address_hash;
                if let Some(idx) = missing.iter().position(|hash| *hash == address_hash) {
                    eprintln!("learned {address_hash}");
                    missing.swap_remove(idx);
                }
            }
            Ok(Err(RecvError::Lagged(_))) => continue,
            Ok(Err(RecvError::Closed)) => panic!("announce channel closed"),
            Err(_elapsed) => break,
        }
    }
    missing
}
