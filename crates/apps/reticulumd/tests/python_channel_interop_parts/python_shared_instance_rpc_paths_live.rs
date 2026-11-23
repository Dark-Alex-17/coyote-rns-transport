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
//    `drop_path_settled` -> true within two cull ticks (observed inside one)
//    with `next_hop_interface_name` `None`; after another
//    `drop_all_via(row.via)` >= 1 and the row leaves the table.
//
// Unix only, like the io part: `SharedInstanceRpcRnsd::assert_running` is
// `#[cfg(unix)]`, and the target stays buildable on Windows. The path-call
// imports come from the CPython stdlib listener part
// (`python_stdlib_listener_rpc_probe.rs`), the stats ones from
// `python_shared_instance_rpc.rs`.

#[cfg(unix)]
use rns_transport::transport::shared_instance_rpc::PathRow;

#[cfg(unix)]
const PATH_LIVE_CALL: Duration = SHARED_INSTANCE_RPC_CALL;
#[cfg(unix)]
const PATH_LIVE_LISTED: Duration = Duration::from_secs(20);
#[cfg(unix)]
const PATH_LIVE_GONE: Duration = Duration::from_secs(5);
#[cfg(unix)]
const PATH_LIVE_POLL: Duration = Duration::from_millis(50);
// Two `job_interval` ticks (`RNS/Transport.py:248`): the cull lands on the
// next tick after the drop, observed in about 50 to 180 ms.
#[cfg(unix)]
const PATH_LIVE_SETTLE: Duration = Duration::from_millis(500);
// rnsd files a re-announce at equal hops only for a later emitted second
// (`RNS/Transport.py:2238`), so successive announces are spaced past one.
#[cfg(unix)]
const PATH_LIVE_ANNOUNCE_GAP: Duration = Duration::from_millis(1100);
/// A false `drop_path_settled` is one dial; the bound is a hang guard well
/// under `PATH_LIVE_CALL`.
#[cfg(unix)]
const PATH_LIVE_FALSE_DROP_RETURNS_WITHIN: Duration = Duration::from_secs(1);
/// Python's `DEFAULT_PER_HOP_TIMEOUT` (`RNS/Reticulum.py:142`), rnsd's whole
/// `first_hop_timeout` without a path and the floor of one with a path.
#[cfg(unix)]
const PATH_LIVE_DEFAULT_PER_HOP_TIMEOUT: f64 = 6.0;

#[cfg(unix)]
fn path_live_row(rows: &[PathRow], hash: &AddressHash) -> Option<PathRow> {
    rows.iter().find(|row| &row.hash == hash).cloned()
}

