// Pins the README's account of a transport whose connected posture is set directly, as
// reticulumd's own bootstrap does, rather than through `attach_shared_instance`: it gets
// the registration announce, whose trigger is the posture alone (`RNS/Transport.py:2911-2916`
// keys on `Transport.owner.is_connected_to_shared_instance`), while nothing is attached, so
// the behaviours that live in the attach's consumer task are not in play. Helpers come from
// the sibling `tests_parts` files included into this module; the announce awaiter is the one
// `two_transport_shared_instance_parity.rs` defines.

/// A plain TCP connector with the posture set by hand registers a destination and the
/// shared instance receives it as a path response, with no announce from the test and no
/// attached interface on the connector.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_probe9_directly_set_posture_announces_on_registration() {
    let server = shared_instance_server().await;
    let connector =
        connector("shared-instance-probe9-posture", server.port, TcpClient::DEFAULT_MTU).await;
    server.await_accepted_connections(1).await;
    connector.transport.set_connected_to_shared_instance(true).await;
    assert!(
        connector.transport.shared_instance().is_none(),
        "the posture alone is set: nothing is attached"
    );
    assert!(connector.transport.connected_to_shared_instance().await, "the posture is on");
    let mut server_announces = server.transport.recv_announces().await;

    let identity = PrivateIdentity::new_from_rand(OsRng);
    let destination = connector
        .transport
        .add_destination(identity, DestinationName::new("shared-instance-probe9", "posture"))
        .await;
    let address_hash = destination.lock().await.desc.address_hash;

    let event = shared_instance_parity_await_announce(&mut server_announces, &address_hash).await;
    assert!(
        event.is_path_response,
        "the registration announce of a hand-set posture carries the path-response context"
    );

    timeout(STOP_TIMEOUT, connector.transport.stop()).await.expect("connector stop completes");
    timeout(STOP_TIMEOUT, server.transport.stop()).await.expect("shared instance stop completes");
}

/// A hand-set posture re-announces nothing sooner than this after the shared instance
/// accepts the connector again: an attach-driven client with the same `reconnect_wait`
/// would have re-announced at `reconnect_wait + 2 s`, so silence past that bound shows the
/// consumer task is not in play.
const SHARED_INSTANCE_PROBE9_SILENCE: Duration =
    SHARED_INSTANCE_RECONNECT_WAIT.saturating_add(Duration::from_secs(3));

/// The README's other half of the hand-set posture: a plain `TcpClient` whose transport
/// has the connected posture set directly reconnects after the shared instance restarts,
/// but neither the drop flush (its path survives) nor the reconnect re-announce (the
/// restarted instance hears nothing past `reconnect_wait + 2 s`) runs, since both live in
/// the consumer task only `attach_shared_instance` spawns. The client is built by hand
/// with the short `reconnect_wait` so the reconnect is observed quickly.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_probe9_directly_set_posture_neither_flushes_nor_reannounces_on_restart() {
    let server = shared_instance_server().await;
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let transport =
        Transport::new(TransportConfig::new("shared-instance-probe9-restart", &identity, false));
    let mut connector_announces = transport.recv_announces().await;
    let client = TcpClient::new(format!("127.0.0.1:{}", server.port))
        .with_mtu(TcpClient::DEFAULT_MTU)
        .with_reconnect_wait(SHARED_INSTANCE_RECONNECT_WAIT);
    let status = client.runtime_status_handle();
    let context = transport.iface_manager().lock().await.new_context(client);
    tokio::spawn(TcpClient::spawn(context));
    server.await_accepted_connections(1).await;
    transport.set_connected_to_shared_instance(true).await;
    assert!(transport.shared_instance().is_none(), "the posture alone is set: nothing is attached");

    // The posture half, as in the registration test: the connector's destination reaches
    // the shared instance as a path response, with no announce from the test.
    let mut server_announces = server.transport.recv_announces().await;
    let destination_identity = PrivateIdentity::new_from_rand(OsRng);
    let destination = transport
        .add_destination(
            destination_identity,
            DestinationName::new("shared-instance-probe9", "restart"),
        )
        .await;
    let address_hash = destination.lock().await.desc.address_hash;
    let event = shared_instance_parity_await_announce(&mut server_announces, &address_hash).await;
    assert!(event.is_path_response, "the registration announce is a path response");

    // The connector learns a path from the shared instance, the thing a drop flush would clear.
    let (_server_destination, desc) =
        register_in_destination(&server.transport, "shared-instance-probe9", "server").await;
    learn_announce(&mut connector_announces, &desc.address_hash).await;
    assert!(transport.has_path(&desc.address_hash).await, "the connector learned the path");

    let (restarted, mut restarted_announces, _) = shared_instance_reconnect_restart(server).await;
    wait_until(
        "connector to reconnect",
        || status.to_json()["stream_state"].as_str() == Some("connected"),
        || status.to_json().to_string(),
    )
    .await;

    let early = timeout(SHARED_INSTANCE_PROBE9_SILENCE, restarted_announces.recv()).await;
    assert!(
        early.is_err(),
        "a hand-set posture re-announces nothing after the reconnect: the re-announce lives in the attach's consumer"
    );
    assert!(
        transport.has_path(&desc.address_hash).await,
        "a hand-set posture flushes nothing on the drop: the path survives the restart"
    );

    timeout(STOP_TIMEOUT, transport.stop()).await.expect("connector stop completes");
    timeout(STOP_TIMEOUT, restarted.transport.stop())
        .await
        .expect("restarted shared instance stop completes");
}

