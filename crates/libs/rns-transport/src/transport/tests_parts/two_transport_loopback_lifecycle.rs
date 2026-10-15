// Loopback coverage for the transport lifecycle surface: link round-trip time after
// activation, `Transport::stop` releasing the listener socket, announce building that
// fails instead of panicking, and an over-cap resource advertisement that is dropped
// without disturbing the link. Helpers come from the sibling `tests_parts` files included
// into this module.

use crate::packet::PACKET_DATA_MAX;
use crate::resource::RANDOM_HASH_SIZE;

const STOP_TIMEOUT: Duration = Duration::from_secs(5);

/// An announce whose app data cannot fit the packet buffer fails to build.
const UNBUILDABLE_APP_DATA_LEN: usize = PACKET_DATA_MAX + 1;

/// A link pair that keeps the initiator's `Link` so tests can inspect it and encrypt
/// packets for it directly.
struct LoopbackLinkHandles {
    listener: LoopbackListener,
    connector: LoopbackConnector,
    link: Arc<Mutex<Link>>,
    link_id: AddressHash,
    desc: DestinationDesc,
    in_events: broadcast::Receiver<LinkEventData>,
}

#[deny(clippy::await_holding_invalid_type)]
async fn loopback_link_handles(aspect: &str) -> LoopbackLinkHandles {
    let listener = listener("loopback-listener", TcpClient::DEFAULT_MTU).await;
    let mut connector =
        connector("loopback-connector", listener.port, TcpClient::DEFAULT_MTU).await;
    listener.await_accepted_connections(1).await;
    let mut in_events = listener.transport.in_link_events();

    let (_destination, desc) =
        register_in_destination(&listener.transport, "loopback", aspect).await;
    let learned = learn_announce(&mut connector.announces, &desc.address_hash).await;
    let (link, link_id) = open_link(&connector.transport, learned).await;
    let activated = await_event(&mut in_events, |event| {
        matches!(event.event, LinkEvent::Activated)
    })
    .await;
    assert_eq!(activated.id, link_id, "both ends share one link id");
    LoopbackLinkHandles {
        listener,
        connector,
        link,
        link_id,
        desc,
        in_events,
    }
}

