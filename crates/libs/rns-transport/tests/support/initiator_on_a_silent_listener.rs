//! Connects a non-transport initiator's plain client to a loopback listener that never
//! answers; shared by the integration tests that watch a pending link from the wire.

use std::time::Duration;

use rns_transport::identity::PrivateIdentity;
use rns_transport::rand_core::OsRng;
use rns_transport::transport::{Transport, TransportConfig};
use tokio::net::{TcpListener, TcpStream};

#[path = "accept_within.rs"]
mod accept_within;
#[path = "spawn_plain_client.rs"]
mod spawn_plain_client;
#[path = "wait_until.rs"]
mod wait_until;
use accept_within::accept_within;
use spawn_plain_client::spawn_plain_client;
use wait_until::wait_until;

const RECONNECT_WAIT: Duration = Duration::from_secs(60);

/// A non-transport initiator with broadcast on, its plain `TcpClient` connected to a raw
/// loopback listener that never answers; returns the transport and the accepted stream.
#[deny(clippy::await_holding_invalid_type)]
pub async fn initiator_on_a_silent_listener(name: &str) -> (Transport, TcpStream) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("a loopback listener binds");
    let addr = listener.local_addr().expect("the listener has an address");
    let transport =
        Transport::new(TransportConfig::new(name, &PrivateIdentity::new_from_rand(OsRng), true));
    let (_iface, status) = spawn_plain_client(&transport, addr, RECONNECT_WAIT).await;
    let stream = accept_within(&listener, "the client's connect").await;
    wait_until("the client to count its stream", || status.connected_streams() >= 1).await;
    (transport, stream)
}
