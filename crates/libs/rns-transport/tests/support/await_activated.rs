//! Waits for one link's `Activated` event on a link event receiver; shared by the
//! integration tests that open links between local clients of a shared instance.

use std::time::Duration;

use rns_transport::destination::link::{LinkEvent, LinkEventData};
use rns_transport::hash::AddressHash;
use tokio::sync::broadcast::{self, error::RecvError};
use tokio::time::timeout;

/// Returns once `events` carries `Activated` for `link_id`, skipping other events and a
/// lagged receiver (a link event stream may legitimately lag); panics naming `what` on a
/// closed channel or after `within`.
#[deny(clippy::await_holding_invalid_type)]
pub async fn await_activated(
    events: &mut broadcast::Receiver<LinkEventData>,
    link_id: AddressHash,
    what: &str,
    within: Duration,
) {
    timeout(within, async {
        loop {
            match events.recv().await {
                Ok(event) if event.id == link_id && matches!(event.event, LinkEvent::Activated) => {
                    return;
                }
                Ok(_) => {}
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => panic!("{what}: link event channel closed"),
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("timed out waiting for {what}"))
}
