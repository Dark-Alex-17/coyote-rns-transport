// Pins what the shared-instance client API says in the log, read through the binary's one
// logger: an attach never warns, the TCP attach line says the reconnect wait is ignored
// while the unix line reports it as in force, an attach whose interface was removed between
// its two sections records the race at debug, and a detach that finds a stale slot says so
// at debug.
// Helpers come from the sibling `tests_parts` files included into this module.

/// No shared-instance record (the lines the test logger captures under its links target)
/// at `Warn` or above may carry the transport's name across the scenario: the attach-time
/// missing-interface line and the stale-slot detach line are debug lines. Stop's own budget
/// warning lives in `lifecycle.rs` under another target and is outside this oracle.
fn shared_instance_assert_never_warned(transport_name: &str) {
    let loud: Vec<_> = shared_instance_records_for(transport_name)
        .into_iter()
        .filter(|(level, _)| *level <= log::Level::Warn)
        .collect();
    assert!(loud.is_empty(), "tp({transport_name}) logged at Warn or above:\n{loud:?}");
}

/// The debug records for `transport_name` whose text contains `needle`.
fn shared_instance_debug_records_containing(transport_name: &str, needle: &str) -> Vec<String> {
    shared_instance_records_for(transport_name)
        .into_iter()
        .filter(|(level, record)| *level == log::Level::Debug && record.contains(needle))
        .map(|(_, record)| record)
        .collect()
}

/// A TCP attach logs the attached interface at debug and says the configured reconnect wait
/// is ignored by the TCP client, since that client keeps its fixed retry; nothing about the
/// attach is logged at warn.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_tcp_attach_logs_the_reconnect_wait_as_ignored_and_never_warns() {
    install_test_logger();
    let name = "shared-instance-log-tcp";
    let server = shared_instance_server().await;
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let transport = Transport::new(TransportConfig::new(name, &identity, false));
    let iface = transport
        .attach_shared_instance(
            server.endpoint(),
            SharedInstanceOptions::default().with_reconnect_wait(Duration::from_millis(250)),
        )
        .await
        .expect("attach to the shared instance");
    server.await_accepted_connections(1).await;
    shared_instance_assert_attached(&transport, iface).await;

    let attached = shared_instance_debug_records_containing(name, "attached shared instance");
    assert_eq!(attached.len(), 1, "one attach line per attach: {attached:?}");
    let line = &attached[0];
    assert!(line.contains(&format!("iface={iface}")), "names the interface: {line}");
    assert!(line.contains("endpoint=tcp"), "names the endpoint kind: {line}");
    assert!(
        line.contains("reconnect_wait=250ms (ignored by the tcp client)"),
        "the TCP line reports the configured wait as ignored: {line}"
    );
    assert!(
        shared_instance_debug_records_containing(name, "tunnel not synthesized").is_empty(),
        "a plain attach synthesizes its tunnel and records no race"
    );
    shared_instance_assert_never_warned(name);

    assert!(timeout(STOP_TIMEOUT, transport.detach_shared_instance()).await.expect("detach"));
    shared_instance_assert_released(&transport, iface).await;
    let detached = shared_instance_debug_records_containing(name, "detached shared instance");
    assert_eq!(detached.len(), 1, "one detach line per detach: {detached:?}");
    assert!(
        shared_instance_debug_records_containing(name, "slot was stale").is_empty(),
        "a detach of a live interface says nothing about a stale slot"
    );
    shared_instance_assert_never_warned(name);
    timeout(STOP_TIMEOUT, server.transport.stop()).await.expect("shared instance stop completes");
}

/// A unix attach logs the reconnect wait as in force, with no mention of it being ignored,
/// because the `LocalUnixClient` honours it.
#[cfg(unix)]
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_unix_attach_logs_the_reconnect_wait_in_force() {
    install_test_logger();
    let name = "shared-instance-log-unix";
    let temp = tempfile::tempdir().expect("tempdir");
    let (owner, unix_endpoint) = shared_instance_unix_owner(temp.path()).await;
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let transport = Transport::new(TransportConfig::new(name, &identity, false));
    let iface = transport
        .attach_shared_instance(
            SharedInstanceEndpoint::Unix(unix_endpoint),
            SharedInstanceOptions::default().with_reconnect_wait(Duration::from_millis(250)),
        )
        .await
        .expect("attach to the unix shared instance");
    shared_instance_assert_attached(&transport, iface).await;

    let attached = shared_instance_debug_records_containing(name, "attached shared instance");
    assert_eq!(attached.len(), 1, "one attach line per attach: {attached:?}");
    let line = &attached[0];
    assert!(line.contains("endpoint=unix"), "names the endpoint kind: {line}");
    assert!(line.contains("reconnect_wait=250ms"), "reports the configured wait: {line}");
    assert!(!line.contains("ignored"), "the unix client honours the wait: {line}");
    shared_instance_assert_never_warned(name);

    timeout(STOP_TIMEOUT, transport.stop()).await.expect("client stop completes");
    timeout(STOP_TIMEOUT, owner.stop()).await.expect("unix shared instance stop completes");
}