/// Polls `path_table(None)` until `hash` is listed (or not, per `listed`).
#[cfg(unix)]
#[deny(clippy::await_holding_invalid_type)]
async fn path_live_wait_listed(
    endpoint: &RpcEndpoint,
    key: &RpcKey,
    hash: &AddressHash,
    listed: bool,
    within: Duration,
) -> Vec<PathRow> {
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

#[cfg(unix)]
#[tokio::test]
#[ignore = "requires local Python Reticulum checkout"]
async fn python_rnsd_path_calls_list_the_client_interface_and_a_drop_settles() {
    let _interop_guard = python_interop_guard().await;
    let instance = live_tcp_shared_instance();
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
    assert_eq!(
        seconds, PATH_LIVE_DEFAULT_PER_HOP_TIMEOUT,
        "DEFAULT_PER_HOP_TIMEOUT without a path, the int 6 as f64"
    );
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
        settle_started.elapsed() < PATH_LIVE_FALSE_DROP_RETURNS_WITHIN,
        "a false drop is one dial: {:?}",
        settle_started.elapsed()
    );
    eprintln!("empty-table pass of seven dials took {:?}", started.elapsed());
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
    assert_eq!(
        row.via, rust_hash,
        "an announce heard without a transport header is filed with via == hash \
         (RNS/Transport.py:2177-2178, :2202): {row:?}"
    );
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
        seconds.is_finite() && (PATH_LIVE_DEFAULT_PER_HOP_TIMEOUT..60.0).contains(&seconds),
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
    let stats_names: Vec<&str> = stats.interfaces.iter().map(|iface| iface.name.as_str()).collect();
    let client_iface =
        stats.interfaces.iter().find(|iface| iface.name == row.interface).unwrap_or_else(|| {
            panic!(
                "the row's interface str {:?} is an interface_stats name: {stats_names:?}",
                row.interface
            )
        });
    eprintln!(
        "path row interface {:?} matches an interface_stats name of {stats_names:?}",
        row.interface
    );
    assert!(
        row.interface.starts_with("LocalInterface["),
        "the client's interface is the LocalClientInterface rnsd spawned for it \
         (RNS/Interfaces/LocalInterface.py:351-353): {client_iface:?}"
    );
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
        settle_started.elapsed() < PATH_LIVE_SETTLE,
        "the cull runs on the next Transport.jobs tick, job_interval 250 ms \
         (RNS/Transport.py:248), so settling takes under two ticks: {:?}",
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
    let detached = tokio::time::timeout(Duration::from_secs(5), transport.detach_shared_instance())
        .await
        .expect("detach concludes");
    assert!(detached, "the shared instance was attached");
    tokio::time::timeout(Duration::from_secs(5), transport.stop()).await.expect("transport stops");
    rnsd.assert_running("at the end");
}

