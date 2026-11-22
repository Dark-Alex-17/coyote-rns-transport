// The README `## Driving rnsd's path table` recipe followed as a consumer
// against a live Python rnsd (RNS 1.5.2), and the rule a path request obeys
// after a drop: sent once the drop has settled it is forwarded upstream,
// sent right after a bare `drop_path` it is answered from the dropped entry.
//
// Two Rust clients attach to one rnsd: C owns and announces a destination,
// C2 asks for a path to it. The recipe step `drop_path_settled` then one
// `Transport::request_path`: rnsd no longer holds the row, so it fans the
// request out to its other interfaces (`RNS/Transport.py:3523-3530`, the
// `[Pathing] Forwarding path request from local client` line), C answers
// with a FRESH announce (a new emission second) which rnsd files again and
// relays to C2 (`:2400-2429`). Pinned: the settle, the dispatch, the fresh
// emission at C2, the re-filed row with a wall-clock timestamp, the
// forwarding log line, and no cached answer for the destination after the
// drop.
//
// The contrast, RECORDED and pinned only when the window is hit: a bare
// `drop_path` then an immediate `request_path` is answered from the dropped
// entry (`:3459`, the `path is known` line) with the OLD emission second,
// and the row is culled on the next tick anyway, leaving rnsd without a row
// while C2 holds one.
//
// Unix only like its siblings (`assert_running`). The path-call imports come
// from `python_stdlib_listener_rpc_probe.rs`, the harness from
// `python_shared_instance_rpc.rs` and `python_shared_instance_live.rs`, the
// announce wait from `python_shared_instance_announce_emitted.rs`.

#[cfg(unix)]
const PATH_RECIPE_ANSWER: Duration = Duration::from_secs(10);
// rnsd's two accounts of a path request from a local client
// (`RNS/Transport.py:3484`, `:3526`).
#[cfg(unix)]
const PATH_RECIPE_KNOWN_LINE: &str = "path is known";
#[cfg(unix)]
const PATH_RECIPE_FORWARDED_LINE: &str = "Forwarding path request from local client";

#[cfg(unix)]
fn path_recipe_now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("wall clock after the epoch")
        .as_secs()
}

/// The next announce event for `hash` as `(hops, emitted, is_path_response)`.
#[cfg(unix)]
#[deny(clippy::await_holding_invalid_type)]
async fn path_recipe_wait_announce(
    announces: &mut tokio::sync::broadcast::Receiver<AnnounceEvent>,
    hash: AddressHash,
    within: Duration,
) -> (u8, u64, bool) {
    let event = live_wait_for_announce_event(announces, hash, within).await;
    (event.hops, event.emitted, event.is_path_response)
}

#[cfg(unix)]
fn path_recipe_lines_for<'a>(log: &'a str, needle: &str, hex: &str) -> Vec<&'a str> {
    log.lines().filter(|line| line.contains(needle) && line.contains(hex)).collect()
}

