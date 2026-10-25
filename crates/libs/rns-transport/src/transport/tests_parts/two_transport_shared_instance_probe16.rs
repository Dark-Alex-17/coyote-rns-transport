// Two CHANGELOG claims about link teardown and reconnect, pinned here. It says building the
// LINKCLOSE of a `Handshake` teardown "no longer warns `can't create data packet for closed link`",
// so a consumer tearing down a half-built link sees the close packet and a quiet log; the
// warning stays for a plain data packet built before the link is `Active`, which is the
// positive control here. And the CHANGELOG and the README say the reconnect consumer
// "stays the plain biased loop": a reconnect served with no disconnect before it
// re-synthesizes the tunnel and flushes nothing, so the tables learned over the connection
// survive it; this is the pin of that rule. Helpers come from the sibling `tests_parts`
// files included into this module.

/// A responder that has derived its session cipher from the request but has not yet
/// proved: `Handshake`, the status Python still sends the LINKCLOSE from
/// (`RNS/Link.py:668`).
fn shared_instance_probe16_handshake_responder() -> Link {
    let signer = PrivateIdentity::new_from_rand(OsRng);
    let identity = *signer.as_identity();
    let destination = DestinationDesc {
        identity,
        address_hash: identity.address_hash,
        name: DestinationName::new("lxmf", "delivery"),
    };
    let (tx, _) = tokio::sync::broadcast::channel(4);
    let mut outbound = Link::new(destination, tx.clone());
    let request = outbound.request();
    let inbound = Link::new_from_request(&request, signer.sign_key().clone(), destination, tx)
        .expect("link request should parse");
    assert_eq!(inbound.status(), crate::destination::link::LinkStatus::Handshake);
    inbound
}

/// The warning is advisory and names no link, so the count is read around each call: a
/// plain data packet built from `Handshake` still raises it (the session key exists, so the
/// packet builds), the LINKCLOSE of the same link's teardown does not.
#[test]
fn shared_instance_probe16_handshake_teardown_builds_its_close_without_the_closed_link_warning() {
    install_test_logger();
    let mut inbound = shared_instance_probe16_handshake_responder();

    let before_data = closed_link_packet_warning_count();
    inbound
        .data_packet(b"too early")
        .expect("a handshake link has its session key, so the data packet still builds");
    assert!(
        closed_link_packet_warning_count() > before_data,
        "positive control: a data packet built before Active raises the closed-link warning"
    );

    let before_close = closed_link_packet_warning_count();
    let close = inbound.teardown().expect("a handshake teardown builds its LINKCLOSE");
    assert_eq!(close.context, PacketContext::LinkClose);
    assert_eq!(close.destination, *inbound.id());
    assert_eq!(
        closed_link_packet_warning_count(),
        before_close,
        "the LINKCLOSE of a handshake teardown raises no closed-link warning"
    );
}

/// The consumer is built by hand on a client attached to a live shared instance and fed a
/// reconnect alone, with no disconnect before it: the plain loop re-synthesizes the tunnel,
/// runs no flush, records no lost event and the learned path is still there afterwards.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_probe16_reconnect_without_its_disconnect_keeps_the_tables() {
    install_test_logger();
    let name = "shared-instance-probe16-lone-reconnect";
    let server = shared_instance_server().await;
    let mut client = shared_instance_reconnect_client(name, &server).await;
    server.await_accepted_connections(1).await;
    shared_instance_assert_attached(&client.transport, client.iface).await;
    let (_destination, desc) =
        register_in_destination(&server.transport, "shared-instance-probe16", "server").await;
    shared_instance_learn_announce(&mut client.announces, &desc.address_hash).await;
    assert!(client.transport.has_path(&desc.address_hash).await, "the client learned the path");
    let interfaces_before = client.transport.interface_hashes().await;

    let (reconnects_tx, reconnects) = tokio::sync::mpsc::channel(1);
    let (disconnects_tx, disconnects) = tokio::sync::mpsc::channel(1);
    reconnects_tx.send(client.iface).await.expect("queue the lone reconnect");
    drop(reconnects_tx);
    let consumer = super::links::SharedInstanceReconnectConsumer {
        handler: client.transport.get_handler(),
        iface_manager: client.transport.iface_manager(),
        cancel: tokio_util::sync::CancellationToken::new(),
        name: name.to_string(),
        tasks: client.transport.shared_instance_tasks.clone(),
        iface: client.iface,
        reconnect_wait: Duration::ZERO,
    };
    let cancel = consumer.cancel.clone();
    timeout(STOP_TIMEOUT, consumer.run(reconnects, disconnects))
        .await
        .expect("the consumer exits once the reconnect channel is drained and closed");
    cancel.cancel();
    drop(disconnects_tx);

    assert!(
        client.transport.has_path(&desc.address_hash).await,
        "a reconnect served without a disconnect flushes nothing: the learned path survives"
    );
    assert_eq!(
        client.transport.interface_hashes().await,
        interfaces_before,
        "the interface set is untouched"
    );
    let records: Vec<String> =
        shared_instance_records_for(name).into_iter().map(|(_, record)| record).collect();
    assert!(
        records.iter().any(|record| record.contains("reconnected, tunnel re-synthesized")),
        "the reconnect is served as a re-synthesis: {records:?}"
    );
    assert!(
        !records.iter().any(|record| record.contains("disconnected,")),
        "no flush ran for a reconnect that arrived alone: {records:?}"
    );
    assert!(
        !records.iter().any(|record| record.contains("without its disconnect")),
        "the plain loop infers no lost event: {records:?}"
    );
    shared_instance_assert_never_warned(name);

    timeout(STOP_TIMEOUT, client.transport.stop()).await.expect("client stop completes");
    timeout(STOP_TIMEOUT, server.transport.stop()).await.expect("shared instance stop completes");
}
