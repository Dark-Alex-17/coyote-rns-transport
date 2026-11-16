//! Serves one connection from a raw listener and reads the client's `Connected` for it;
//! shared by the integration tests that stand in for the shared instance with a
//! `TcpListener`. The including test binary declares `accept_within` and `next_event`
//! alongside this module; declaring them here as well would load those files twice.

use std::time::Duration;

use rns_transport::transport::SharedInstanceEvent;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;

use super::accept_within::accept_within;
use super::next_event::next_event;

const WAIT: Duration = Duration::from_secs(5);

/// Serves one connection on `listener` and reads `Connected` for it; panics naming `what`
/// after 5 s on either wait.
#[deny(clippy::await_holding_invalid_type)]
pub async fn serve_connected(
    listener: &TcpListener,
    events: &mut mpsc::Receiver<SharedInstanceEvent>,
    what: &str,
) -> TcpStream {
    let stream = accept_within(listener, what).await;
    assert_eq!(
        next_event(what, events, WAIT).await,
        Some(SharedInstanceEvent::Connected),
        "{what}"
    );
    stream
}
