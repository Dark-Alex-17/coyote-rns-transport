// The roaming proof of `python_shared_instance_rpc_roaming.rs` across a
// restart of rnsd: a restart does not clear the dead 1-hop row, rnsd
// persists it on the way out and reads it back on start, and the same
// recipe moves it to the live route.
//
// Topology: the `roaming_topology` fixture (R2, R1, D, rnsd with the TCP
// upstreams A and B, C attached with state events), then A goes down as in
// the roaming proof. B's listener stays up throughout: the fixture fixes the
// A-first order before B exists, and rnsd's reconnect loop re-dials B after
// the respawn (`RNS/Interfaces/TCPInterface.py:270-296`, every
// `RECONNECT_WAIT` = 5 s, `:80`), so nothing is gained by opening B later.
//
// Proven here:
//  * SIGTERM runs `sigterm_handler` (`RNS/Reticulum.py:203-206`):
//    `Transport.detach_interfaces` (`RNS/Transport.py:3625-3687`) closes the
//    sockets but removes nothing from `Transport.interfaces`, then
//    `RNS.exit()` (`RNS/__init__.py:344-349`) runs `Reticulum.exit_handler`
//    and `Transport.exit_handler` (`RNS/Transport.py:3977-3981`), whose
//    `persist_data` (`:3962-3968`) calls `save_path_table` (`:3788-3863`);
//    its filter keeps an entry whose interface hash is still in
//    `interface_hashes()` (`:3820-3830`), which the offline A's is, so the
//    dead row lands in `<config_dir>/storage/destination_table`, and
//    `os._exit(0)` ends the process with status 0;
//  * rnsd respawned on the same config dir, control port and instance port
//    (CPython's `multiprocessing.connection.SocketListener` and
//    `RNS/Interfaces/LocalInterface.py:44-51` `ThreadingTCPServer.server_bind`
//    set `SO_REUSEADDR`) keeps its `storage/transport_identity`, so the old
//    RPC key still authenticates, and C's reconnect loop attaches again on
//    the same interface. The count that reconnect adds is read against the
//    count seen once the old process is reaped, not against 1: the dying
//    rnsd detaches its local clients before its shared-instance listener
//    (`RNS/Transport.py:3679-3684`), so C's redial can be accepted once more
//    by the dying instance before it exits, and that stream counts too;
//  * a transport-enabled rnsd restores its rows on start
//    (`RNS/Transport.py:404-441`), each interface found by
//    `find_interface_from_hash` (`:2987-2992`), the `full_hash` of
//    `str(interface)` with no `online` check, and each announce read from
//    `cache/announces/<packet_hash>`, written at `:2457` for every accepted
//    announce; the line `Loaded path table entry for <hex> from storage`
//    (`:442`, LOG_PATHING, the config's loglevel) appears while A's listener
//    is still gone, `path_table` lists the row at hops 1 under A's str, A
//    reads offline and B online: the dead route survives the restart;
//  * the restored row hands the dead route back as the stale one did. The
//    instance drop flushed C's own table (`shared_connection_disappeared`,
//    `RNS/Transport.py:3690-3702`, `Transport.path_table = {}` at `:3698`;
//    the Rust twin is pinned by
//    `rns-transport/src/transport/tests_parts/two_transport_shared_instance_flush.rs`),
//    so after the reconnect C holds no row and one explicit
//    `Transport::request_path` is answered from the restored entry (`path is
//    known`, no `Forwarding path request` line) at hops 1; a link C opens
//    over it closes with `LinkCloseReason::Timeout`, C expires its own row
//    and queues no rediscovery, and a second explicit request is answered
//    from the restored entry again;
//  * the recipe recovers it: `drop_path_settled` then one
//    `Transport::request_path` moves the row to B at hops 3 and a link over
//    it activates.
//
// A respawn that cannot bind the instance or control port is the
// environment, not the proof: `spawn_with_interfaces` panics on its start
// wait and the panic prints the new rnsd's output, which then carries the
// bind error. Unix only like its siblings; the constants, helpers and the
// fixture come from the roaming part, `next_state` from
// `python_shared_instance_connect_count.rs`.

#[cfg(unix)]
const RESTART_BUDGET: Duration = Duration::from_secs(150);
#[cfg(unix)]
const RESTART_STOP: Duration = Duration::from_secs(15);
#[cfg(unix)]
const RESTART_RECONNECT: Duration = Duration::from_secs(30);
#[cfg(unix)]
const RESTART_RESTORE: Duration = Duration::from_secs(10);

