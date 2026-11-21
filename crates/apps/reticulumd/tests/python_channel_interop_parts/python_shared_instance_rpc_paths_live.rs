// The five path calls and the settling helper against a live Python rnsd
// (RNS 1.5.2), from the shared-instance path RPC as rnsd serves it
// (`RNS/Reticulum.py:1283-1306` requests, `:1638-1648` rows, `:1689-1695`
// drop all_via, `:1715` next hop str, `RNS/Transport.py:3194-3197` first-hop
// timeout, `:3219-3226` drop path). The CPython stdlib listener part pins the
// bytes on the wire with canned replies; this part proves rnsd itself reads
// those bytes and that its real replies decode.
//
// Proven here:
//  * an EMPTY table: `path_table` returns at once with `max_hops` nil and
//    with an int (a missing key would hang rnsd, `RNS/Reticulum.py:1283-1285`),
//    the unknown-hash calls read rnsd's `"None"`, int `6`, `False` and `0`,
//    and `drop_path_settled` on an unknown hash is `Ok(false)` at once;
//  * a destination announced from a Rust shared-instance client is listed
//    with rnsd's real `hash`/`via` bin16, float `timestamp`/`expires`,
//    `hops` 0 and `interface` str; `next_hop_interface_name` is `Some` of
//    that same str; `first_hop_timeout` is MTU * latency plus the default
//    per-hop timeout; a `drop_path` of a random hash is `false` and leaves
//    the listed row alone;
//  * `drop_path` -> true and the immediate re-read is RECORDED (the window
//    of `RNS/Transport.py:3222` + the `:855` cull), the entry is gone within
//    the cull interval, a second `drop_path` -> false; after a fresh announce
//    `drop_path_settled` -> true inside one cull tick with
//    `next_hop_interface_name` `None`; after another `drop_all_via(row.via)`
//    >= 1 and the row leaves the table.

const PATH_LIVE_CALL: Duration = Duration::from_secs(5);
const PATH_LIVE_LISTED: Duration = Duration::from_secs(20);
const PATH_LIVE_GONE: Duration = Duration::from_secs(5);
const PATH_LIVE_POLL: Duration = Duration::from_millis(50);
// rnsd files a re-announce at equal hops only for a later emitted second
// (`RNS/Transport.py:2238`), so successive announces are spaced past one.
const PATH_LIVE_ANNOUNCE_GAP: Duration = Duration::from_millis(1100);

fn path_live_row(
    rows: &[rns_transport::transport::shared_instance_rpc::PathRow],
    hash: &AddressHash,
) -> Option<rns_transport::transport::shared_instance_rpc::PathRow> {
    rows.iter().find(|row| &row.hash == hash).cloned()
}

/// Polls `path_table(None)` until `hash` is listed (or not, per `listed`).
#[deny(clippy::await_holding_invalid_type)]
async fn path_live_wait_listed(
    endpoint: &RpcEndpoint,
    key: &RpcKey,
    hash: &AddressHash,
    listed: bool,
    within: Duration,
) -> Vec<rns_transport::transport::shared_instance_rpc::PathRow> {
    let started = tokio::time::Instant::now();
    let mut last = Vec::new();
    while started.elapsed() < within {
        last = path_table(endpoint, key, None, PATH_LIVE_CALL).await.expect("path_table poll");
        if path_live_row(&last, hash).is_some() == listed {
            return last;
        }
        tokio::time::sleep(PATH_LIVE_POLL).await;
    }
    panic!(
        "path_table did not {} {hash} within {within:?}: {last:?}",
        if listed { "list" } else { "drop" }
    );
}