// The consumer patterns the README recipe and the CHANGELOG retry rule
// promise, against a live rnsd: `path_table(Some(n))` keeps a row whose
// hops are at most `n` (`RNS/Reticulum.py:1641`, `path_hops <= max_hops`),
// so `Some(0)` and `Some(255)` both list a hops-0 row; the `interface_stats`
// entry a row is matched to by `name` is the one whose `online` the recipe
// reads, and the client's `LocalInterface[<port>]` is online while the client
// is attached; `drop_path_settled` repeated on a settled row is `Ok(false)`
// without polling ("a repeated call then answers `Ok(false)` once the cull
// has run, which a retrying caller reads as settled"); and `drop_all_via` of
// the row's `via` once the row is gone is `0`, then at least 1 again after a
// re-announce.
#[cfg(unix)]
#[tokio::test]
#[ignore = "requires local Python Reticulum checkout"]
async fn python_rnsd_path_calls_filter_by_hops_and_repeat_idempotently() {
    let _interop_guard = python_interop_guard().await;
    let instance = live_tcp_shared_instance();
    let temp = tempfile::tempdir().expect("tempdir");
    let control_port = free_tcp_port();
    let mut rnsd = SharedInstanceRpcRnsd::spawn(
        &instance,
        control_port,
        &temp.path().join("python-rnsd-path-rpc-idempotent"),
    )
    .await;
    let endpoint = RpcEndpoint::Tcp(SocketAddr::from(([127, 0, 0, 1], control_port)));
    let key = rnsd.identity_key().await;
    rnsd.poll_interface_stats(&endpoint, &key).await.expect("the rpc listener is up");

    let rust_identity = to_transport_private_identity(&PrivateIdentity::new_from_rand(OsRng));
    let config =
        TransportConfig::new("python-rnsd-path-rpc-idempotent-rust", &rust_identity, false);
    let transport = Transport::new(config);
    let destination = transport
        .add_destination(rust_identity.clone(), DestinationName::new("test", "pathrpcidem"))
        .await;
    let rust_hash = { destination.lock().await.desc.address_hash };
    transport
        .attach_shared_instance(instance.endpoint.clone(), SharedInstanceOptions::default())
        .await
        .expect("attach to the python shared instance");
    assert_eq!(transport.announce_destinations_to_shared_instance().await, 1);
    let last_announce = tokio::time::Instant::now();
    let rows = path_live_wait_listed(&endpoint, &key, &rust_hash, true, PATH_LIVE_LISTED).await;
    let row = path_live_row(&rows, &rust_hash).expect("listed");

    // The hops filter keeps rows at or under the bound.
    let at_zero =
        path_table(&endpoint, &key, Some(0), PATH_LIVE_CALL).await.expect("path_table(Some(0))");
    assert!(
        path_live_row(&at_zero, &rust_hash).is_some(),
        "a hops-{} row is listed under max_hops 0: {at_zero:?}",
        row.hops
    );
    let at_max = path_table(&endpoint, &key, Some(255), PATH_LIVE_CALL)
        .await
        .expect("path_table(Some(255))");
    assert_eq!(
        path_live_row(&at_max, &rust_hash),
        Some(row.clone()),
        "max_hops 255 lists the same row as the unfiltered read"
    );

    // The README recipe reads the matched entry's `online`.
    let stats = interface_stats(&endpoint, &key, PATH_LIVE_CALL).await.expect("interface_stats");
    let client_iface =
        stats.interfaces.iter().find(|iface| iface.name == row.interface).unwrap_or_else(|| {
            panic!("{:?} is a stats name: {:?}", row.interface, stats.interfaces)
        });
    eprintln!("client interface stats entry: {client_iface:?}");
    assert!(client_iface.online, "the attached client's interface is online: {client_iface:?}");

    // Settle once, then repeat: the retrying caller reads `Ok(false)`.
    let settled = drop_path_settled(&endpoint, &key, &rust_hash, PATH_LIVE_CALL)
        .await
        .expect("drop_path_settled");
    assert!(settled, "the listed row settles");
    let repeat_started = tokio::time::Instant::now();
    let repeated = drop_path_settled(&endpoint, &key, &rust_hash, PATH_LIVE_CALL)
        .await
        .expect("drop_path_settled repeated on the settled row");
    assert!(!repeated, "a settled row has nothing left to drop");
    assert!(
        repeat_started.elapsed() < PATH_LIVE_FALSE_DROP_RETURNS_WITHIN,
        "a false drop is one dial: {:?}",
        repeat_started.elapsed()
    );
    assert!(path_live_row(
        &path_table(&endpoint, &key, None, PATH_LIVE_CALL).await.expect("path_table"),
        &rust_hash
    )
    .is_none());

    // drop_all_via on the row's via with nothing left is 0; at least 1 again
    // once the destination is re-learned.
    let none_left =
        drop_all_via(&endpoint, &key, &row.via, PATH_LIVE_CALL).await.expect("drop_all_via empty");
    assert_eq!(none_left, 0, "no row goes via {:?} after the settle", row.via);
    tokio::time::sleep_until(last_announce + PATH_LIVE_ANNOUNCE_GAP).await;
    assert_eq!(transport.announce_destinations_to_shared_instance().await, 1);
    path_live_wait_listed(&endpoint, &key, &rust_hash, true, PATH_LIVE_LISTED).await;
    let count =
        drop_all_via(&endpoint, &key, &row.via, PATH_LIVE_CALL).await.expect("drop_all_via again");
    assert!(count >= 1, "the re-learned row goes via {:?}: {count}", row.via);
    path_live_wait_listed(&endpoint, &key, &rust_hash, false, PATH_LIVE_GONE).await;

    let detached = tokio::time::timeout(Duration::from_secs(5), transport.detach_shared_instance())
        .await
        .expect("detach concludes");
    assert!(detached);
    tokio::time::timeout(Duration::from_secs(5), transport.stop()).await.expect("transport stops");
    rnsd.assert_running("at the end");
}