// One explicit request from C, answered by rnsd from the restored row: C
// learns hops 1 and rnsd logs the known-path line without forwarding
// anything upstream. Only log appended after this request counts, so a
// second call cannot be satisfied by the first call's known-path line.
// Returns the hops C was handed.
#[cfg(unix)]
#[deny(clippy::await_holding_invalid_type)]
async fn restart_request_answered_from_restored_row(
    c: &Transport,
    rnsd_log: &ProcessLogTail,
    hash: &AddressHash,
    hex: &str,
) -> u8 {
    let mut announces = c.recv_announces().await;
    let log_before = rnsd_log.snapshot().len();
    let dispatch = c.request_path(hash, None, None).await;
    assert!(dispatch.sent_ifaces + dispatch.queued_ifaces >= 1, "{dispatch:?}");
    let (hops, _, path_response) =
        path_recipe_wait_announce(&mut announces, *hash, ROAMING_REPATH).await;
    eprintln!("C's answer over the restored row: hops={hops} path_response={path_response}");
    assert_eq!(hops, 1, "the restored entry is handed back");
    assert_eq!(c.hops_to(hash).await, 1);
    let waited = tokio::time::Instant::now();
    let since_request = loop {
        let snapshot = rnsd_log.snapshot();
        let since_request = snapshot[log_before.min(snapshot.len())..].to_string();
        let known = path_recipe_lines_for(&since_request, PATH_RECIPE_KNOWN_LINE, hex);
        if let Some(known) = known.first() {
            eprintln!("rnsd: {known}");
            break since_request;
        }
        assert!(
            waited.elapsed() < ROAMING_REPATH,
            "the known-path answer for this request within {ROAMING_REPATH:?}"
        );
        tokio::time::sleep(PATH_LIVE_POLL).await;
    };
    let forwarded = path_recipe_lines_for(&since_request, PATH_RECIPE_FORWARDED_LINE, hex);
    assert!(
        forwarded.is_empty(),
        "with the restored row on file the request is answered, never forwarded: {forwarded:?}"
    );
    hops
}