/// The release notes say the warnings of the stop's join loop are logged on the links
/// target as `tp(<name>): shared instance task ...`, so the never-warned oracle sees them.
/// Positive control for that oracle: a registration announce left waiting on a handler
/// wedged inside its delay is still running when the stop budget runs out, and the abort
/// warning reaches the test logger under the transport's name. The guard is leaked on
/// purpose and never held across an await, as in the wedged-stop test of `probe2`.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_probe9_stop_abort_of_a_pending_announce_warns_under_the_transport_name() {
    install_test_logger();
    let name = "shared-instance-probe9-abort";
    let server = shared_instance_server().await;
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let transport = Transport::new(TransportConfig::new(name, &identity, false));
    let iface = transport
        .attach_shared_instance(server.endpoint(), SharedInstanceOptions::default())
        .await
        .expect("attach to the shared instance");
    server.await_accepted_connections(1).await;
    shared_instance_assert_attached(&transport, iface).await;
    shared_instance_assert_never_warned(name);

    // The announce task sleeps the registration delay and then needs the handler; the
    // handler is wedged inside that delay, so the task can never finish on its own.
    let destination_identity = PrivateIdentity::new_from_rand(OsRng);
    let _destination = transport
        .add_destination(
            destination_identity,
            DestinationName::new("shared-instance-probe9", "abort"),
        )
        .await;
    let handler = transport.get_handler();
    let wedge = handler.lock().await;
    std::mem::forget(wedge);
    // Past the delay the announce is parked on the wedged handler, where no cancel reaches it;
    // a task still in its delay would leave on the cancel instead.
    tokio::time::sleep(super::links::SHARED_INSTANCE_REGISTRATION_ANNOUNCE_DELAY.saturating_mul(2))
        .await;

    timeout(STOP_TIMEOUT + Duration::from_secs(3), transport.stop())
        .await
        .expect("stop returns once its budget is spent even with the handler wedged");

    let aborted: Vec<_> = shared_instance_records_for(name)
        .into_iter()
        .filter(|(level, record)| {
            *level == log::Level::Warn
                && record
                    .contains("shared instance task(s) still running after stop budget, aborted")
        })
        .collect();
    assert_eq!(aborted.len(), 1, "one abort warning per stop, under tp({name}): {aborted:?}");
    assert!(
        aborted[0].1.starts_with(&format!("tp({name}): ")),
        "the abort warning opens with the transport's name: {}",
        aborted[0].1
    );
    timeout(STOP_TIMEOUT, server.transport.stop()).await.expect("shared instance stop completes");
}

/// The same connector with the posture left off registers the same way and the shared
/// instance hears nothing within the registration delay: the flip is the whole trigger.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_probe9_posture_switched_off_before_registration_announces_nothing() {
    let server = shared_instance_server().await;
    let connector =
        connector("shared-instance-probe9-off", server.port, TcpClient::DEFAULT_MTU).await;
    server.await_accepted_connections(1).await;
    connector.transport.set_connected_to_shared_instance(true).await;
    connector.transport.set_connected_to_shared_instance(false).await;
    let mut server_announces = server.transport.recv_announces().await;

    let identity = PrivateIdentity::new_from_rand(OsRng);
    let _destination = connector
        .transport
        .add_destination(identity, DestinationName::new("shared-instance-probe9", "off"))
        .await;

    let early = timeout(SHARED_INSTANCE_PARITY_SILENCE, server_announces.recv()).await;
    assert!(early.is_err(), "a posture switched off before the registration announces nothing");

    timeout(STOP_TIMEOUT, connector.transport.stop()).await.expect("connector stop completes");
    timeout(STOP_TIMEOUT, server.transport.stop()).await.expect("shared instance stop completes");
}
