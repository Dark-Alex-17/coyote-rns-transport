// Coverage for the reconnect consumer under event loss and under a storm of restarts: a
// reconnect that arrives without the disconnect of its cycle (the client worker's
// `try_send` dropped it on a full channel) still flushes the tables before the tunnel is
// re-synthesized, six back-to-back restarts of the TCP shared instance are each observed
// as one drop line followed by one reconnect line, with no event lost, and the drop flush
// sends no close packet for a link that rode the dropped interface itself, so the
// restarted instance receives no close for a link it never knew. Helpers come from the
// sibling `tests_parts` files included into this module.

const SHARED_INSTANCE_STORM_RESTARTS: usize = 6;

/// Each restart waits for the client's re-accept, which the TCP client reaches after at
/// most one refused redial plus its 100 ms wait; a minute leaves room for a loaded host.
const SHARED_INSTANCE_STORM_TIMEOUT: Duration = Duration::from_secs(60);

/// The drop lines and the reconnect lines of `records`, counted in that order.
fn shared_instance_storm_cycle_counts(records: &[String]) -> (usize, usize) {
    let dropped = records.iter().filter(|record| record.contains("disconnected,")).count();
    let reconnected = records
        .iter()
        .filter(|record| record.contains("reconnected, tunnel re-synthesized"))
        .count();
    (dropped, reconnected)
}

/// Polls the test logger until `name` has recorded at least `cycles` drop lines and
/// `cycles` reconnect lines; returns every record of `name` in arrival order.
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_storm_await_cycle_records(name: &str, cycles: usize) -> Vec<String> {
    let deadline = Instant::now() + LOOPBACK_TIMEOUT;
    loop {
        let records: Vec<String> =
            shared_instance_records_for(name).into_iter().map(|(_, record)| record).collect();
        let (dropped, reconnected) = shared_instance_storm_cycle_counts(&records);
        if dropped >= cycles && reconnected >= cycles {
            return records;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {cycles} drop and reconnect cycles of tp({name}), saw \
             {dropped} drop line(s) and {reconnected} reconnect line(s): {records:?}"
        );
        tokio::time::sleep(LOOPBACK_POLL).await;
    }
}

/// A reconnect event is only emitted after a previous connection existed, so one that
/// arrives with no disconnect served before it means the disconnect was dropped on a full
/// channel: the consumer flushes first, records the lost event, then re-synthesizes the
/// tunnel. The consumer is built by hand on a client attached to a live shared instance
/// and fed the reconnect alone; without the rule the learned path would survive.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_storm_reconnect_without_its_disconnect_flushes_first() {
    install_test_logger();
    let name = "shared-instance-storm-lost-disconnect";
    let server = shared_instance_server().await;
    let mut client = shared_instance_reconnect_client(name, &server).await;
    server.await_accepted_connections(1).await;
    shared_instance_assert_attached(&client.transport, client.iface).await;
    let (_destination, desc) =
        register_in_destination(&server.transport, "shared-instance-storm", "server").await;
    shared_instance_learn_announce(&mut client.announces, &desc.address_hash).await;
    assert!(client.transport.has_path(&desc.address_hash).await, "the client learned the path");

    let (reconnects_tx, reconnects) = tokio::sync::mpsc::channel(1);
    let (disconnects_tx, disconnects) = tokio::sync::mpsc::channel(1);
    reconnects_tx.send(client.iface).await.expect("queue the reconnect");
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

    shared_instance_reconnect_await_path_gone(&client.transport, &desc.address_hash).await;
    let records: Vec<String> =
        shared_instance_records_for(name).into_iter().map(|(_, record)| record).collect();
    let lost = records
        .iter()
        .position(|record| record.contains("reconnected without its disconnect event"))
        .unwrap_or_else(|| panic!("the lost disconnect is recorded: {records:?}"));
    let dropped = records
        .iter()
        .position(|record| record.contains("disconnected,"))
        .unwrap_or_else(|| panic!("the flush is recorded: {records:?}"));
    let reconnected = records
        .iter()
        .position(|record| record.contains("reconnected, tunnel re-synthesized"))
        .unwrap_or_else(|| panic!("the reconnect is recorded: {records:?}"));
    assert!(
        lost < dropped && dropped < reconnected,
        "the lost event is recorded, then the flush, then the reconnect: {records:?}"
    );
    shared_instance_assert_never_warned(name);

    timeout(STOP_TIMEOUT, client.transport.stop()).await.expect("client stop completes");
    timeout(STOP_TIMEOUT, server.transport.stop()).await.expect("shared instance stop completes");
}

