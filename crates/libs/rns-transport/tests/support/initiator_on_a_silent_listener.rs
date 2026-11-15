//! Connects a non-transport initiator's plain client to a loopback listener that never
//! answers; shared by the integration tests that watch a pending link from the wire. The
//! including test binary declares `accept_within`, `broadcasting`, `spawn_plain_client`
//! and `wait_until` alongside this module; declaring them here as well would load those
//! files twice.

use std::time::Duration;

use rns_transport::transport::Transport;
use tokio::net::{TcpListener, TcpStream};

use super::accept_within::accept_within;
use super::broadcasting::broadcasting;
use super::spawn_plain_client::spawn_plain_client;
use super::wait_until::wait_until;

const RECONNECT_WAIT: Duration = Duration::from_secs(60);

/// A non-transport initiator with broadcast on, its plain `TcpClient` connected to a raw
/// loopback listener that never answers; returns the transport and the accepted stream.
#[deny(clippy::await_holding_invalid_type)]
pub async fn initiator_on_a_silent_listener(name: &str) -> (Transport, TcpStream) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("a loopback listener binds");
    let addr = listener.local_addr().expect("the listener has an address");
    let transport = broadcasting(name);
    let (_iface, status) = spawn_plain_client(&transport, addr, RECONNECT_WAIT).await;
    let stream = accept_within(&listener, "the client's connect").await;
    wait_until("the client to count its stream", || status.connected_streams() >= 1).await;
    (transport, stream)
}
