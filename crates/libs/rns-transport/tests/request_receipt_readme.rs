//! End-to-end usage of `Transport::request` from outside the crate, in the shape the
//! README's "Request handlers" and "Sending requests" sections show: two transports over a
//! loopback TCP pair, the README's handler on the responder, the README's request snippet
//! verbatim on the requester, and the claims around it that a first-time consumer reads
//! next: a packet-form request goes `Sent -> Ready` and never `Delivered`; a response larger
//! than the link MDU arrives as a Resource and still resolves the receipt through
//! `Receiving`; a link that closes fails a pending receipt `LinkClosed` and a request on the
//! closed link is refused with `RnsError::ConnectionError`; `Transport::stop()` fails every
//! pending receipt the same way, on every link the transport holds and whatever state each
//! receipt is in.

use std::error::Error;
use std::sync::Arc;
use std::time::{Duration, Instant};

use rns_transport::destination::link::{Link, LinkEvent, LinkEventData, LinkStatus};
use rns_transport::destination::request::{
    InboundRequest, RequestAllow, RequestError, RequestOptions, RequestReceipt, RequestResponse,
    RequestStatus,
};
use rns_transport::destination::{DestinationDesc, DestinationName, SingleInputDestination};
use rns_transport::error::RnsError;
use rns_transport::hash::AddressHash;
use rns_transport::identity::PrivateIdentity;
use rns_transport::iface::tcp_client::TcpClient;
use rns_transport::iface::tcp_server::TcpServer;
use rns_transport::rand_core::OsRng;
use rns_transport::rmpv;
use rns_transport::transport::{AnnounceEvent, SendPacketOutcome, Transport, TransportConfig};
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::{broadcast, Mutex};
use tokio::time::timeout;

const WAIT: Duration = Duration::from_secs(15);
const POLL: Duration = Duration::from_millis(20);
/// A second announce from the same interface inside the connector's ingress-control sample
/// window is rate-limited; this keeps a second destination's announce outside it.
const ANNOUNCE_SPACING: Duration = Duration::from_secs(2);

