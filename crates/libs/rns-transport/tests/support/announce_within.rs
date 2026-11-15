//! Waits a bounded time for the announce of one destination; shared by the integration tests
//! that check whether a held announce surfaces inside a window.

use std::time::{Duration, Instant};

use rns_transport::hash::AddressHash;
use rns_transport::transport::AnnounceEvent;
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;
use tokio::time::timeout;

/// Waits up to `within` for the announce of `hash`; `Ok(elapsed)` when it arrived, `Err(())`
/// when the window passed without it.
#[deny(clippy::await_holding_invalid_type)]
pub async fn announce_within(
    announces: &mut broadcast::Receiver<AnnounceEvent>,
    hash: &AddressHash,
    within: Duration,
) -> Result<Duration, ()> {
    let started = Instant::now();
    timeout(within, async {
        loop {
            match announces.recv().await {
                Ok(event) => {
                    let address_hash = event.destination.lock().await.desc.address_hash;
                    if address_hash == *hash {
                        return started.elapsed();
                    }
                }
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => panic!("announce channel closed"),
            }
        }
    })
    .await
    .map_err(|_| ())
}
