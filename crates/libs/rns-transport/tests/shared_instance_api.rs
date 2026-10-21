//! The README's "Attaching to a shared instance" snippet, verbatim, run from outside the
//! crate against an in-process shared instance: a transport-enabled node with a `TcpServer`
//! on 127.0.0.1, the posture reticulumd takes for its own clients. Around the snippet, the
//! claims a first-time consumer reads next: the attach is reported by `shared_instance()`
//! and `connected_to_shared_instance()`, the on-demand announce reaches the shared instance
//! as a path response at one hop, and the detach releases the interface and the posture.

use std::error::Error;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use rns_transport::destination::request::{InboundRequest, RequestAllow, RequestResponse};
use rns_transport::destination::{DestinationName, SingleInputDestination};
use rns_transport::hash::AddressHash;
use rns_transport::identity::PrivateIdentity;
use rns_transport::iface::tcp_server::TcpServer;
use rns_transport::rand_core::OsRng;
use rns_transport::transport::{
    AnnounceEvent, SharedInstanceEndpoint, SharedInstanceOptions, Transport, TransportConfig,
};
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::{broadcast, Mutex};
use tokio::time::timeout;

const WAIT: Duration = Duration::from_secs(15);
const POLL: Duration = Duration::from_millis(20);

/// The shared instance: a transport-enabled node with a `TcpServer` on 127.0.0.1.
struct SharedInstance {
    transport: Transport,
    endpoint: SocketAddr,
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
async fn shared_instance() -> SharedInstance {
    let mut config = TransportConfig::new(
        "readme-shared-instance",
        &PrivateIdentity::new_from_rand(OsRng),
        false,
    );
    config.set_transport_enabled(true);
    let transport = Transport::new(config);
    let port = free_loopback_port().await;
    let server = TcpServer::new(format!("127.0.0.1:{port}"), transport.iface_manager());
    let status = server.runtime_status_handle();
    transport.iface_manager().lock().await.spawn(server, TcpServer::spawn);
    wait_until("the shared instance to listen", || {
        status.to_json()["listener_state"].as_str() == Some("listening")
    })
    .await;
    SharedInstance { transport, endpoint: SocketAddr::from(([127, 0, 0, 1], port)) }
}

#[deny(clippy::await_holding_invalid_type)]
async fn learn_announce(
    announces: &mut broadcast::Receiver<AnnounceEvent>,
    hash: &AddressHash,
) -> AnnounceEvent {
    timeout(WAIT, async {
        loop {
            match announces.recv().await {
                Ok(event) => {
                    let address_hash = event.destination.lock().await.desc.address_hash;
                    if address_hash == *hash {
                        return event;
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

/// The README "Attaching to a shared instance" snippet up to its announce, verbatim; the
/// snippet's detach line follows in the test, once the shared instance has learned the
/// announce.
#[deny(clippy::await_holding_invalid_type)]
async fn readme_attach_and_announce(
    transport: &Transport,
    endpoint: SocketAddr,
) -> Result<(AddressHash, Arc<Mutex<SingleInputDestination>>), Box<dyn Error + Send + Sync>> {
    let iface = transport
        .attach_shared_instance(
            SharedInstanceEndpoint::Tcp(endpoint),
            SharedInstanceOptions::default(),
        )
        .await?;
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let destination =
        transport.add_destination(identity, DestinationName::new("example", "echo")).await;
    destination.lock().await.register_request_handler(
        "/echo",
        Arc::new(|request: InboundRequest| async move {
            Some(RequestResponse::Value(request.into_data()))
        }),
        RequestAllow::All,
        true,
    )?;
    let announced = transport.announce_destinations_to_shared_instance().await;
    assert_eq!(announced, 1);
    Ok((iface, destination))
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn readme_snippet_attaches_announces_and_detaches() {
    let shared = shared_instance().await;
    let mut announces = shared.transport.recv_announces().await;
    let transport = Transport::new(TransportConfig::new(
        "readme-shared-instance-client",
        &PrivateIdentity::new_from_rand(OsRng),
        false,
    ));

    let (iface, destination) =
        readme_attach_and_announce(&transport, shared.endpoint).await.expect("the README snippet");
    assert_eq!(transport.shared_instance(), Some(iface), "the slot names the attached iface");
    assert!(transport.connected_to_shared_instance().await, "the client posture is set");
    assert!(transport.interface_hashes().await.contains(&iface), "the iface is registered");

    let hash = destination.lock().await.desc.address_hash;
    let event = learn_announce(&mut announces, &hash).await;
    assert!(
        event.is_path_response,
        "the shared instance learns the destination as a path response"
    );
    assert_eq!(event.hops, 1, "at one hop");

    assert!(transport.detach_shared_instance().await);
    assert_eq!(transport.shared_instance(), None, "the slot is cleared");
    assert!(!transport.connected_to_shared_instance().await, "the client posture is cleared");
    assert!(!transport.interface_hashes().await.contains(&iface), "the iface is gone");
    assert!(!transport.detach_shared_instance().await, "a second detach finds nothing");

    timeout(WAIT, transport.stop()).await.expect("client stop completes");
    timeout(WAIT, shared.transport.stop()).await.expect("shared instance stop completes");
}