#[cfg(unix)]
#[deny(clippy::await_holding_invalid_type)]
async fn restart_scenario(
    started: tokio::time::Instant,
    config_dir: &std::path::Path,
    rnsd_slot: &mut Option<SharedInstanceRpcRnsd>,
) {
    let (events_tx, mut events) = tokio::sync::mpsc::channel(1024);
    let c_options = SharedInstanceOptions::default()
        .with_reconnect_wait(Duration::from_millis(50))
        .with_disconnect_reasons(true)
        .with_state_events(events_tx);
    let RoamingTopology {
        r2,
        r1,
        d,
        c,
        a_server,
        a_name,
        b_name,
        hash,
        hex,
        control_port,
        endpoint,
        key,
        instance,
        interfaces,
        ..
    } = roaming_topology("restart", config_dir, c_options, rnsd_slot).await;

    // A goes down: the stats entry reads offline, the row does not move.
    assert!(d.stop_interface(a_server).await, "D's A listener stops");
    let (a_entry, offline_after) =
        roaming_wait_online(&endpoint, &key, "A", false, ROAMING_A_OFFLINE).await;
    eprintln!("A offline {offline_after:?} after D closed it: {a_entry:?}");
    let rows = path_table(&endpoint, &key, None, PATH_LIVE_CALL).await.expect("path_table");
    let stale = path_live_row(&rows, &hash).expect("the row survives A going offline");
    assert_eq!(stale.hops, 1, "{stale:?}");
    assert_eq!(stale.interface, a_name, "the stale row is never displaced: {stale:?}");

    // The attach's Connecting and Connected are still queued; from here on
    // the stream carries the restart alone.
    while events.try_recv().is_ok() {}
    let iface = c.shared_instance().expect("C is attached");
    assert_eq!(c.interface_connect_count(&iface).await, Some(1));
    let identity_file = config_dir.join("storage/transport_identity");
    let identity_before = fs::read(&identity_file).expect("the transport identity on disk");

    // SIGTERM: rnsd persists the table and exits 0; the dead row is in it.
    let rnsd = rnsd_slot.take().expect("the fixture spawned rnsd");
    let stop_started = tokio::time::Instant::now();
    let (status, output) = rnsd.stop_gracefully(RESTART_STOP).await;
    let stop_after = stop_started.elapsed();
    let table = config_dir.join("storage/destination_table");
    let table_len = fs::metadata(&table)
        .unwrap_or_else(|err| {
            panic!("{} after SIGTERM: {err}\nold rnsd output:\n{output}", table.display())
        })
        .len();
    assert!(
        table_len > 0,
        "{} is empty after SIGTERM\nold rnsd output:\n{output}",
        table.display()
    );
    let saved =
        output.lines().find(|line| line.contains(" path table entries in ")).map(str::to_string);
    eprintln!(
        "rnsd exited {status} {stop_after:?} after SIGTERM; {} holds {table_len} bytes; {saved:?}",
        table.display()
    );
    assert!(
        matches!(
            next_state("the disconnect after the stop", &mut events).await,
            SharedInstanceEvent::Disconnected | SharedInstanceEvent::DisconnectedWithReason { .. }
        ),
        "a stopped rnsd reads as a disconnect\nold rnsd output:\n{output}"
    );
    assert_eq!(
        next_state("the redial's Connecting", &mut events).await,
        SharedInstanceEvent::Connecting,
        "the client redials the stopped rnsd\nold rnsd output:\n{output}"
    );
    let mut while_stopping = Vec::new();
    while let Ok(event) = events.try_recv() {
        while_stopping.push(event);
    }
    eprintln!("states seen while the old rnsd shut down: {while_stopping:?}");
    let connects_before_respawn = c.interface_connect_count(&iface).await;
    eprintln!("connect count once the old rnsd is reaped: {connects_before_respawn:?}");
    let detached_local = output.lines().any(|line| line.contains("Detaching local clients"));
    let detached_shared = output.lines().any(|line| line.contains("Detaching shared instance"));
    eprintln!(
        "old rnsd logged Detaching local clients={detached_local} Detaching shared \
         instance={detached_shared}"
    );

    // The same dir, control port and instance port; the identity, and so the
    // RPC key, carry over, and C reconnects on its own.
    let respawn_started = tokio::time::Instant::now();
    let rnsd = rnsd_slot.insert(
        SharedInstanceRpcRnsd::spawn_with_interfaces(
            &instance,
            control_port,
            config_dir,
            &interfaces,
        )
        .await,
    );
    assert_eq!(
        fs::read(&identity_file).expect("the transport identity after the respawn"),
        identity_before,
        "the respawn keeps the transport identity"
    );
    let mut dial_failures = 0usize;
    tokio::time::timeout(RESTART_RECONNECT, async {
        loop {
            match next_state("the Connected after the respawn", &mut events).await {
                SharedInstanceEvent::Connected => break,
                SharedInstanceEvent::DialFailed { .. } => dial_failures += 1,
                other => eprintln!("state while rnsd restarts: {other:?}"),
            }
        }
    })
    .await
    .expect("the client reconnects to the respawned rnsd");
    let reconnect_after = respawn_started.elapsed();
    eprintln!("C reconnected {reconnect_after:?} after the respawn, {dial_failures} DialFailed");
    let connects = c.interface_connect_count(&iface).await;
    assert_eq!(
        connects,
        connects_before_respawn.map(|n| n + 1),
        "the reconnect to the respawned rnsd counts one more stream"
    );
    rnsd.poll_interface_stats(&endpoint, &key).await.expect("the old key still authenticates");
    roaming_wait_client_attached(&endpoint, &key).await;

    // The restore, read while A is still offline.
    let restore_started = tokio::time::Instant::now();
    let loaded = rnsd
        ._log
        .wait_for("the restored row's log line", RESTART_RESTORE, |line| {
            (line.contains("Loaded path table entry for ")
                && line.contains(&hex)
                && line.contains(" from storage"))
            .then(|| line.to_string())
        })
        .await;
    eprintln!("rnsd: {loaded}");
    let restored = roaming_wait_row(&endpoint, &key, &hash, 1, &a_name, RESTART_RESTORE).await;
    let restore_after = restore_started.elapsed();
    eprintln!("rnsd row restored from storage {restore_after:?} after the reconnect: {restored:?}");
    let (a_entry, _) = roaming_wait_online(&endpoint, &key, "A", false, RESTART_RESTORE).await;
    assert_eq!(a_entry.name, a_name, "{a_entry:?}");
    let (b_entry, b_online_after) =
        roaming_wait_online(&endpoint, &key, "B", true, ROAMING_B_ONLINE).await;
    assert_eq!(b_entry.name, b_name, "{b_entry:?}");
    eprintln!(
        "after the restore: A online={} B online={} ({b_online_after:?})",
        a_entry.online, b_entry.online
    );

    // The instance drop flushed C's own table (`shared_connection_disappeared`,
    // `RNS/Transport.py:3690-3702`, `Transport.path_table = {}` at `:3698`;
    // the Rust twin is `two_transport_shared_instance_flush.rs`), so the
    // restored row is first seen through one explicit request, which rnsd
    // answers from it. Then the link over it times out, C expires its row,
    // and a second request is answered from the restored entry again.
    let c_has_row = c.has_path(&hash).await;
    eprintln!("C's own row after the reconnect: {c_has_row}");
    assert!(!c_has_row, "the instance drop flushed C's own table");
    let c_hops_restored =
        restart_request_answered_from_restored_row(&c, &rnsd._log, &hash, &hex).await;
    let desc = c.destination_desc(&hash).await.expect("C holds D's description");
    let mut link_events = c.out_link_events();
    let link_started = tokio::time::Instant::now();
    let link = c.link(desc).await.expect("Ok: a path exists");
    let reason = wait_for_out_link_closed(&mut link_events, &link, ROAMING_LINK).await;
    let link_timeout_after = link_started.elapsed();
    eprintln!(
        "link over the restored row closed {link_timeout_after:?} after the request: {reason:?}"
    );
    assert_eq!(reason, Some(LinkCloseReason::Timeout));
    roaming_wait_path(&c, &hash, false, ROAMING_REPATH, "C drops the row of the timed-out link")
        .await;
    let c_hops_after_timeout =
        restart_request_answered_from_restored_row(&c, &rnsd._log, &hash, &hex).await;

    // The recipe: settle the drop, one request, the row moves to B.
    assert!(roaming_stats_entry(&endpoint, &key, "B").await.online);
    let recovery_started = tokio::time::Instant::now();
    let settled =
        drop_path_settled(&endpoint, &key, &hash, PATH_LIVE_CALL).await.expect("drop_path_settled");
    assert!(settled, "the restored row settles");
    assert!(c.expire_path(&hash).await, "C forgets the restored answer");
    let mut announces = c.recv_announces().await;
    let dispatch = c.request_path(&hash, None, None).await;
    assert!(dispatch.sent_ifaces + dispatch.queued_ifaces >= 1, "{dispatch:?}");
    let row_via_b = roaming_wait_row(&endpoint, &key, &hash, 3, &b_name, ROAMING_BRING_UP).await;
    eprintln!("rnsd row via B: {row_via_b:?}");
    let (c_hops_via_b, _, path_response) =
        path_recipe_wait_announce(&mut announces, hash, ROAMING_BRING_UP).await;
    eprintln!("C's answer over B: hops={c_hops_via_b} path_response={path_response}");
    assert_eq!(c_hops_via_b, 3, "D, R2, R1, rnsd; C takes the shared-instance hop back");
    let mut link_events = c.out_link_events();
    let link = c.link(desc).await.expect("Ok: the B route is a path");
    wait_for_out_link_active(&mut link_events, &link, ROAMING_LINK).await;
    let recovery_after = recovery_started.elapsed();

    let detached = tokio::time::timeout(ROAMING_TEARDOWN, c.detach_shared_instance())
        .await
        .expect("detach concludes");
    assert!(detached);
    for transport in [&c, &d, &r1, &r2] {
        tokio::time::timeout(ROAMING_TEARDOWN, transport.stop()).await.expect("stops");
    }
    rnsd.assert_running("at the end");
    eprintln!(
        "OBSERVED: A={a_name} B={b_name} stop={stop_after:?} destination_table={table_len} bytes \
         reconnect={reconnect_after:?} dial_failures={dial_failures} connects={connects:?} \
         restore={restore_after:?} restored_row hops={} via={} A_online={} B_online={} \
         c_row_after_reconnect={c_has_row} C_hops restored={c_hops_restored} \
         after_timeout={c_hops_after_timeout} \
         via_B={c_hops_via_b} link_timeout={link_timeout_after:?} recovery={recovery_after:?} \
         total={:?}",
        restored.hops,
        restored.interface,
        a_entry.online,
        b_entry.online,
        started.elapsed()
    );
}

#[cfg(unix)]
#[tokio::test]
#[ignore = "requires local Python Reticulum checkout"]
async fn python_rnsd_roaming_restart_restores_the_dead_one_hop_row_from_storage_until_drop_settled_moves_it_to_the_live_route(
) {
    let _interop_guard = python_interop_guard().await;
    let started = tokio::time::Instant::now();
    let temp = tempfile::tempdir().expect("tempdir");
    let config_dir = temp.path().join("python-rnsd-roaming-restart");
    let mut rnsd = None;
    let outcome =
        tokio::time::timeout(RESTART_BUDGET, restart_scenario(started, &config_dir, &mut rnsd))
            .await;
    roaming_expect_in_budget(outcome, &rnsd, "the restart scenario");
}