/// Sends one Data packet in each direction over the pair's link and checks delivery.
#[deny(clippy::await_holding_invalid_type)]
async fn exchange_data_both_ways(pair: &mut LoopbackLinkHandles) {
    let mut out_events = pair.connector.transport.out_link_events();
    let link_id = pair.link_id;

    let report = pair
        .connector
        .transport
        .send_to_out_links_with_report(&pair.desc.address_hash, b"ping after advert")
        .await;
    assert_sent_on_one_link(report, "outbound");
    let inbound = await_event(&mut pair.in_events, |event| {
        event.id == link_id && matches!(event.event, LinkEvent::Data(_))
    })
    .await;
    assert_eq!(data_bytes(inbound), b"ping after advert");

    let report = pair
        .listener
        .transport
        .send_to_in_links_with_report(&pair.desc.address_hash, b"pong after advert")
        .await;
    assert_sent_on_one_link(report, "inbound");
    let outbound = await_event(&mut out_events, |event| {
        event.id == link_id && matches!(event.event, LinkEvent::Data(_))
    })
    .await;
    assert_eq!(data_bytes(outbound), b"pong after advert");
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_link_reports_rtt_on_both_ends() {
    let pair = loopback_link_handles("rtt").await;

    let initiator_rtt = pair.link.lock().await.rtt();
    assert!(initiator_rtt > Duration::ZERO, "initiator rtt is measured on activation");
    assert!(initiator_rtt < LOOPBACK_TIMEOUT, "initiator rtt is plausible: {initiator_rtt:?}");

    // The responder learns its rtt from the initiator's rtt packet, which lands a little
    // after the activation event the fixture waited for.
    let responder = pair
        .listener
        .transport
        .find_in_link(&pair.link_id)
        .await
        .expect("responder tracks the inbound link");
    let deadline = Instant::now() + LOOPBACK_TIMEOUT;
    let responder_rtt = loop {
        let rtt = responder.lock().await.rtt();
        if rtt > Duration::ZERO {
            break rtt;
        }
        assert!(Instant::now() < deadline, "timed out waiting for the responder rtt");
        tokio::time::sleep(LOOPBACK_POLL).await;
    };
    assert!(responder_rtt < LOOPBACK_TIMEOUT, "responder rtt is plausible: {responder_rtt:?}");
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_stop_releases_listener_and_is_idempotent() {
    let pair = loopback_link_handles("stop").await;
    let port = pair.listener.port;

    timeout(STOP_TIMEOUT, pair.listener.transport.stop())
        .await
        .expect("listener stop completes");

    assert!(
        pair.listener.transport.interface_hashes().await.is_empty(),
        "stop detaches every interface"
    );
    let state = pair.listener.status.to_json();
    assert_eq!(
        state["listener_state"].as_str(),
        Some("closed"),
        "the server worker has exited: {state}"
    );
    let rebound = tokio::net::TcpListener::bind(("127.0.0.1", port)).await;
    assert!(rebound.is_ok(), "stop releases the listening socket: {rebound:?}");

    timeout(STOP_TIMEOUT, pair.listener.transport.stop())
        .await
        .expect("a second stop is a no-op that still returns");
    timeout(STOP_TIMEOUT, pair.connector.transport.stop())
        .await
        .expect("connector stop completes");
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn send_announce_survives_an_unbuildable_announce() {
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let transport = Transport::new(TransportConfig::new("t", &identity, false));
    let destination =
        transport.add_destination(identity, DestinationName::new("app", "aspect")).await;
    let oversized = vec![0u8; UNBUILDABLE_APP_DATA_LEN];

    transport.send_announce(&destination, Some(&oversized)).await;
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn try_send_announce_reports_the_build_error() {
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let transport = Transport::new(TransportConfig::new("t", &identity, false));
    let destination =
        transport.add_destination(identity, DestinationName::new("app", "aspect")).await;
    let oversized = vec![0u8; UNBUILDABLE_APP_DATA_LEN];

    assert!(matches!(
        transport.try_send_announce(&destination, Some(&oversized)).await,
        Err(RnsError::OutOfMemory)
    ));
    assert!(transport.try_send_announce(&destination, Some(b"small")).await.is_ok());
}

/// An advertisement whose transfer size is above the reference decode cap is dropped on
/// the receiving transport: no request goes back, no resource event fires, and the link
/// keeps carrying data afterwards.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_drops_oversized_advertisement_and_keeps_link_usable() {
    let mut pair = loopback_link_handles("advert-cap").await;
    let mut connector_tap = pair.connector.transport.iface_rx();
    let mut listener_resources = pair.listener.transport.resource_events();

    let advertisement = ResourceAdvertisement {
        transfer_size: crate::resource::MAX_REFERENCE_ADVERTISED_TRANSFER_SIZE + 1,
        data_size: 1,
        parts: 1,
        hash: Hash::new_from_slice(&[3u8; 32]),
        random_hash: [0u8; RANDOM_HASH_SIZE],
        original_hash: Hash::new_from_slice(&[3u8; 32]),
        segment_index: 1,
        total_segments: 1,
        request_id: None,
        flags: 0,
        hashmap: vec![0u8; MAPHASH_LEN],
    };
    let packet = {
        let link = pair.link.lock().await;
        crate::resource::build_link_packet(
            &link,
            PacketType::Data,
            PacketContext::ResourceAdvrtisement,
            &advertisement.pack().expect("advertisement packs"),
        )
        .expect("advertisement fits a link packet")
    };
    let outcome = pair.connector.transport.send_link_packet_on_bound_iface(&pair.link, packet).await;
    assert_eq!(outcome, SendPacketOutcome::SentDirect, "advertisement reaches the wire");

    exchange_data_both_ways(&mut pair).await;

    loop {
        match connector_tap.try_recv() {
            Ok(message) => assert!(
                !(message.packet.destination == pair.link_id
                    && message.packet.context == PacketContext::ResourceRequest),
                "a dropped advertisement must not be answered with a part request"
            ),
            Err(tokio::sync::broadcast::error::TryRecvError::Lagged(_)) => continue,
            Err(_) => break,
        }
    }
    loop {
        match listener_resources.try_recv() {
            Ok(event) => assert_ne!(
                event.link_id, pair.link_id,
                "a dropped advertisement must not create receiver state: {:?}",
                event.kind
            ),
            Err(tokio::sync::broadcast::error::TryRecvError::Lagged(_)) => continue,
            Err(_) => break,
        }
    }
}