/// `shared_connection_disappeared` (`RNS/Transport.py:3690-3702`) tears the links down on
/// a connection that is already gone. A link the client opened to the shared instance's
/// own destination rides the dropped interface, so its close packet is skipped rather than
/// queued for the restarted instance: the drop line counts it as skipped, the restarted
/// instance holds no link once the client's re-announce has landed, and neither transport
/// warns (every fixture's shared instance runs under the one name `shared-instance`, so
/// that check spans them all). With the skip removed the client's channel still accepts
/// the packet and the line counts it as sent.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_storm_drop_skips_the_close_packet_of_a_link_over_the_dropped_iface() {
    install_test_logger();
    let name = "shared-instance-storm-skipped-close";
    let server = shared_instance_server().await;
    let mut server_announces = server.transport.recv_announces().await;
    let mut client = shared_instance_reconnect_client(name, &server).await;
    server.await_accepted_connections(1).await;
    shared_instance_assert_attached(&client.transport, client.iface).await;
    let registered =
        shared_instance_reconnect_register(&client, &mut server_announces, "registered").await;
    let (_server_destination, _link_id) =
        shared_instance_reconnect_open_link(&server, &mut client).await;

    let (restarted, mut restarted_announces, _) = shared_instance_reconnect_restart(server).await;
    let record = shared_instance_probe10_await_debug_record(name, "disconnected,").await;
    assert!(
        record.contains(
            "1 link(s) torn down (0 close packet(s) sent, 1 skipped on the dropped interface)"
        ),
        "the link over the dropped interface sends no close packet: {record}"
    );

    shared_instance_reconnect_await_reannounces(&mut restarted_announces, vec![registered]).await;
    restarted.await_tunneled_destinations(vec![registered]).await;
    {
        let handler = restarted.transport.get_handler();
        let handler = handler.lock().await;
        assert!(handler.in_links.is_empty(), "the restarted instance accepted no link");
        assert!(handler.out_links.is_empty(), "the restarted instance opened no link");
    }
    shared_instance_assert_never_warned(name);

    timeout(STOP_TIMEOUT, client.transport.stop()).await.expect("client stop completes");
    timeout(STOP_TIMEOUT, restarted.transport.stop())
        .await
        .expect("restarted shared instance stop completes");
}

/// Six back-to-back restarts of the shared instance, each waited for up to the re-accept:
/// the consumer records one drop line and one reconnect line per restart, in record order
/// every drop line precedes the reconnect line of its cycle, and no reconnect arrives
/// without its disconnect, so the two channels lose nothing at this rate.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_storm_of_restarts_loses_no_event() {
    install_test_logger();
    let name = "shared-instance-storm-restarts";
    let server = shared_instance_server().await;
    let client = shared_instance_reconnect_client(name, &server).await;
    server.await_accepted_connections(1).await;
    shared_instance_assert_attached(&client.transport, client.iface).await;

    let started = Instant::now();
    let server = timeout(SHARED_INSTANCE_STORM_TIMEOUT, async {
        let mut server = server;
        for _ in 0..SHARED_INSTANCE_STORM_RESTARTS {
            let (restarted, _restarted_announces, _) =
                shared_instance_reconnect_restart(server).await;
            server = restarted;
        }
        server
    })
    .await
    .expect("the restarts complete inside the storm timeout");
    let storm_took = started.elapsed();

    let records =
        shared_instance_storm_await_cycle_records(name, SHARED_INSTANCE_STORM_RESTARTS).await;
    let (dropped, reconnected) = shared_instance_storm_cycle_counts(&records);
    assert_eq!(
        dropped, SHARED_INSTANCE_STORM_RESTARTS,
        "one drop line per restart after a {storm_took:?} storm: {records:?}"
    );
    assert_eq!(
        reconnected, SHARED_INSTANCE_STORM_RESTARTS,
        "one reconnect line per restart after a {storm_took:?} storm: {records:?}"
    );
    let mut dropped_so_far = 0usize;
    let mut reconnected_so_far = 0usize;
    for record in &records {
        if record.contains("disconnected,") {
            dropped_so_far += 1;
        } else if record.contains("reconnected, tunnel re-synthesized") {
            reconnected_so_far += 1;
            assert!(
                dropped_so_far >= reconnected_so_far,
                "reconnect line {reconnected_so_far} precedes the drop line of its cycle: \
                 {records:?}"
            );
        }
    }
    assert!(
        !records.iter().any(|record| record.contains("reconnected without its disconnect event")),
        "no disconnect was lost across {SHARED_INSTANCE_STORM_RESTARTS} restarts: {records:?}"
    );
    shared_instance_assert_never_warned(name);

    timeout(STOP_TIMEOUT, client.transport.stop()).await.expect("client stop completes");
    timeout(STOP_TIMEOUT, server.transport.stop()).await.expect("shared instance stop completes");
}
