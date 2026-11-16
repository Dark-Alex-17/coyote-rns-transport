//! Sends one context-free announce for a destination; shared by the integration tests that
//! drive the receiving side's announce limiter.

use std::sync::Arc;

use rns_transport::destination::SingleInputDestination;
use rns_transport::rand_core::OsRng;
use rns_transport::transport::Transport;
use tokio::sync::Mutex;

/// Announces `destination` on `transport` with no packet context, so the receiving limiter
/// weighs it; a path-response announce is admitted before the limiter looks at the
/// interface.
#[deny(clippy::await_holding_invalid_type)]
pub async fn announce(transport: &Transport, destination: &Arc<Mutex<SingleInputDestination>>) {
    let packet = destination.lock().await.announce(OsRng, None).expect("announce packet");
    transport.send_packet(packet).await;
}
