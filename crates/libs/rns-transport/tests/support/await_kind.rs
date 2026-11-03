//! Reads an `interface_events()` stream up to a wanted event; shared by the integration
//! tests that subscribe to a transport's interface events.

use std::time::Duration;

use rns_transport::hash::AddressHash;
use rns_transport::transport::{InterfaceEvent, InterfaceEventKind};
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;
use tokio::time::timeout;

const WAIT: Duration = Duration::from_secs(5);

/// The next event tagged with `iface` whose kind satisfies `wanted`; other events are skipped.
#[deny(clippy::await_holding_invalid_type)]
pub async fn await_kind(
    what: &str,
    events: &mut broadcast::Receiver<InterfaceEvent>,
    iface: AddressHash,
    mut wanted: impl FnMut(InterfaceEventKind) -> bool,
) {
    timeout(WAIT, async {
        loop {
            match events.recv().await {
                Ok(event) if event.iface() == iface && wanted(event.kind()) => return,
                Ok(_) | Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => panic!("{what}: the interface event stream closed"),
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("{what} timed out"))
}
