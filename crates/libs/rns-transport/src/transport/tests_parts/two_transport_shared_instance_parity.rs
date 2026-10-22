// Coverage for the announce a shared-instance client sends on its own after a destination
// is registered: the shared instance receives it as a path response without the test
// announcing, a transport that is not attached sends none, the path-response announce
// differs from the plain one only in its context, and the re-announce of every registered
// destination carries each destination's app data. Helpers come from the sibling
// `tests_parts` files included into this module.

/// Longer than the client's post-registration delay, so a registration announce that
/// should not be sent has had time to arrive before the explicit announce is sent.
const SHARED_INSTANCE_PARITY_SILENCE: Duration = Duration::from_secs(1);

/// Consumes announces until the one for `hash` arrives and returns the whole event.
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_parity_await_announce(
    announces: &mut broadcast::Receiver<AnnounceEvent>,
    hash: &AddressHash,
) -> AnnounceEvent {
    timeout(LOOPBACK_TIMEOUT, async {
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
    .expect("timed out waiting for the announce")
}

/// Registering a destination on an attached client is enough for the shared instance to
/// learn it: the client announces it as a path response after its delay, with no announce
/// from the test.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_parity_registered_destination_is_announced_as_path_response() {
    let server = shared_instance_server().await;
    let client = shared_instance_client("shared-instance-parity-client", &server).await;
    server.await_accepted_connections(1).await;
    shared_instance_assert_attached(&client.transport, client.iface).await;
    let mut server_announces = server.transport.recv_announces().await;

    let identity = PrivateIdentity::new_from_rand(OsRng);
    let destination = client
        .transport
        .add_destination(identity, DestinationName::new("shared-instance-parity", "registered"))
        .await;
    let address_hash = destination.lock().await.desc.address_hash;

    let event = shared_instance_parity_await_announce(&mut server_announces, &address_hash).await;
    assert!(event.is_path_response, "the registration announce carries the path-response context");
    assert_eq!(event.hops, 1, "the shared instance learns the client's destination at one hop");

    timeout(STOP_TIMEOUT, client.transport.stop()).await.expect("client stop completes");
    timeout(STOP_TIMEOUT, server.transport.stop()).await.expect("shared instance stop completes");
}

/// A transport that merely connects to the shared instance over TCP, without attaching,
/// is no shared-instance client: registering a destination sends nothing, and the first
/// announce the shared instance sees for it is the plain one the test sends afterwards.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_parity_unattached_transport_does_not_announce_on_registration() {
    let server = shared_instance_server().await;
    let connector =
        connector("shared-instance-parity-unattached", server.port, TcpClient::DEFAULT_MTU).await;
    server.await_accepted_connections(1).await;
    assert!(
        !connector.transport.connected_to_shared_instance().await,
        "a plain TCP connector is not a shared-instance client"
    );
    let mut server_announces = server.transport.recv_announces().await;

    let identity = PrivateIdentity::new_from_rand(OsRng);
    let destination = connector
        .transport
        .add_destination(identity, DestinationName::new("shared-instance-parity", "unattached"))
        .await;
    let address_hash = destination.lock().await.desc.address_hash;

    let early = timeout(SHARED_INSTANCE_PARITY_SILENCE, server_announces.recv()).await;
    assert!(early.is_err(), "no announce arrives from the registration alone");

    connector.transport.send_announce(&destination, None).await;
    let event = shared_instance_parity_await_announce(&mut server_announces, &address_hash).await;
    assert!(!event.is_path_response, "the explicit announce is a plain one");

    timeout(STOP_TIMEOUT, connector.transport.stop()).await.expect("connector stop completes");
    timeout(STOP_TIMEOUT, server.transport.stop()).await.expect("shared instance stop completes");
}

/// The path-response flavour builds the same packet as the plain announce apart from its
/// context: same header, addressing and payload length. The payload bytes differ by the
/// random part of the announce, so the structure is compared rather than the bytes.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_parity_path_response_announce_differs_only_in_context() {
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let mut destination = SingleInputDestination::new(
        identity,
        DestinationName::new("shared-instance-parity", "context"),
    );
    let app_data = b"shared-instance-parity";

    let plain = destination.announce(OsRng, Some(app_data)).expect("plain announce builds");
    let path_response = destination
        .announce_with_context(OsRng, Some(app_data), true)
        .expect("path-response announce builds");
    let explicit_plain = destination
        .announce_with_context(OsRng, Some(app_data), false)
        .expect("explicit plain announce builds");

    assert_eq!(plain.context, PacketContext::None);
    assert_eq!(path_response.context, PacketContext::PathResponse);
    assert_eq!(explicit_plain.context, PacketContext::None);
    for announce in [&path_response, &explicit_plain] {
        assert_eq!(announce.header, plain.header);
        assert_eq!(announce.destination, plain.destination);
        assert_eq!(announce.transport, plain.transport);
        assert_eq!(announce.ifac, plain.ifac);
        assert_eq!(announce.data.len(), plain.data.len());
        assert!(announce.data.as_slice().ends_with(app_data), "the app data is appended");
    }
}

/// Re-announcing every registered destination sends one path-response announce per
/// destination, each carrying the app data registered for it, and reports the count. The
/// transport is not a shared-instance client here, so no registration announce competes
/// for the interface and the two packets read back are the re-announce alone.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_parity_reannounce_sends_every_destination_with_its_app_data() {
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let transport =
        Transport::new(TransportConfig::new("shared-instance-parity-reannounce", &identity, false));
    let mut iface_channel = transport.iface_manager().lock().await.new_channel(16);

    let with_app_data = transport
        .add_destination(
            PrivateIdentity::new_from_rand(OsRng),
            DestinationName::new("shared-instance-parity", "with-app-data"),
        )
        .await;
    let without_app_data = transport
        .add_destination(
            PrivateIdentity::new_from_rand(OsRng),
            DestinationName::new("shared-instance-parity", "without-app-data"),
        )
        .await;
    let with_app_data_hash = with_app_data.lock().await.desc.address_hash;
    let without_app_data_hash = without_app_data.lock().await.desc.address_hash;
    let app_data = b"reannounce app data";
    transport.set_destination_announce_app_data(&with_app_data, Some(app_data.to_vec())).await;

    assert_eq!(transport.announce_destinations_to_shared_instance().await, 2);

    let mut announced = Vec::new();
    for _ in 0..2 {
        let sent = timeout(LOOPBACK_TIMEOUT, iface_channel.tx_channel.recv())
            .await
            .expect("the re-announce reaches the interface")
            .expect("tx channel open");
        assert_eq!(sent.packet.header.packet_type, PacketType::Announce);
        assert_eq!(sent.packet.context, PacketContext::PathResponse);
        if sent.packet.destination == with_app_data_hash {
            assert!(
                sent.packet.data.as_slice().ends_with(app_data),
                "the registered app data is appended"
            );
        }
        announced.push(sent.packet.destination);
    }
    announced.sort();
    let mut expected = vec![with_app_data_hash, without_app_data_hash];
    expected.sort();
    assert_eq!(announced, expected, "each registered destination is announced once");

    timeout(STOP_TIMEOUT, transport.stop()).await.expect("transport stop completes");
}