/// The two ends of one active link, built from the public API only.
struct Pair {
    listener: Transport,
    connector: Transport,
    destination: Arc<Mutex<SingleInputDestination>>,
    link: Arc<Mutex<Link>>,
    link_id: AddressHash,
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
async fn learn_announce(
    announces: &mut broadcast::Receiver<AnnounceEvent>,
    hash: &AddressHash,
) -> DestinationDesc {
    timeout(WAIT, async {
        loop {
            match announces.recv().await {
                Ok(event) => {
                    let desc = event.destination.lock().await.desc;
                    if desc.address_hash == *hash {
                        return desc;
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
async fn await_activated(events: &mut broadcast::Receiver<LinkEventData>, link_id: AddressHash) {
    timeout(WAIT, async {
        loop {
            match events.recv().await {
                Ok(event) if event.id == link_id && matches!(event.event, LinkEvent::Activated) => {
                    return;
                }
                Ok(_) => {}
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => panic!("link event channel closed"),
            }
        }
    })
    .await
    .expect("the link activates");
}

/// Boots a listener and a connector on 127.0.0.1, announces one destination, opens a link.
#[deny(clippy::await_holding_invalid_type)]
async fn pair(aspect: &str) -> Pair {
    let listener = Transport::new(TransportConfig::new(
        "readme-listener",
        &PrivateIdentity::new_from_rand(OsRng),
        false,
    ));
    let port = free_loopback_port().await;
    let server = TcpServer::new(format!("127.0.0.1:{port}"), listener.iface_manager());
    let server_status = server.runtime_status_handle();
    listener.iface_manager().lock().await.spawn(server, TcpServer::spawn);
    wait_until("listener to listen", || {
        server_status.to_json()["listener_state"].as_str() == Some("listening")
    })
    .await;

    let connector = Transport::new(TransportConfig::new(
        "readme-connector",
        &PrivateIdentity::new_from_rand(OsRng),
        false,
    ));
    let mut announces = connector.recv_announces().await;
    let client = TcpClient::new(format!("127.0.0.1:{port}"));
    let client_status = client.runtime_status_handle();
    let context = connector.iface_manager().lock().await.new_context(client);
    tokio::spawn(TcpClient::spawn(context));
    wait_until("connector to connect", || {
        client_status.to_json()["stream_state"].as_str() == Some("connected")
    })
    .await;
    wait_until("listener to accept", || {
        server_status.to_json()["accepted_connections"].as_u64() == Some(1)
    })
    .await;

    let destination = listener
        .add_destination(
            PrivateIdentity::new_from_rand(OsRng),
            DestinationName::new("readme", aspect),
        )
        .await;
    let desc = destination.lock().await.desc;
    let announce = destination.lock().await.announce(OsRng, None).expect("announce packet");
    listener.send_packet(announce).await;
    let learned = learn_announce(&mut announces, &desc.address_hash).await;

    let mut out_events = connector.out_link_events();
    let link = connector.link(learned).await.expect("link request sent");
    let link_id = *link.lock().await.id();
    await_activated(&mut out_events, link_id).await;

    Pair { listener, connector, destination, link, link_id }
}

/// Announces a second destination on the pair's listener and opens a second link to it from
/// the pair's connector, so one transport holds two active links.
#[deny(clippy::await_holding_invalid_type)]
async fn second_link(
    pair: &Pair,
    aspect: &str,
) -> (Arc<Mutex<SingleInputDestination>>, AddressHash) {
    tokio::time::sleep(ANNOUNCE_SPACING).await;
    let destination = pair
        .listener
        .add_destination(
            PrivateIdentity::new_from_rand(OsRng),
            DestinationName::new("readme", aspect),
        )
        .await;
    let desc = destination.lock().await.desc;
    let mut announces = pair.connector.recv_announces().await;
    let announce = destination.lock().await.announce(OsRng, None).expect("announce packet");
    pair.listener.send_packet(announce).await;
    let learned = learn_announce(&mut announces, &desc.address_hash).await;

    let mut out_events = pair.connector.out_link_events();
    let link = pair.connector.link(learned).await.expect("link request sent");
    let link_id = *link.lock().await.id();
    await_activated(&mut out_events, link_id).await;
    assert_ne!(link_id, pair.link_id, "a distinct link");
    (destination, link_id)
}

/// The README "Request handlers" snippet, verbatim.
#[deny(clippy::await_holding_invalid_type)]
async fn register_readme_echo(
    destination: &Arc<Mutex<SingleInputDestination>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    destination.lock().await.register_request_handler(
        "/echo",
        Arc::new(|request: InboundRequest| async move {
            Some(RequestResponse::Value(request.into_data()))
        }),
        RequestAllow::All,
        true,
    )?;
    Ok(())
}

/// The README "Sending requests" snippet, verbatim, returning the receipt for the asserts
/// that follow it.
#[deny(clippy::await_holding_invalid_type)]
async fn readme_request(
    transport: &Transport,
    link_id: AddressHash,
) -> Result<RequestReceipt, Box<dyn Error + Send + Sync>> {
    let receipt = transport
        .request(&link_id, "/echo", rmpv::Value::from("ping"), RequestOptions::default())
        .await?;
    let response = receipt.response().await?;
    assert_eq!(response.value(), Some(&rmpv::Value::from("ping")));
    Ok(receipt)
}

/// Every status a subscriber sees until the receipt concludes.
#[deny(clippy::await_holding_invalid_type)]
async fn statuses_until_concluded(receipt: &RequestReceipt) -> Vec<RequestStatus> {
    let mut status_rx = receipt.subscribe();
    let mut seen = vec![*status_rx.borrow_and_update()];
    timeout(WAIT, async {
        while !matches!(seen.last(), Some(RequestStatus::Ready | RequestStatus::Failed)) {
            status_rx.changed().await.expect("the entry outlives the subscription");
            seen.push(*status_rx.borrow_and_update());
        }
    })
    .await
    .expect("the receipt concludes");
    seen
}

/// A handler that never answers, so a receipt stays pending until something else ends it.
fn silent() -> impl Fn(InboundRequest) -> std::future::Ready<Option<RequestResponse>> + Send + Sync
{
    |_: InboundRequest| std::future::ready(None)
}

#[deny(clippy::await_holding_invalid_type)]
async fn register(
    destination: &Arc<Mutex<SingleInputDestination>>,
    path: &str,
    handler: impl Fn(InboundRequest) -> std::future::Ready<Option<RequestResponse>>
        + Send
        + Sync
        + 'static,
) {
    destination
        .lock()
        .await
        .register_request_handler(path, Arc::new(handler), RequestAllow::All, true)
        .expect("handler registers");
}

/// Closes the link from the connector's side and lets the close packet reach the listener.
#[deny(clippy::await_holding_invalid_type)]
async fn close_from_connector(pair: &Pair) {
    let packet = pair.link.lock().await.teardown().expect("an active link closes");
    let outcome = pair.connector.send_link_packet_on_bound_iface(&pair.link, packet).await;
    assert_eq!(outcome, SendPacketOutcome::SentDirect, "the close packet reaches the wire");
}

/// Waits until `receipt` reports `wanted`, failing if it concludes first.
#[deny(clippy::await_holding_invalid_type)]
async fn await_status(receipt: &RequestReceipt, wanted: RequestStatus) {
    let mut status_rx = receipt.subscribe();
    timeout(WAIT, async {
        while *status_rx.borrow_and_update() != wanted {
            let now = *status_rx.borrow();
            assert!(
                !matches!(now, RequestStatus::Ready | RequestStatus::Failed),
                "concluded {now:?} before {wanted:?}"
            );
            status_rx.changed().await.expect("the entry outlives the subscription");
        }
    })
    .await
    .unwrap_or_else(|_| panic!("timed out waiting for {wanted:?}"));
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn readme_snippets_round_trip_a_packet_request() {
    let pair = pair("snippet").await;
    register_readme_echo(&pair.destination).await.expect("the README handler registers");

    let before = Instant::now();
    let receipt = readme_request(&pair.connector, pair.link_id).await.expect("the README snippet");

    // What the README says around the snippet.
    assert_eq!(receipt.status(), RequestStatus::Ready);
    assert_eq!(receipt.progress(), 1.0);
    let started = receipt.started_at().expect("a packet request starts at the send");
    let concluded = receipt.concluded_at().expect("a ready receipt has concluded");
    assert!(started >= before && concluded >= started, "{started:?} {concluded:?}");
    assert!(receipt.response_size().is_some(), "a packet response sets response_size");
    assert!(receipt.response_transfer_size().is_some(), "and response_transfer_size");

    // A clone made after the fact reads the same body, and a second read is idempotent.
    let clone = receipt.clone();
    let again = clone.response().await.expect("the clone resolves");
    assert_eq!(again.value(), Some(&rmpv::Value::from("ping")));
    assert_eq!(clone.request_id(), receipt.request_id());
    assert_eq!(*clone.subscribe().borrow(), RequestStatus::Ready);
    drop(pair);
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn packet_form_request_never_reports_delivered() {
    let pair = pair("never-delivered").await;
    register_readme_echo(&pair.destination).await.expect("the README handler registers");

    let receipt = pair
        .connector
        .request(&pair.link_id, "/echo", rmpv::Value::from(7u8), RequestOptions::default())
        .await
        .expect("a packet request sends");
    let seen = statuses_until_concluded(&receipt).await;
    assert!(!seen.contains(&RequestStatus::Delivered), "packet form: {seen:?}");
    assert!(!seen.contains(&RequestStatus::Receiving), "packet response: {seen:?}");
    assert_eq!(seen.last(), Some(&RequestStatus::Ready), "{seen:?}");
    assert_eq!(receipt.response().await.expect("ready").value(), Some(&rmpv::Value::from(7u8)));
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn response_over_the_link_mdu_arrives_as_a_resource_and_resolves_the_receipt() {
    let pair = pair("resource-response").await;
    let body: Vec<u8> = {
        let mdu = pair.link.lock().await.link_mdu();
        (0..mdu * 8).map(|i| (i % 251) as u8).collect()
    };
    let served = body.clone();
    register(&pair.destination, "/big", move |_: InboundRequest| {
        std::future::ready(Some(RequestResponse::Bytes(served.clone())))
    })
    .await;

    let receipt = pair
        .connector
        .request(&pair.link_id, "/big", rmpv::Value::Nil, RequestOptions::default())
        .await
        .expect("a packet request sends");
    let seen = statuses_until_concluded(&receipt).await;
    assert_eq!(seen.last(), Some(&RequestStatus::Ready), "{seen:?}");
    assert!(!seen.contains(&RequestStatus::Delivered), "packet form: {seen:?}");
    assert!(seen.contains(&RequestStatus::Receiving), "a Resource response is received: {seen:?}");

    let response = receipt.response().await.expect("the Resource response resolves");
    assert_eq!(response.value(), Some(&rmpv::Value::from(body.clone())));
    assert!(response.metadata().is_none(), "a Bytes response is not a file response");
    assert_eq!(receipt.progress(), 1.0);
    let size = receipt.response_size().expect("d of the advertisement");
    let transfer = receipt.response_transfer_size().expect("t of the advertisement");
    assert!(size >= body.len(), "response_size {size} covers the {} byte body", body.len());
    assert!(transfer > 0, "response_transfer_size is the bytes on the wire");
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn link_close_fails_the_pending_receipt_and_refuses_the_next_request() {
    let pair = pair("close").await;
    register_readme_echo(&pair.destination).await.expect("the README handler registers");
    register(&pair.destination, "/quiet", silent()).await;

    let pending = pair
        .connector
        .request(&pair.link_id, "/quiet", rmpv::Value::Nil, RequestOptions::default())
        .await
        .expect("a packet request sends");
    assert_eq!(pending.status(), RequestStatus::Sent);

    close_from_connector(&pair).await;
    wait_until("the link to close", || {
        pair.link.try_lock().map(|link| link.status() == LinkStatus::Closed).unwrap_or(false)
    })
    .await;

    let outcome = timeout(WAIT, pending.response()).await.expect("response() returns");
    assert_eq!(outcome, Err(RequestError::LinkClosed));
    assert_eq!(pending.status(), RequestStatus::Failed);
    assert!(pending.concluded_at().is_some());

    // The closed link no longer takes requests: either not active or already forgotten.
    let refused = pair
        .connector
        .request(&pair.link_id, "/echo", rmpv::Value::from("late"), RequestOptions::default())
        .await;
    assert!(refused.is_err(), "a request on a closed link is refused: {refused:?}");
    drop(pair);
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn request_on_a_closed_link_is_a_connection_error() {
    let pair = pair("closed-link-error").await;
    register(&pair.destination, "/quiet", silent()).await;

    let pending = pair
        .connector
        .request(&pair.link_id, "/quiet", rmpv::Value::Nil, RequestOptions::default())
        .await
        .expect("a packet request sends");

    // `teardown()` closes the link locally at once; the link stays known to the transport
    // until the next link check, so the request meets a link that is not active rather than
    // an unknown one.
    let packet = pair.link.lock().await.teardown().expect("an active link closes");
    assert_eq!(pair.link.lock().await.status(), LinkStatus::Closed);
    let refused = pair
        .connector
        .request(&pair.link_id, "/quiet", rmpv::Value::Nil, RequestOptions::default())
        .await;
    assert!(matches!(refused, Err(RnsError::ConnectionError)), "{refused:?}");

    // The receipt that was pending at the close has already failed `LinkClosed`.
    assert_eq!(pending.status(), RequestStatus::Failed);
    assert_eq!(pending.response().await, Err(RequestError::LinkClosed));

    // Delivering the close packet changes nothing for the requester side.
    let outcome = pair.connector.send_link_packet_on_bound_iface(&pair.link, packet).await;
    assert_eq!(outcome, SendPacketOutcome::SentDirect);
    assert_eq!(pending.response().await, Err(RequestError::LinkClosed), "re-reads agree");
    drop(pair);
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn stop_fails_every_pending_receipt_on_every_link() {
    let pair = pair("stop-all-a").await;
    register(&pair.destination, "/quiet", silent()).await;
    let (second_destination, second_link_id) = second_link(&pair, "stop-all-b").await;
    register(&second_destination, "/quiet", silent()).await;

    // First link: a packet-form receipt in `Sent` and a resource-form receipt that has
    // reached `Delivered` with a timeout that would never fire on its own.
    let packet_form = pair
        .connector
        .request(&pair.link_id, "/quiet", rmpv::Value::Nil, RequestOptions::default())
        .await
        .expect("a packet request sends");
    let body: Vec<u8> = {
        let mdu = pair.link.lock().await.link_mdu();
        (0..mdu + 1).map(|i| (i % 253) as u8).collect()
    };
    let resource_form = pair
        .connector
        .request(
            &pair.link_id,
            "/quiet",
            rmpv::Value::Binary(body),
            RequestOptions::default().with_timeout(Duration::MAX),
        )
        .await
        .expect("a resource-form request sends");
    assert!(resource_form.started_at().is_none(), "no clock before Delivered");
    await_status(&resource_form, RequestStatus::Delivered).await;
    assert!(resource_form.started_at().is_some(), "the clock starts at Delivered");

    // Second link: another packet-form receipt in `Sent`.
    let other_link = pair
        .connector
        .request(&second_link_id, "/quiet", rmpv::Value::Nil, RequestOptions::default())
        .await
        .expect("a packet request sends on the second link");
    assert_eq!(packet_form.status(), RequestStatus::Sent);
    assert_eq!(other_link.status(), RequestStatus::Sent);

    timeout(WAIT, pair.connector.stop()).await.expect("stop returns");

    for (name, receipt) in [
        ("packet form, first link", &packet_form),
        ("resource form in Delivered, first link", &resource_form),
        ("packet form, second link", &other_link),
    ] {
        assert_eq!(receipt.status(), RequestStatus::Failed, "{name}");
        assert!(receipt.concluded_at().is_some(), "{name}");
        let outcome =
            timeout(WAIT, receipt.response()).await.unwrap_or_else(|_| panic!("{name} hangs"));
        assert_eq!(outcome, Err(RequestError::LinkClosed), "{name}");
    }
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn stop_fails_the_pending_receipt_link_closed() {
    let pair = pair("stop").await;
    register(&pair.destination, "/quiet", silent()).await;

    let pending = pair
        .connector
        .request(&pair.link_id, "/quiet", rmpv::Value::Nil, RequestOptions::default())
        .await
        .expect("a packet request sends");
    let waiter = {
        let receipt = pending.clone();
        tokio::spawn(async move { receipt.response().await })
    };

    pair.connector.stop().await;

    let outcome = timeout(WAIT, waiter).await.expect("the waiter wakes").expect("no panic");
    assert_eq!(outcome, Err(RequestError::LinkClosed));
    assert_eq!(pending.status(), RequestStatus::Failed);
    assert_eq!(pending.response().await, Err(RequestError::LinkClosed), "re-reads agree");
}