/// After `remove_interface` took the attached interface out from under the slot, the detach
/// that clears the stale slot still reports `true` and records at debug that the slot was
/// stale, naming both results it got back; it never warns.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_detach_of_a_stale_slot_logs_at_debug_and_never_warns() {
    install_test_logger();
    let name = "shared-instance-log-stale";
    let server = shared_instance_server().await;
    let client = shared_instance_client(name, &server).await;
    server.await_accepted_connections(1).await;
    shared_instance_assert_attached(&client.transport, client.iface).await;
    assert!(
        timeout(STOP_TIMEOUT, client.transport.remove_interface(client.iface))
            .await
            .expect("remove_interface completes"),
        "remove_interface finds the attached interface"
    );
    assert_eq!(client.transport.shared_instance(), Some(client.iface), "the slot is stale");

    assert!(
        timeout(STOP_TIMEOUT, client.transport.detach_shared_instance())
            .await
            .expect("detach completes on a stale slot"),
        "detach reports the stale slot"
    );
    shared_instance_assert_released(&client.transport, client.iface).await;

    let stale = shared_instance_debug_records_containing(name, "slot was stale");
    assert_eq!(stale.len(), 1, "one stale-slot line per stale detach: {stale:?}");
    let line = &stale[0];
    assert!(line.contains(&format!("iface={client_iface}", client_iface = client.iface)), "{line}");
    assert!(
        line.contains("unflagged=false stopped=false"),
        "names what set_shared_instance and stop_interface reported: {line}"
    );
    let detached = shared_instance_debug_records_containing(name, "detached shared instance");
    assert_eq!(detached.len(), 1, "the ordinary detach line still follows: {detached:?}");
    shared_instance_assert_never_warned(name);

    timeout(STOP_TIMEOUT, client.transport.stop()).await.expect("client stop completes");
    timeout(STOP_TIMEOUT, server.transport.stop()).await.expect("shared instance stop completes");
}

/// An interface removed from the manager between the attach's two sections (what a detach
/// or stop does when it wins the window) leaves the attach nothing to synthesize its tunnel
/// on; the attach records that at debug, naming the race rather than an unknown interface,
/// still returns the interface, and nothing is logged at warn. The handler is held on a
/// blocking thread so the removal is certain to land before the attach's second section;
/// a detach racing for real reaches the same line only when its `stop_interface` lands
/// first, so the removal is driven through the manager directly here.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_attach_whose_interface_is_removed_in_the_window_logs_the_race_at_debug() {
    install_test_logger();
    let name = "shared-instance-log-race";
    let server = shared_instance_server().await;
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let transport = Transport::new(TransportConfig::new(name, &identity, false));
    let (stall, release) = stall_transport_until_released(&transport).await;

    let (attached, removed_in_window) = tokio::join!(
        async {
            timeout(
                STOP_TIMEOUT,
                transport
                    .attach_shared_instance(server.endpoint(), SharedInstanceOptions::default()),
            )
            .await
            .expect("the attach returns within its budget")
        },
        async {
            let slot = shared_instance_await_slot(&transport, true)
                .await
                .expect("the attach's first section filled the slot");
            // The attach's second section is parked on the stalled handler; the manager is
            // free, so the interface it spawned can be taken away before it resumes.
            let removed = { transport.iface_manager().lock().await.stop_interface(slot) };
            release.send(()).expect("the stall thread is waiting for the release");
            (slot, removed)
        },
    );
    stall.await.expect("the stall thread finishes once released");
    let iface = attached.expect("the attach still returns its interface");
    let (slot_in_window, removed) = removed_in_window;
    assert_eq!(slot_in_window, iface, "the slot named the attach's interface in the window");
    assert!(removed, "the manager removed the interface while the attach was parked");
    assert!(!transport.interface_hashes().await.contains(&iface), "the interface is gone");

    let raced = shared_instance_debug_records_containing(name, "tunnel not synthesized");
    assert_eq!(raced.len(), 1, "the lost race is recorded once: {raced:?}");
    let line = &raced[0];
    assert!(line.contains(&format!("iface={iface}")), "names the interface: {line}");
    assert!(
        line.contains("removed by a detach, stop or stop_interface that raced the attach"),
        "names the race rather than an unknown interface: {line}"
    );
    let attached_lines = shared_instance_debug_records_containing(name, "attached shared instance");
    assert_eq!(attached_lines.len(), 1, "the attach line still follows: {attached_lines:?}");
    shared_instance_assert_never_warned(name);

    // The slot is stale exactly as after `remove_interface`; detach clears it at debug.
    assert!(timeout(STOP_TIMEOUT, transport.detach_shared_instance()).await.expect("detach"));
    shared_instance_assert_released(&transport, iface).await;
    shared_instance_assert_never_warned(name);
    timeout(STOP_TIMEOUT, transport.stop()).await.expect("client stop completes");
    timeout(STOP_TIMEOUT, server.transport.stop()).await.expect("shared instance stop completes");
}
