//! Spawns a plain `TcpClient` through a transport's interface manager; shared by the
//! integration tests that count a client's connects.

use std::net::SocketAddr;
use std::time::Duration;

use rns_transport::hash::AddressHash;
use rns_transport::iface::tcp_client::{TcpClient, TcpRuntimeStatusHandle};
use rns_transport::transport::Transport;

/// Spawns a plain `TcpClient` to `endpoint` through `transport`'s interface manager with
/// `reconnect_wait` between redials; returns its interface address and its status handle.
#[deny(clippy::await_holding_invalid_type)]
pub async fn spawn_plain_client(
    transport: &Transport,
    endpoint: SocketAddr,
    reconnect_wait: Duration,
) -> (AddressHash, TcpRuntimeStatusHandle) {
    let client = TcpClient::new(endpoint.to_string()).with_reconnect_wait(reconnect_wait);
    let status = client.runtime_status_handle();
    let iface = transport.iface_manager().lock().await.spawn(client, TcpClient::spawn);
    (iface, status)
}
