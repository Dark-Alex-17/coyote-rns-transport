//! Waits for one link's `Closed` event on an out-link event receiver; shared by the
//! integration tests that pin when and why a pending link is closed.

use std::time::Duration;

use rns_transport::destination::link::{LinkEvent, LinkEventData, LinkId};
use tokio::sync::broadcast;
use tokio::time::timeout;

/// The `Closed` event for `id` within `within`, skipping anything else first.
#[deny(clippy::await_holding_invalid_type)]
pub async fn closed_event_for(
    events: &mut broadcast::Receiver<LinkEventData>,
    id: LinkId,
    within: Duration,
) -> LinkEventData {
    let deadline = tokio::time::Instant::now() + within;
    loop {
        let left = deadline.saturating_duration_since(tokio::time::Instant::now());
        let event = timeout(left, events.recv())
            .await
            .expect("the close publishes an out-link event")
            .expect("the out-link event stream remains open");
        if event.id == id && matches!(event.event, LinkEvent::Closed) {
            return event;
        }
    }
}
