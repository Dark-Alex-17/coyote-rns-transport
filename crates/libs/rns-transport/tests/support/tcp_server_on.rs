//! A node listening with a `TcpServer` on a chosen loopback port, optionally flagged as the
//! shared instance; shared by the integration tests that stand up a shared instance or an
//! ordinary listener over TCP. The including test binary declares `wait_until_within`
//! alongside this module; declaring it here as well would load that file twice.

use std::time::Duration;

use rns_transport::hash::AddressHash;
use rns_transport::identity::PrivateIdentity;
use rns_transport::iface::tcp_server::TcpServer;
use rns_transport::rand_core::OsRng;
use rns_transport::transport::{Transport, TransportConfig};

use super::wait_until_within::wait_until_within;

const WAIT: Duration = Duration::from_secs(15);

/// A node with a `TcpServer` on `127.0.0.1:<port>`, flagged as the shared instance when asked
/// (rnsd's topology, which also needs `transport_enabled`) and an ordinary listener
/// otherwise; returns the listener's interface hash as well.
#[deny(clippy::await_holding_invalid_type)]
pub async fn tcp_server_on(
    name: &str,
    port: u16,
    shared_instance: bool,
) -> (Transport, AddressHash) {
    let mut config = TransportConfig::new(name, &PrivateIdentity::new_from_rand(OsRng), false);
    config.set_transport_enabled(shared_instance);
    let transport = Transport::new(config);
    let server = TcpServer::new(format!("127.0.0.1:{port}"), transport.iface_manager());
    let status = server.runtime_status_handle();
    let listener = transport.iface_manager().lock().await.spawn(server, TcpServer::spawn);
    if shared_instance {
        assert!(
            transport.iface_manager().lock().await.set_shared_instance(listener, true),
            "the listener is flagged as the shared instance"
        );
    }
    wait_until_within("the listener to listen", WAIT, || {
        status.to_json()["listener_state"].as_str() == Some("listening")
    })
    .await;
    (transport, listener)
}
