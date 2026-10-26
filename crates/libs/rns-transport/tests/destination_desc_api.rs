//! The README's `destination_desc` snippet, verbatim, run from outside the crate: a
//! listener with a `TcpServer` on 127.0.0.1 announces `example/echo`, a connector with a
//! `TcpClient` learns the announce, resolves the desc, and opens a link with it. Around
//! the snippet, the claims a first-time consumer reads next: `destination_identity` is
//! `Some` for the same address, and the link the desc opens activates on the announcing
//! side under the same id.

use std::error::Error;
use std::sync::Arc;
use std::time::{Duration, Instant};

use rns_transport::destination::link::{Link, LinkEvent, LinkEventData};
use rns_transport::destination::DestinationName;
use rns_transport::hash::AddressHash;
use rns_transport::identity::PrivateIdentity;
use rns_transport::iface::tcp_client::TcpClient;
use rns_transport::iface::tcp_server::{TcpListenerRuntimeStatusHandle, TcpServer};
use rns_transport::rand_core::OsRng;
use rns_transport::transport::{AnnounceEvent, Transport, TransportConfig};
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::{broadcast, Mutex};
use tokio::time::timeout;

const WAIT: Duration = Duration::from_secs(15);
const POLL: Duration = Duration::from_millis(20);

/// The announcing side: a transport with a `TcpServer` on 127.0.0.1.
struct Listener {
    transport: Transport,
    port: u16,
    status: TcpListenerRuntimeStatusHandle,
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
async fn free_loopback_port() -> u16 {
    let probe = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind probe");
    let port = probe.local_addr().expect("probe address").port();
    drop(probe);
    port
}

#[deny(clippy::await_holding_invalid_type)]
async fn listener() -> Listener {
    let transport = Transport::new(TransportConfig::new(
        "readme-destination-desc-listener",
        &PrivateIdentity::new_from_rand(OsRng),
        false,
    ));
    let port = free_loopback_port().await;
    let server = TcpServer::new(format!("127.0.0.1:{port}"), transport.iface_manager());
    let status = server.runtime_status_handle();
    transport.iface_manager().lock().await.spawn(server, TcpServer::spawn);
    wait_until("the listener to listen", || {
        status.to_json()["listener_state"].as_str() == Some("listening")
    })
    .await;
    Listener { transport, port, status }
}

/// The connecting side: a transport with a `TcpClient` on the listener's port. The
/// announce receiver is subscribed before the stream is up so the announce cannot be missed.
#[deny(clippy::await_holding_invalid_type)]
async fn connector(port: u16) -> (Transport, broadcast::Receiver<AnnounceEvent>) {
    let transport = Transport::new(TransportConfig::new(
        "readme-destination-desc-connector",
        &PrivateIdentity::new_from_rand(OsRng),
        false,
    ));
    let announces = transport.recv_announces().await;
    let client = TcpClient::new(format!("127.0.0.1:{port}"));
    let status = client.runtime_status_handle();
    let context = transport.iface_manager().lock().await.new_context(client);
    tokio::spawn(TcpClient::spawn(context));
    wait_until("the connector to connect", || {
        status.to_json()["stream_state"].as_str() == Some("connected")
    })
    .await;
    (transport, announces)
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

#[deny(clippy::await_holding_invalid_type)]
async fn await_activated(events: &mut broadcast::Receiver<LinkEventData>) -> LinkEventData {
    timeout(WAIT, async {
        loop {
            match events.recv().await {
                Ok(event) if matches!(event.event, LinkEvent::Activated) => return event,
                Ok(_) => {}
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => panic!("link event channel closed"),
            }
        }
    })
    .await
    .expect("the link activates on the announcing side")
}

/// The README `destination_desc` snippet, verbatim.
#[deny(clippy::await_holding_invalid_type)]
async fn readme_destination_desc_and_link(
    transport: &Transport,
    destination_hash: AddressHash,
) -> Result<Arc<Mutex<Link>>, Box<dyn Error + Send + Sync>> {
    let desc = transport
        .destination_desc(&destination_hash)
        .await
        .ok_or("the announce has not been learned yet")?;
    assert_eq!(
        desc.name.as_name_hash_slice(),
        DestinationName::new("example", "echo").as_name_hash_slice(),
    );
    let link = transport.link(desc).await?;
    Ok(link)
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn readme_snippet_resolves_the_desc_and_opens_a_link() {
    let listener = listener().await;
    let (connector, mut announces) = connector(listener.port).await;
    // The accepted child interface that carries the listener's traffic is registered a
    // task wake after the connector reports its stream connected.
    wait_until("the listener to accept", || {
        listener.status.to_json()["accepted_connections"].as_u64() == Some(1)
    })
    .await;
    let mut in_events = listener.transport.in_link_events();

    let destination = listener
        .transport
        .add_destination(
            PrivateIdentity::new_from_rand(OsRng),
            DestinationName::new("example", "echo"),
        )
        .await;
    let destination_hash = destination.lock().await.desc.address_hash;
    let packet = destination.lock().await.announce(OsRng, None).expect("valid announce packet");
    listener.transport.send_packet(packet).await;
    learn_announce(&mut announces, &destination_hash).await;

    let link = readme_destination_desc_and_link(&connector, destination_hash)
        .await
        .expect("the README snippet");
    assert!(
        connector.destination_identity(&destination_hash).await.is_some(),
        "destination_identity is Some whenever destination_desc is Some"
    );
    let link_id = *link.lock().await.id();
    let activated = await_activated(&mut in_events).await;
    assert_eq!(activated.id, link_id, "both ends share one link id");

    timeout(WAIT, connector.stop()).await.expect("connector stop completes");
    timeout(WAIT, listener.transport.stop()).await.expect("listener stop completes");
}