#[cfg(unix)]
#[tokio::test]
#[ignore = "requires local Python Reticulum checkout"]
async fn python_rnsd_recipe_drop_settled_then_request_path_is_forwarded_not_answered_stale() {
    let _interop_guard = python_interop_guard().await;
    let instance = live_tcp_shared_instance();
    let temp = tempfile::tempdir().expect("tempdir");
    let control_port = free_tcp_port();
    let mut rnsd = SharedInstanceRpcRnsd::spawn(
        &instance,
        control_port,
        &temp.path().join("python-rnsd-path-recipe"),
    )
    .await;
    let endpoint = RpcEndpoint::Tcp(SocketAddr::from(([127, 0, 0, 1], control_port)));
    let key = rnsd.identity_key().await;
    rnsd.poll_interface_stats(&endpoint, &key).await.expect("the rpc listener is up");

    // C owns the destination.
    let owner_identity = to_transport_private_identity(&PrivateIdentity::new_from_rand(OsRng));
    let owner = Transport::new(TransportConfig::new("path-recipe-owner", &owner_identity, false));
    let destination = owner
        .add_destination(owner_identity.clone(), DestinationName::new("test", "pathrecipe"))
        .await;
    let hash = { destination.lock().await.desc.address_hash };
    let hex = format!("<{}>", hex::encode(hash.as_slice()));
    owner
        .attach_shared_instance(instance.endpoint.clone(), SharedInstanceOptions::default())
        .await
        .expect("C attaches");
    assert_eq!(owner.announce_destinations_to_shared_instance().await, 1);
    let last_announce = tokio::time::Instant::now();
    let rows = path_live_wait_listed(&endpoint, &key, &hash, true, PATH_LIVE_LISTED).await;
    let first_row = path_live_row(&rows, &hash).expect("listed");
    eprintln!("row after C's announce: {first_row:?}");

    // C2 asks for paths. Attached after C's announce, so it holds no row.
    let asker_identity = to_transport_private_identity(&PrivateIdentity::new_from_rand(OsRng));
    let asker = Transport::new(TransportConfig::new("path-recipe-asker", &asker_identity, false));
    asker
        .attach_shared_instance(instance.endpoint.clone(), SharedInstanceOptions::default())
        .await
        .expect("C2 attaches");
    // C2's shared-instance interface is up once rnsd lists two local clients.
    let started = tokio::time::Instant::now();
    loop {
        let stats = interface_stats(&endpoint, &key, PATH_LIVE_CALL).await.expect("stats");
        let locals =
            stats.interfaces.iter().filter(|i| i.name.starts_with("LocalInterface[")).count();
        if locals >= 2 {
            break;
        }
        assert!(started.elapsed() < PATH_LIVE_LISTED, "C2 never attached: {stats:?}");
        tokio::time::sleep(PATH_LIVE_POLL).await;
    }
    assert!(!asker.has_path(&hash).await, "C2 attached after the announce holds no row");

    // ---- The README recipe: drop_path_settled, then ONE request_path. ----
    // A whole emission second past C's announce, so a fresh answer is told
    // apart from a replay by `emitted` alone.
    tokio::time::sleep_until(last_announce + PATH_LIVE_ANNOUNCE_GAP).await;
    let log_before = rnsd._log.snapshot().len();
    let settled =
        drop_path_settled(&endpoint, &key, &hash, PATH_LIVE_CALL).await.expect("drop_path_settled");
    assert!(settled, "the listed row settles");
    assert_eq!(
        next_hop_interface_name(&endpoint, &key, &hash, PATH_LIVE_CALL).await.expect("next hop"),
        None
    );
    let mut announces = asker.recv_announces().await;
    let request_second = path_recipe_now_secs();
    let dispatch = asker.request_path(&hash, None, None).await;
    assert!(
        dispatch.sent_ifaces + dispatch.queued_ifaces >= 1,
        "the request left C2 over the shared instance: {dispatch:?}"
    );
    let (hops, emitted, is_path_response) =
        path_recipe_wait_announce(&mut announces, hash, PATH_RECIPE_ANSWER).await;
    eprintln!(
        "recipe answer at C2: hops={hops} emitted={emitted} request_second={request_second} \
         path_response={is_path_response} (C's first announce emitted ~{})",
        first_row.timestamp as u64
    );
    assert!(
        emitted >= request_second,
        "after the settle the answer is C's FRESH announce (emitted {emitted}), \
         not a replay of the dropped entry (announced at ~{})",
        first_row.timestamp as u64
    );
    assert!(asker.has_path(&hash).await, "C2 learned the path");
    // rnsd re-filed the row from C's answer: a wall-clock timestamp again.
    let rows = path_live_wait_listed(&endpoint, &key, &hash, true, PATH_LIVE_LISTED).await;
    let refiled = path_live_row(&rows, &hash).expect("re-filed");
    assert!(refiled.timestamp > 1.0e9, "a fresh row, not the dropped int-0 one: {refiled:?}");
    assert!(refiled.timestamp >= first_row.timestamp, "{refiled:?} vs {first_row:?}");
    assert_eq!(refiled.hops, 0, "{refiled:?}");
    assert_eq!(refiled.interface, first_row.interface, "same client interface: {refiled:?}");
    // rnsd's own account: the request was forwarded, never answered from cache.
    let forwarded = rnsd
        ._log
        .wait_for("the forwarding line", PATH_RECIPE_ANSWER, |line| {
            (line.contains(PATH_RECIPE_FORWARDED_LINE) && line.contains(&hex))
                .then(|| line.to_string())
        })
        .await;
    eprintln!("rnsd: {forwarded}");
    let since_drop = rnsd._log.snapshot();
    let since_drop = &since_drop[log_before.min(since_drop.len())..];
    let cached = path_recipe_lines_for(since_drop, PATH_RECIPE_KNOWN_LINE, &hex);
    assert!(
        cached.is_empty(),
        "after drop_path_settled no path request for the destination is answered from the \
         dropped entry: {cached:?}"
    );
    rnsd.assert_running("after the recipe");

    // ---- The contrast, recorded: bare drop_path then an immediate request. ----
    tokio::time::sleep_until(last_announce + 2 * PATH_LIVE_ANNOUNCE_GAP).await;
    assert_eq!(owner.announce_destinations_to_shared_instance().await, 1);
    let rows = path_live_wait_listed(&endpoint, &key, &hash, true, PATH_LIVE_LISTED).await;
    let relearned = path_live_row(&rows, &hash).expect("re-learned");
    tokio::time::sleep(PATH_LIVE_ANNOUNCE_GAP).await;
    assert!(asker.expire_path(&hash).await, "C2 forgets its row so the next answer is an event");
    let log_before = rnsd._log.snapshot().len();
    let mut announces = asker.recv_announces().await;
    let dropped = drop_path(&endpoint, &key, &hash, PATH_LIVE_CALL).await.expect("bare drop");
    assert!(dropped);
    let drop_done = tokio::time::Instant::now();
    let request_second = path_recipe_now_secs();
    let dispatch = asker.request_path(&hash, None, None).await;
    assert!(dispatch.sent_ifaces + dispatch.queued_ifaces >= 1, "{dispatch:?}");
    let (hops, emitted, is_path_response) =
        path_recipe_wait_announce(&mut announces, hash, PATH_RECIPE_ANSWER).await;
    let answered_at = drop_done.elapsed();
    tokio::time::sleep(Duration::from_millis(600)).await; // past one cull tick
    let log = rnsd._log.snapshot();
    let since = &log[log_before.min(log.len())..];
    let stale = path_recipe_lines_for(since, PATH_RECIPE_KNOWN_LINE, &hex);
    let forwarded = path_recipe_lines_for(since, PATH_RECIPE_FORWARDED_LINE, &hex);
    let rows_after = path_table(&endpoint, &key, None, PATH_LIVE_CALL).await.expect("path_table");
    let row_after = path_live_row(&rows_after, &hash);
    eprintln!(
        "WINDOW RECORD: request right after a bare drop_path; answer at C2 {answered_at:?}: \
         hops={hops} emitted={emitted} request_second={request_second} path_response={is_path_response}; \
         rnsd 'path is known' lines={} forwarding lines={}; rnsd row 600 ms later={row_after:?}",
        stale.len(),
        forwarded.len()
    );
    if !stale.is_empty() {
        // The window was hit: rnsd replayed the dropped entry's cached
        // announce (its emission second is the re-announce's, before the
        // request), and still culled the row on the next tick.
        assert!(
            emitted < request_second,
            "a cached answer carries the OLD emission second: {emitted} vs {request_second}"
        );
        assert!(
            emitted <= relearned.timestamp as u64 + 1,
            "the replay is the re-announce on file (~{}): {emitted}",
            relearned.timestamp as u64
        );
        assert!(
            row_after.is_none(),
            "the dropped row is culled even though it just answered: {row_after:?}"
        );
        assert!(forwarded.is_empty(), "a cached answer is not also forwarded: {forwarded:?}");
        eprintln!("WINDOW HIT: stale answer confirmed");
    } else {
        assert!(!forwarded.is_empty(), "the request was either answered from cache or forwarded");
        eprintln!("WINDOW MISSED: the cull landed before the request; forwarded instead");
    }

    for transport in [&owner, &asker] {
        let detached =
            tokio::time::timeout(Duration::from_secs(5), transport.detach_shared_instance())
                .await
                .expect("detach concludes");
        assert!(detached);
        tokio::time::timeout(Duration::from_secs(5), transport.stop()).await.expect("stops");
    }
    rnsd.assert_running("at the end");
}
