//! Attaches a transport to a shared instance with a state stream wired in and the attach's
//! `Connecting` already read; shared by the integration tests that drive an attach against
//! a raw listener. The including test binary declares `next_event` alongside this module;
//! declaring it here as well would load that file twice.

use std::time::Duration;

use rns_transport::hash::AddressHash;
use rns_transport::transport::{
    SharedInstanceEndpoint, SharedInstanceEvent, SharedInstanceOptions, Transport,
};
use tokio::sync::mpsc;

use super::next_event::next_event;

const WAIT: Duration = Duration::from_secs(5);
const RECONNECT_WAIT: Duration = Duration::from_millis(50);

/// Attaches `transport` at `endpoint` with `options` plus a 16-slot state stream and a
/// 50 ms reconnect wait, reads the attach's `Connecting`, and returns the stream with the
/// interface address the attach reported.
#[deny(clippy::await_holding_invalid_type)]
pub async fn attach_with_events(
    transport: &Transport,
    endpoint: SharedInstanceEndpoint,
    options: SharedInstanceOptions,
) -> (mpsc::Receiver<SharedInstanceEvent>, AddressHash) {
    let (events_tx, mut events) = mpsc::channel(16);
    let iface = transport
        .attach_shared_instance(
            endpoint,
            options.with_reconnect_wait(RECONNECT_WAIT).with_state_events(events_tx),
        )
        .await
        .expect("the attach is accepted");
    assert_eq!(
        next_event("the attach's Connecting", &mut events, WAIT).await,
        Some(SharedInstanceEvent::Connecting)
    );
    (events, iface)
}