// The two harness variants driven live, and the path contract under
// `enable_transport = yes`: `spawn_with_interfaces` hands rnsd the caller's
// whole `[interfaces]` text (a `TCPServerInterface`, no AutoInterface), so
// `interface_stats` lists that interface under `str(interface)`
// (`RNS/Interfaces/TCPInterface.py:689-695`) with the configured name as
// `short_name` and no AutoInterface; `with_transport(true)` writes
// `enable_transport = yes`, which rnsd reports as the `transport_id` and
// `transport_uptime` keys of the stats reply (`RNS/Reticulum.py:1614-1618`,
// absent with transport off). A Rust client's announce is then listed with
// `interface` equal to a stats `name` and unlike that entry's `short_name`,
// `first_hop_timeout` is inside `PATH_LIVE_DEFAULT_PER_HOP_TIMEOUT..60.0`, and
// `drop_path_settled` settles within two cull ticks with the next hop `None`
// and the row gone, the same as with transport off.
#[cfg(unix)]
#[tokio::test]
#[ignore = "requires local Python Reticulum checkout"]
async fn python_rnsd_with_transport_and_a_given_interfaces_text_serves_the_path_calls() {
    let _interop_guard = python_interop_guard().await;
    let instance = live_tcp_shared_instance().with_transport(true);
    let temp = tempfile::tempdir().expect("tempdir");
    let control_port = free_tcp_port();
    let listen_port = free_tcp_port();
    let interfaces = format!(
        "[interfaces]\n  [[live-tcp-server]]\n    type = TCPServerInterface\n    enabled = yes\n    listen_ip = 127.0.0.1\n    listen_port = {listen_port}\n"
    );
    let mut rnsd = SharedInstanceRpcRnsd::spawn_with_interfaces(
        &instance,
        control_port,
        &temp.path().join("python-rnsd-path-rpc-transport"),
        &interfaces,
    )
    .await;
    let endpoint = RpcEndpoint::Tcp(SocketAddr::from(([127, 0, 0, 1], control_port)));
    let key = rnsd.identity_key().await;
    let stats = rnsd.poll_interface_stats(&endpoint, &key).await.expect("the rpc listener is up");
    rnsd.assert_running("after the first interface_stats");

    // The interfaces text was used as given: the TCP server under its
    // `str(interface)` and configured name, and no AutoInterface at all.
    let expected_name = format!("TCPServerInterface[live-tcp-server/127.0.0.1:{listen_port}]");
    let tcp =
        stats.interfaces.iter().find(|iface| iface.short_name == "live-tcp-server").unwrap_or_else(
            || panic!("the configured interface is listed: {:?}", stats.interfaces),
        );
    assert_eq!(tcp.name, expected_name, "{tcp:?}");
    assert_eq!(tcp.kind, "TCPServerInterface", "{tcp:?}");
    assert!(tcp.online, "a loopback TCP server is up: {tcp:?}");
    assert!(
        stats.interfaces.iter().all(|iface| iface.kind != "AutoInterface"),
        "spawn_with_interfaces replaces the AutoInterface section: {:?}",
        stats.interfaces
    );
    // Transport is on: rnsd adds `transport_id` (a 16-byte bin) and
    // `transport_uptime` to the reply.
    let top = stats.raw.as_map().expect("the stats reply is a map");
    let top_key =
        |name: &str| top.iter().find(|(k, _)| k.as_str() == Some(name)).map(|(_, v)| v.clone());
    match top_key("transport_id") {
        Some(rmpv::Value::Binary(bytes)) => assert_eq!(bytes.len(), 16, "{bytes:?}"),
        other => panic!("enable_transport = yes puts a bin transport_id in the stats: {other:?}"),
    }
    assert!(
        matches!(top_key("transport_uptime"), Some(rmpv::Value::F64(_) | rmpv::Value::Integer(_))),
        "transport_uptime is a number: {:?}",
        top_key("transport_uptime")
    );

    // A Rust client announces one destination through the shared instance.
    let rust_identity = to_transport_private_identity(&PrivateIdentity::new_from_rand(OsRng));
    let config = TransportConfig::new("python-rnsd-path-rpc-transport-rust", &rust_identity, false);
    let transport = Transport::new(config);
    let destination = transport
        .add_destination(rust_identity.clone(), DestinationName::new("test", "pathrpctransport"))
        .await;
    let rust_hash = { destination.lock().await.desc.address_hash };
    transport
        .attach_shared_instance(instance.endpoint.clone(), SharedInstanceOptions::default())
        .await
        .expect("attach to the python shared instance");
    assert_eq!(transport.announce_destinations_to_shared_instance().await, 1);

    let rows = path_live_wait_listed(&endpoint, &key, &rust_hash, true, PATH_LIVE_LISTED).await;
    let row = path_live_row(&rows, &rust_hash).expect("listed");
    eprintln!("transport-enabled rnsd path row for the Rust destination: {row:?}");
    // The inbound hop (`RNS/Transport.py:1800`) is taken back for a local
    // client interface with no transport gate (`:1938`), so transport on or
    // off, the client's own announce is filed at zero hops.
    assert_eq!(row.hops, 0, "a local client's own announce is filed at zero hops: {row:?}");
    assert_eq!(row.via, rust_hash, "a directly heard announce is filed with via == hash: {row:?}");
    assert!(row.timestamp > 1.0e9, "wall-clock seconds: {row:?}");
    assert!(row.expires > row.timestamp, "{row:?}");
    assert!(
        row.interface.starts_with("LocalInterface["),
        "the client's interface is the LocalClientInterface rnsd spawned for it: {row:?}"
    );
    let stats = interface_stats(&endpoint, &key, PATH_LIVE_CALL).await.expect("stats after attach");
    let client_iface =
        stats.interfaces.iter().find(|iface| iface.name == row.interface).unwrap_or_else(|| {
            panic!(
                "the row's interface {:?} is an interface_stats name: {:?}",
                row.interface,
                stats.interfaces.iter().map(|iface| iface.name.as_str()).collect::<Vec<_>>()
            )
        });
    assert_ne!(client_iface.short_name, row.interface, "{client_iface:?}");
    let name = next_hop_interface_name(&endpoint, &key, &rust_hash, PATH_LIVE_CALL)
        .await
        .expect("next_hop_interface_name of the listed row");
    assert_eq!(name.as_deref(), Some(row.interface.as_str()));
    let seconds = first_hop_timeout(&endpoint, &key, &rust_hash, PATH_LIVE_CALL)
        .await
        .expect("first_hop_timeout of the listed row");
    assert!(
        seconds.is_finite() && (PATH_LIVE_DEFAULT_PER_HOP_TIMEOUT..60.0).contains(&seconds),
        "MTU * latency + DEFAULT_PER_HOP_TIMEOUT (RNS/Transport.py:3194-3197): {seconds}"
    );

    let settle_started = tokio::time::Instant::now();
    let settled = drop_path_settled(&endpoint, &key, &rust_hash, PATH_LIVE_CALL)
        .await
        .expect("drop_path_settled on the listed row");
    assert!(settled);
    eprintln!("transport-enabled drop_path_settled -> true in {:?}", settle_started.elapsed());
    assert!(settle_started.elapsed() < PATH_LIVE_SETTLE, "{:?}", settle_started.elapsed());
    let name = next_hop_interface_name(&endpoint, &key, &rust_hash, PATH_LIVE_CALL)
        .await
        .expect("next_hop_interface_name after settling");
    assert_eq!(name, None);
    let rows = path_table(&endpoint, &key, None, PATH_LIVE_CALL).await.expect("path_table");
    assert!(path_live_row(&rows, &rust_hash).is_none(), "{rows:?}");
    let again = drop_path(&endpoint, &key, &rust_hash, PATH_LIVE_CALL).await.expect("second drop");
    assert!(!again, "the gone entry drops as false");
    let detached = tokio::time::timeout(Duration::from_secs(5), transport.detach_shared_instance())
        .await
        .expect("detach concludes");
    assert!(detached, "the shared instance was attached");
    tokio::time::timeout(Duration::from_secs(5), transport.stop()).await.expect("transport stops");
    rnsd.assert_running("at the end");
}
