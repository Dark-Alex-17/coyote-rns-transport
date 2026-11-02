//! `delivery::await_link_activation` under `Duration::MAX` still resolves, measured from
//! outside the crate: the saturated deadline waits for activation instead of either
//! panicking or freezing the wait. Two transports meet over loopback TCP, the listener
//! announces a destination, the connector opens a link to it, and an unbounded wait on
//! that link returns `Ok(())` once the link activates, well inside the test's own bound.
//! `tests/link_activation_deadline_api.rs` pins the negative half (a pending link never
//! returns); this is its positive control.

use std::time::{Duration, Instant};

use rns_transport::delivery::await_link_activation;
use rns_transport::destination::link::LinkStatus;
use rns_transport::destination::DestinationName;
use rns_transport::hash::AddressHash;
use rns_transport::identity::PrivateIdentity;
use rns_transport::iface::tcp_client::TcpClient;
use rns_transport::iface::tcp_server::TcpServer;
use rns_transport::rand_core::OsRng;
use rns_transport::transport::{AnnounceEvent, Transport, TransportConfig};
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;
use tokio::time::timeout;

#[path = "support/free_loopback_port.rs"]
mod free_loopback_port;
use free_loopback_port::free_loopback_port;

const WAIT: Duration = Duration::from_secs(15);
const POLL: Duration = Duration::from_millis(20);

fn transport(name: &str) -> Transport {
    Transport::new(TransportConfig::new(name, &PrivateIdentity::new_from_rand(OsRng), false))
}

#[deny(clippy::await_holding_invalid_type)]
async fn wait_until(what: &str, mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + WAIT;
    while !condition() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        tokio::time::sleep(POLL).await;
    }
}

#[deny(clippy::await_holding_invalid_type)]
async fn learn_announce(announces: &mut broadcast::Receiver<AnnounceEvent>, hash: &AddressHash) {
    timeout(WAIT, async {
        loop {
            match announces.recv().await {
                Ok(event) => {
                    if event.destination.lock().await.desc.address_hash == *hash {
                        return;
                    }
                }
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => panic!("announce channel closed"),
            }
        }
    })
    .await
    .expect("the announce arrives")
}

/// An unbounded wait on a link that does activate returns `Ok(())` when it does, and the
/// link reads `Active` afterwards.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn max_timeout_resolves_when_the_link_activates() {
    let listener = transport("link-activation-max-listener");
    let port = free_loopback_port().await;
    let server = TcpServer::new(format!("127.0.0.1:{port}"), listener.iface_manager());
    let server_status = server.runtime_status_handle();
    listener.iface_manager().lock().await.spawn(server, TcpServer::spawn);
    wait_until("the listener to listen", || {
        server_status.to_json()["listener_state"].as_str() == Some("listening")
    })
    .await;

    let connector = transport("link-activation-max-connector");
    let mut announces = connector.recv_announces().await;
    let client = TcpClient::new(format!("127.0.0.1:{port}"));
    let client_status = client.runtime_status_handle();
    connector.iface_manager().lock().await.spawn(client, TcpClient::spawn);
    wait_until("the connector to connect", || {
        client_status.to_json()["stream_state"].as_str() == Some("connected")
    })
    .await;
    wait_until("the listener to accept", || {
        server_status.to_json()["accepted_connections"].as_u64() == Some(1)
    })
    .await;

    let destination = listener
        .add_destination(
            PrivateIdentity::new_from_rand(OsRng),
            DestinationName::new("example", "activation-max"),
        )
        .await;
    let destination_hash = destination.lock().await.desc.address_hash;
    let announce = destination.lock().await.announce(OsRng, None).expect("valid announce packet");
    listener.send_packet(announce).await;
    learn_announce(&mut announces, &destination_hash).await;

    let desc = connector.destination_desc(&destination_hash).await.expect("the desc was learned");
    let link = connector.link(desc).await.expect("the link opens");

    let started = Instant::now();
    timeout(WAIT, await_link_activation(&connector, &link, Duration::MAX))
        .await
        .expect("an unbounded wait on an activating link returns before the test's bound")
        .expect("the wait ends in activation, not an error");
    assert_eq!(link.lock().await.status(), LinkStatus::Active, "the link is active afterwards");
    assert!(
        started.elapsed() < WAIT,
        "the unbounded wait returned on activation, after {:?}",
        started.elapsed()
    );

    // A second unbounded wait on the now-active link returns at once.
    timeout(Duration::from_secs(1), await_link_activation(&connector, &link, Duration::MAX))
        .await
        .expect("an already-active link answers at once")
        .expect("without error");

    timeout(WAIT, connector.stop()).await.expect("connector stop completes");
    timeout(WAIT, listener.stop()).await.expect("listener stop completes");
}