#[tokio::test]
#[ignore = "requires local Python Reticulum checkout"]
async fn python_rnsd_path_calls_list_the_client_interface_and_a_drop_settles() {
    let _interop_guard = python_interop_guard().await;
    let instance = live_tcp_shared_instance().with_transport(false);
    let temp = tempfile::tempdir().expect("tempdir");
    let control_port = free_tcp_port();
    let mut rnsd = SharedInstanceRpcRnsd::spawn(
        &instance,
        control_port,
        &temp.path().join("python-rnsd-path-rpc"),
    )
    .await;
    let endpoint = RpcEndpoint::Tcp(SocketAddr::from(([127, 0, 0, 1], control_port)));
    let key = rnsd.identity_key().await;
    rnsd.poll_interface_stats(&endpoint, &key).await.expect("the rpc listener is up");
    rnsd.assert_running("after the first interface_stats");

    // Empty table: none of the five hangs, every reply decodes.
    let unknown = AddressHash::new_from_rand(OsRng);
    let started = tokio::time::Instant::now();
    let rows = path_table(&endpoint, &key, None, PATH_LIVE_CALL).await.expect("path_table(None)");
    assert!(rows.is_empty(), "a fresh rnsd with no peers lists no paths: {rows:?}");
    let rows =
        path_table(&endpoint, &key, Some(0), PATH_LIVE_CALL).await.expect("path_table(Some(0))");
    assert!(rows.is_empty(), "{rows:?}");
    let name = next_hop_interface_name(&endpoint, &key, &unknown, PATH_LIVE_CALL)
        .await
        .expect("next_hop_interface_name on an unknown hash");
    assert_eq!(name, None, "rnsd's str \"None\" reads as None");
    let seconds = first_hop_timeout(&endpoint, &key, &unknown, PATH_LIVE_CALL)
        .await
        .expect("first_hop_timeout on an unknown hash");
    assert_eq!(seconds, 6.0, "DEFAULT_PER_HOP_TIMEOUT without a path, the int 6 as f64");
    let dropped =
        drop_path(&endpoint, &key, &unknown, PATH_LIVE_CALL).await.expect("drop_path unknown");
    assert!(!dropped, "no entry to drop");
    let count = drop_all_via(&endpoint, &key, &unknown, PATH_LIVE_CALL)
        .await
        .expect("drop_all_via unknown");
    assert_eq!(count, 0);
    let settle_started = tokio::time::Instant::now();
    let settled = drop_path_settled(&endpoint, &key, &unknown, PATH_LIVE_CALL)
        .await
        .expect("drop_path_settled unknown");
    assert!(!settled);
    assert!(
        settle_started.elapsed() < Duration::from_secs(2),
        "a false drop returns without polling: {:?}",
        settle_started.elapsed()
    );
    eprintln!("empty-table pass of six calls took {:?}", started.elapsed());
    rnsd.assert_running("after the empty-table calls");

    // A Rust client of the shared instance announces one destination.
    let rust_identity = to_transport_private_identity(&PrivateIdentity::new_from_rand(OsRng));
    let config = TransportConfig::new("python-rnsd-path-rpc-rust", &rust_identity, false);
    let transport = Transport::new(config);
    let destination = transport
        .add_destination(rust_identity.clone(), DestinationName::new("test", "pathrpc"))
        .await;
    let rust_hash = { destination.lock().await.desc.address_hash };
    transport
        .attach_shared_instance(instance.endpoint.clone(), SharedInstanceOptions::default())
        .await
        .expect("attach to the python shared instance");
    assert_eq!(transport.announce_destinations_to_shared_instance().await, 1);
    let mut last_announce = tokio::time::Instant::now();

    let rows = path_live_wait_listed(&endpoint, &key, &rust_hash, true, PATH_LIVE_LISTED).await;
    let row = path_live_row(&rows, &rust_hash).expect("listed");
    eprintln!("rnsd path row for the Rust destination: {row:?}");
    assert_eq!(row.hops, 0, "a local client's own announce is filed at zero hops: {row:?}");
    assert!(row.timestamp > 1.0e9, "wall-clock seconds: {row:?}");
    assert!(row.expires > row.timestamp, "{row:?}");
    assert!(!row.interface.is_empty(), "{row:?}");
    assert!(row.raw.is_map(), "{row:?}");
    let name = next_hop_interface_name(&endpoint, &key, &rust_hash, PATH_LIVE_CALL)
        .await
        .expect("next_hop_interface_name of the listed row");
    assert_eq!(
        name.as_deref(),
        Some(row.interface.as_str()),
        "the next hop is the row's interface str"
    );
    let seconds = first_hop_timeout(&endpoint, &key, &rust_hash, PATH_LIVE_CALL)
        .await
        .expect("first_hop_timeout of the listed row");
    assert!(
        seconds.is_finite() && (6.0..60.0).contains(&seconds),
        "rnsd 1.5.2 answers MTU * latency + DEFAULT_PER_HOP_TIMEOUT \
         (RNS/Transport.py:3194-3197), observed 6.000004 over a LocalInterface: {seconds}"
    );
    eprintln!("first_hop_timeout with a path: {seconds}");
    // The name equality (`RNS/Reticulum.py:1527` and `:1645` are the same
    // `str()` of the same object): the spawned `LocalClientInterface` is added
    // to `Transport.interfaces` (`RNS/Interfaces/LocalInterface.py:457`), so a
    // stats read taken AFTER the attach lists it under the row's str.
    let stats = interface_stats(&endpoint, &key, PATH_LIVE_CALL)
        .await
        .expect("interface_stats after the attach");
    let same_name = stats.interfaces.iter().any(|iface| iface.name == row.interface);
    eprintln!(
        "path row interface {:?} {} an interface_stats name (stats names: {:?})",
        row.interface,
        if same_name { "matches" } else { "does not match" },
        stats.interfaces.iter().map(|iface| iface.name.as_str()).collect::<Vec<_>>()
    );
    assert!(same_name, "the row's interface str is an interface_stats name");
    let client_iface =
        stats.interfaces.iter().find(|iface| iface.name == row.interface).expect("listed");
    assert_ne!(
        client_iface.short_name, row.interface,
        "short_name is str(interface.name), not str(interface): {client_iface:?}"
    );

    // A random hash while a path IS listed: false, and the listed row stays.
    let other = AddressHash::new_from_rand(OsRng);
    let dropped = drop_path(&endpoint, &key, &other, PATH_LIVE_CALL)
        .await
        .expect("drop_path of a random hash beside a listed row");
    assert!(!dropped, "a hash never announced has no entry to drop");
    let rows = path_table(&endpoint, &key, None, PATH_LIVE_CALL)
        .await
        .expect("path_table after the random-hash drop");
    assert!(
        path_live_row(&rows, &rust_hash).is_some(),
        "a false drop of another hash leaves the listed row alone: {rows:?}"
    );

    // drop_path -> true; the immediate re-read is recorded; the cull follows.
    let dropped = drop_path(&endpoint, &key, &rust_hash, PATH_LIVE_CALL).await.expect("drop_path");
    assert!(dropped, "the listed entry drops");
    let drop_done = tokio::time::Instant::now();
    let immediate = next_hop_interface_name(&endpoint, &key, &rust_hash, PATH_LIVE_CALL)
        .await
        .expect("next_hop_interface_name right after the drop");
    let immediate_rows = path_table(&endpoint, &key, None, PATH_LIVE_CALL)
        .await
        .expect("path_table right after the drop");
    eprintln!(
        "window: {:?} after drop_path next_hop={immediate:?} row={:?}",
        drop_done.elapsed(),
        path_live_row(&immediate_rows, &rust_hash)
    );
    if let Some(stale) = path_live_row(&immediate_rows, &rust_hash) {
        assert_eq!(
            stale.timestamp, 0.0,
            "a dropped-not-yet-culled row carries timestamp 0: {stale:?}"
        );
    }
    let gone_started = tokio::time::Instant::now();
    loop {
        let name = next_hop_interface_name(&endpoint, &key, &rust_hash, PATH_LIVE_CALL)
            .await
            .expect("next_hop_interface_name poll");
        if name.is_none() {
            break;
        }
        assert!(
            gone_started.elapsed() < PATH_LIVE_GONE,
            "next hop still {name:?} after {PATH_LIVE_GONE:?}"
        );
        tokio::time::sleep(PATH_LIVE_POLL).await;
    }
    eprintln!("next_hop_interface_name read None {:?} after drop_path", drop_done.elapsed());
    path_live_wait_listed(&endpoint, &key, &rust_hash, false, PATH_LIVE_GONE).await;
    let again =
        drop_path(&endpoint, &key, &rust_hash, PATH_LIVE_CALL).await.expect("second drop_path");
    assert!(!again, "the gone entry drops as false");

    // Re-learn, then the settling helper.
    tokio::time::sleep_until(last_announce + PATH_LIVE_ANNOUNCE_GAP).await;
    assert_eq!(transport.announce_destinations_to_shared_instance().await, 1);
    last_announce = tokio::time::Instant::now();
    path_live_wait_listed(&endpoint, &key, &rust_hash, true, PATH_LIVE_LISTED).await;
    let settle_started = tokio::time::Instant::now();
    let settled = drop_path_settled(&endpoint, &key, &rust_hash, PATH_LIVE_CALL)
        .await
        .expect("drop_path_settled on the re-learned row");
    assert!(settled);
    eprintln!("drop_path_settled -> true in {:?}", settle_started.elapsed());
    assert!(
        settle_started.elapsed() < Duration::from_millis(1000),
        "the cull runs on the next Transport.jobs tick, job_interval 250 ms \
         (RNS/Transport.py:248), so settling takes well under a second: {:?}",
        settle_started.elapsed()
    );
    let name = next_hop_interface_name(&endpoint, &key, &rust_hash, PATH_LIVE_CALL)
        .await
        .expect("next_hop_interface_name after settling");
    assert_eq!(name, None, "settled means the next hop reads None");
    let rows =
        path_table(&endpoint, &key, None, PATH_LIVE_CALL).await.expect("path_table after settling");
    assert!(path_live_row(&rows, &rust_hash).is_none(), "settled means the row is gone: {rows:?}");

    // Re-learn, then drop everything via the row's next hop.
    tokio::time::sleep_until(last_announce + PATH_LIVE_ANNOUNCE_GAP).await;
    assert_eq!(transport.announce_destinations_to_shared_instance().await, 1);
    let rows = path_live_wait_listed(&endpoint, &key, &rust_hash, true, PATH_LIVE_LISTED).await;
    let row = path_live_row(&rows, &rust_hash).expect("listed again");
    let count =
        drop_all_via(&endpoint, &key, &row.via, PATH_LIVE_CALL).await.expect("drop_all_via");
    assert!(count >= 1, "at least the Rust row goes via {:?}: {count}", row.via);
    path_live_wait_listed(&endpoint, &key, &rust_hash, false, PATH_LIVE_GONE).await;
    rnsd.assert_running("at the end");
}
