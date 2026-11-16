//! Waits for the announce of one destination on an announce receiver; shared by the
//! integration tests that watch a local client learn its peer through a shared instance.

use std::time::Duration;

use rns_transport::hash::AddressHash;
use rns_transport::transport::AnnounceEvent;
use tokio::sync::broadcast::{self, error::RecvError};
use tokio::time::timeout;

/// The announce event for `hash` within `within`, skipping other destinations and a lagged
/// receiver (an announce stream may legitimately lag); panics naming `what` on a closed
/// channel or after `within`.
#[deny(clippy::await_holding_invalid_type)]
pub async fn learn_announce(
    what: &str,
    announces: &mut broadcast::Receiver<AnnounceEvent>,
    hash: &AddressHash,
    within: Duration,
) -> AnnounceEvent {
    timeout(within, async {
        loop {
            match announces.recv().await {
                Ok(event) => {
                    let address_hash = event.destination.lock().await.desc.address_hash;
                    if address_hash == *hash {
                        return event;
                    }
                }
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => panic!("{what}: announce channel closed"),
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("{what}: the announce did not arrive"))
}
