// The README `## Driving rnsd's path table` recipe walked end to end on
// loopback against a live Python rnsd (RNS 1.5.2) with two TCP upstreams,
// the roaming case the path calls exist for.
//
// Topology, every port from `free_tcp_port()`: D is a Rust transport that
// hosts the destination, with a `TcpServer` (rnsd's upstream A dials it)
// and a `TcpClient` into R2; R2 and R1 are transport-enabled Rust relays,
// R1 dialling R2 and later opening the `TcpServer` rnsd's upstream B dials;
// rnsd runs `enable_transport = yes` with the two `TCPClientInterface`s A
// and B; C is a Rust shared-instance client of rnsd with no destination.
//
// Proven here:
//  * rnsd files D's path response over A at hops 1 under the str
//    `TCPInterface[A/<ip>:<port>]` (`RNS/Interfaces/TCPInterface.py:457-463`),
//    which is the `interface_stats` `name` of the entry whose `short_name`
//    is `A` (`RNS/Reticulum.py:1527-1528`), and C hears it at hops 1 too: a
//    client of a shared instance takes one hop back on every packet it
//    hears from the instance (`RNS/Transport.py:1940`), which this crate's
//    shared-instance client mirrors, so C's hop counts equal rnsd's rows;
//  * A's peer closing sets the interface offline and starts the reconnect
//    loop, which keeps it in `Transport.interfaces` and retries every
//    `RECONNECT_WAIT` seconds (`RNS/Interfaces/TCPInterface.py:415`,
//    `:270-296`, `:80`), so the stats entry reads `online == false`
//    (`RNS/Reticulum.py:1560`) while the 1-hop row stays: a row is replaced
//    only by an unheard announce with no more hops and a newer emission, or
//    with more hops once the row has expired (`RNS/Transport.py:2236-2286`),
//    never for its interface going offline;
//  * a link C opens over the dead row is relayed by rnsd onto the offline
//    interface, where the send is a silent no-op
//    (`RNS/Interfaces/TCPInterface.py:313-314`), and closes with
//    `LinkCloseReason::Timeout`; on that timeout C expires its own row and,
//    being attached to a shared instance, queues no rediscovery
//    (`crates/libs/rns-transport/src/transport/jobs_parts/link_maintenance.rs:130-144`),
//    so the test sends one explicit `Transport::request_path`, which rnsd
//    answers from its stale entry (`RNS/Transport.py:3459`, the `path is
//    known` log line, no `Forwarding path request` line) at hops 1 again:
//    the dead route is handed back until the row is dropped on rnsd's side;
//  * the recipe, `drop_path_settled` then one `Transport::request_path`:
//    rnsd forwards the request upstream (`RNS/Transport.py:3523-3530`), the
//    answer arrives over B, the row now reads hops 3 (D, R2, R1, rnsd) under
//    the B str, `next_hop_interface_name` names B, C hears hops 3, and a
//    link over the new row activates.
//
// Unix only like its siblings (`assert_running`). The path-call imports come
// from `python_stdlib_listener_rpc_probe.rs`, the row helpers and `PathRow`
// from `python_shared_instance_rpc_paths_live.rs`, the announce wait from
// `python_shared_instance_rpc_paths_recipe_probe.rs`, the harness from
// `python_shared_instance_rpc.rs` and `python_shared_instance_live.rs`. The
// bring-up is the `roaming_topology` fixture, shared with the recipe budget
// probe and the restart part.

#[cfg(unix)]
use rns_transport::destination::link::LinkCloseReason;
#[cfg(unix)]
use rns_transport::iface::tcp_server::TcpListenerRuntimeStatusHandle;
#[cfg(unix)]
use rns_transport::transport::TransportPathStatus;

#[cfg(unix)]
const ROAMING_BUDGET: Duration = Duration::from_secs(120);
#[cfg(unix)]
const ROAMING_BRING_UP: Duration = Duration::from_secs(15);
// rnsd serves one RPC dial at a time on `rpc_loop` (`RNS/Reticulum.py:1274-1334`);
// the status polls that can run for seconds dial it at a gentler cadence.
#[cfg(unix)]
const ROAMING_STATS_POLL: Duration = Duration::from_millis(250);
// rnsd's reconnect loop dials B every `RECONNECT_WAIT` = 5 s
// (`RNS/Interfaces/TCPInterface.py:80`, `:276`).
#[cfg(unix)]
const ROAMING_B_ONLINE: Duration = Duration::from_secs(10);
#[cfg(unix)]
const ROAMING_A_OFFLINE: Duration = Duration::from_secs(30);
// The first hop's timeout plus `ESTABLISHMENT_TIMEOUT_PER_HOP` at C's one
// hop, about 12 s (`transport_sections/link_establishment.rs`), with room
// for the maintenance tick that closes it.
#[cfg(unix)]
const ROAMING_LINK: Duration = Duration::from_secs(30);
#[cfg(unix)]
const ROAMING_REPATH: Duration = Duration::from_secs(10);
#[cfg(unix)]
const ROAMING_TEARDOWN: Duration = Duration::from_secs(5);

#[cfg(unix)]
fn roaming_relay(name: &str) -> Transport {
    let identity = to_transport_private_identity(&PrivateIdentity::new_from_rand(OsRng));
    let mut config = TransportConfig::new(name, &identity, false);
    config.set_transport_enabled(true);
    config.set_announce_retry_limit(0);
    Transport::new(config)
}

#[cfg(unix)]
#[deny(clippy::await_holding_invalid_type)]
async fn roaming_wait_until(what: &str, within: Duration, mut ready: impl FnMut() -> bool) {
    let started = tokio::time::Instant::now();
    while !ready() {
        assert!(started.elapsed() < within, "{what} did not happen within {within:?}");
        tokio::time::sleep(PATH_LIVE_POLL).await;
    }
}

/// Spawns a `TcpServer` on `port` and waits for it to listen; returns the
/// listener's address hash and its status handle.
#[cfg(unix)]
#[deny(clippy::await_holding_invalid_type)]
async fn roaming_listen(
    transport: &Transport,
    port: u16,
) -> (AddressHash, TcpListenerRuntimeStatusHandle) {
    let server = TcpServer::new(format!("127.0.0.1:{port}"), transport.iface_manager());
    let status = server.runtime_status_handle();
    let iface = transport.iface_manager().lock().await.spawn(server, TcpServer::spawn);
    roaming_wait_until(&format!("the server on {port} listening"), ROAMING_BRING_UP, || {
        status.to_json()["listener_state"].as_str() == Some("listening")
    })
    .await;
    (iface, status)
}

/// Dials the server on `port` and waits for the stream and for `server` to
/// count `accepted` connections; returns the client's address hash.
#[cfg(unix)]
#[deny(clippy::await_holding_invalid_type)]
async fn roaming_dial(
    transport: &Transport,
    port: u16,
    server: &TcpListenerRuntimeStatusHandle,
    accepted: u64,
) -> AddressHash {
    let client = TcpClient::new(format!("127.0.0.1:{port}"));
    let status = client.runtime_status_handle();
    let iface = transport.iface_manager().lock().await.spawn(client, TcpClient::spawn);
    roaming_wait_until(&format!("the client of {port} connecting"), ROAMING_BRING_UP, || {
        status.to_json()["stream_state"].as_str() == Some("connected")
    })
    .await;
    roaming_wait_until(
        &format!("the server on {port} accepting connection {accepted}"),
        ROAMING_BRING_UP,
        || server.to_json()["accepted_connections"].as_u64() == Some(accepted),
    )
    .await;
    iface
}

/// Polls `path_status` until `found` matches whether a path is held.
#[cfg(unix)]
#[deny(clippy::await_holding_invalid_type)]
async fn roaming_wait_path(
    transport: &Transport,
    hash: &AddressHash,
    found: bool,
    within: Duration,
    what: &str,
) -> TransportPathStatus {
    let started = tokio::time::Instant::now();
    loop {
        let status = transport.path_status(hash).await;
        if status.path_found == found {
            return status;
        }
        assert!(started.elapsed() < within, "{what} within {within:?}: {status:?}");
        tokio::time::sleep(PATH_LIVE_POLL).await;
    }
}

#[cfg(unix)]
#[deny(clippy::await_holding_invalid_type)]
async fn roaming_stats_entry(
    endpoint: &RpcEndpoint,
    key: &RpcKey,
    short_name: &str,
) -> InterfaceStat {
    let stats = interface_stats(endpoint, key, PATH_LIVE_CALL).await.expect("interface_stats");
    stats
        .interfaces
        .iter()
        .find(|iface| iface.short_name == short_name)
        .cloned()
        .unwrap_or_else(|| panic!("no {short_name} entry in {:?}", stats.interfaces))
}

/// Polls `interface_stats` until the entry named `short_name` reads
/// `online`; returns the entry and how long that took.
#[cfg(unix)]
#[deny(clippy::await_holding_invalid_type)]
async fn roaming_wait_online(
    endpoint: &RpcEndpoint,
    key: &RpcKey,
    short_name: &str,
    online: bool,
    within: Duration,
) -> (InterfaceStat, Duration) {
    let started = tokio::time::Instant::now();
    loop {
        let entry = roaming_stats_entry(endpoint, key, short_name).await;
        if entry.online == online {
            return (entry, started.elapsed());
        }
        assert!(
            started.elapsed() < within,
            "{short_name} never read online == {online} within {within:?}: {entry:?}"
        );
        tokio::time::sleep(ROAMING_STATS_POLL).await;
    }
}

/// Polls the path calls until `next_hop_interface_name` names `interface`
/// and the row for `hash` reads `hops` over it.
#[cfg(unix)]
#[deny(clippy::await_holding_invalid_type)]
async fn roaming_wait_row(
    endpoint: &RpcEndpoint,
    key: &RpcKey,
    hash: &AddressHash,
    hops: u8,
    interface: &str,
    within: Duration,
) -> PathRow {
    let started = tokio::time::Instant::now();
    loop {
        let name = next_hop_interface_name(endpoint, key, hash, PATH_LIVE_CALL)
            .await
            .expect("next_hop_interface_name poll");
        let rows = path_table(endpoint, key, None, PATH_LIVE_CALL).await.expect("path_table poll");
        let row = path_live_row(&rows, hash);
        if name.as_deref() == Some(interface) {
            if let Some(row) =
                row.clone().filter(|row| row.hops == hops && row.interface == interface)
            {
                return row;
            }
        }
        assert!(
            started.elapsed() < within,
            "rnsd never filed {hash} at hops {hops} over {interface:?} within {within:?}: \
             next_hop={name:?} row={row:?}"
        );
        tokio::time::sleep(ROAMING_STATS_POLL).await;
    }
}

/// Takes A down as every roaming scenario does: D stops the listener A
/// dials, the stats entry for A reads offline, and the hops-1 row for `hash`
/// still names A. Returns that row and how long the offline read took.
#[cfg(unix)]
#[deny(clippy::await_holding_invalid_type)]
async fn roaming_take_a_down(
    d: &Transport,
    a_server: AddressHash,
    endpoint: &RpcEndpoint,
    key: &RpcKey,
    hash: &AddressHash,
    a_name: &str,
) -> (PathRow, Duration) {
    assert!(d.stop_interface(a_server).await, "D's A listener stops");
    let (a_entry, offline_after) =
        roaming_wait_online(endpoint, key, "A", false, ROAMING_A_OFFLINE).await;
    eprintln!("A offline {offline_after:?} after D closed it: {a_entry:?}");
    let rows = path_table(endpoint, key, None, PATH_LIVE_CALL).await.expect("path_table");
    let stale = path_live_row(&rows, hash).expect("the row survives A going offline");
    assert_eq!(stale.hops, 1, "{stale:?}");
    assert_eq!(stale.interface, a_name, "the stale row is never displaced: {stale:?}");
    (stale, offline_after)
}

/// Polls `interface_stats` until a `LocalInterface[` entry lists C as a
/// client of rnsd (the `LocalClientInterface` joins the stats on accept).
#[cfg(unix)]
#[deny(clippy::await_holding_invalid_type)]
async fn roaming_wait_client_attached(endpoint: &RpcEndpoint, key: &RpcKey) {
    let attach_started = tokio::time::Instant::now();
    loop {
        let stats = interface_stats(endpoint, key, PATH_LIVE_CALL).await.expect("stats");
        if stats.interfaces.iter().any(|iface| iface.name.starts_with("LocalInterface[")) {
            break;
        }
        assert!(attach_started.elapsed() < ROAMING_BRING_UP, "C never attached: {stats:?}");
        tokio::time::sleep(PATH_LIVE_POLL).await;
    }
}

/// The roaming topology up to where the scenarios diverge: R2, R1 and D run
/// with D's one announce learnt by R1 at two hops over `r1_to_r2`; rnsd runs
/// in the caller's slot with A online, C attached, and the hops-1 row for D
/// filed under A's str after C's first request, which was forwarded and not
/// answered; B's listener is up on R1 and B online while R1's own row still
/// names R2. The callers take A down themselves. `config_dir` is rnsd's and
/// belongs to the caller, so a respawn can reuse it with `instance`,
/// `control_port` and `interfaces`.
#[cfg(unix)]
struct RoamingTopology {
    r2: Transport,
    r1: Transport,
    d: Transport,
    c: Transport,
    a_server: AddressHash,
    a_name: String,
    b_name: String,
    hash: AddressHash,
    hex: String,
    control_port: u16,
    endpoint: RpcEndpoint,
    key: RpcKey,
    instance: LiveSharedInstance,
    interfaces: String,
    c_hops_via_a: u8,
    row_via_a: PathRow,
}

/// `prefix` names the four transports and the destination aspect.
#[cfg(unix)]
#[deny(clippy::await_holding_invalid_type)]
async fn roaming_topology(
    prefix: &str,
    config_dir: &std::path::Path,
    c_options: SharedInstanceOptions,
    rnsd_slot: &mut Option<SharedInstanceRpcRnsd>,
) -> RoamingTopology {
    // D announces once over R2; R1 learns it at two hops.
    let r2 = roaming_relay(&format!("{prefix}-r2"));
    let r2_port = free_tcp_port();
    let (_, r2_listener) = roaming_listen(&r2, r2_port).await;
    let r1 = roaming_relay(&format!("{prefix}-r1"));
    let r1_to_r2 = roaming_dial(&r1, r2_port, &r2_listener, 1).await;
    let d_identity = to_transport_private_identity(&PrivateIdentity::new_from_rand(OsRng));
    let d = Transport::new(TransportConfig::new(format!("{prefix}-d"), &d_identity, false));
    let a_port = free_tcp_port();
    let (a_server, _) = roaming_listen(&d, a_port).await;
    roaming_dial(&d, r2_port, &r2_listener, 2).await;
    let destination =
        d.add_destination(d_identity.clone(), DestinationName::new("test", prefix)).await;
    let hash = { destination.lock().await.desc.address_hash };
    let hex = format!("<{}>", hex::encode(hash.as_slice()));
    let packet = destination.lock().await.announce(OsRng, None).expect("valid announce packet");
    d.send_packet(packet).await;
    let r1_path =
        roaming_wait_path(&r1, &hash, true, ROAMING_BRING_UP, "R1 learns D over R2").await;
    assert_eq!(r1_path.interface, Some(r1_to_r2), "{r1_path:?}");
    assert_eq!(r1_path.hops, Some(2), "{r1_path:?}");

    // rnsd comes up with A dialling D and B dialling a port nobody listens
    // on yet; C attaches and asks for the path, which rnsd forwards over A.
    let control_port = free_tcp_port();
    let b_port = free_tcp_port();
    let interfaces = format!(
        "[interfaces]\n  [[A]]\n    type = TCPClientInterface\n    enabled = yes\n    target_host = 127.0.0.1\n    target_port = {a_port}\n  [[B]]\n    type = TCPClientInterface\n    enabled = yes\n    target_host = 127.0.0.1\n    target_port = {b_port}\n"
    );
    let instance = live_tcp_shared_instance().with_transport(true);
    let rnsd = rnsd_slot.insert(
        SharedInstanceRpcRnsd::spawn_with_interfaces(
            &instance,
            control_port,
            config_dir,
            &interfaces,
        )
        .await,
    );
    let endpoint = RpcEndpoint::Tcp(SocketAddr::from(([127, 0, 0, 1], control_port)));
    let key = rnsd.identity_key().await;
    rnsd.poll_interface_stats(&endpoint, &key).await.expect("the rpc listener is up");
    let a_name = format!("TCPInterface[A/127.0.0.1:{a_port}]");
    let b_name = format!("TCPInterface[B/127.0.0.1:{b_port}]");
    let (a_entry, _) = roaming_wait_online(&endpoint, &key, "A", true, ROAMING_BRING_UP).await;
    assert_eq!(a_entry.name, a_name, "{a_entry:?}");
    let c_identity = to_transport_private_identity(&PrivateIdentity::new_from_rand(OsRng));
    let c = Transport::new(TransportConfig::new(format!("{prefix}-c"), &c_identity, false));
    c.attach_shared_instance(instance.endpoint.clone(), c_options).await.expect("C attaches");
    roaming_wait_client_attached(&endpoint, &key).await;
    assert!(!c.has_path(&hash).await, "C attached after D's only announce holds no row");
    let mut announces = c.recv_announces().await;
    let log_before = rnsd.log.snapshot().len();
    let dispatch = c.request_path(&hash, None, None).await;
    assert!(
        dispatch.sent_ifaces + dispatch.queued_ifaces >= 1,
        "the request left C over the shared instance: {dispatch:?}"
    );
    let (c_hops_via_a, _, path_response) =
        path_recipe_wait_announce(&mut announces, hash, ROAMING_BRING_UP).await;
    eprintln!("C's first answer: hops={c_hops_via_a} path_response={path_response}");
    assert_eq!(c_hops_via_a, 1, "D direct to rnsd; C takes the shared-instance hop back");
    let rows = path_live_wait_listed(&endpoint, &key, &hash, true, PATH_LIVE_LISTED).await;
    let row_via_a = path_live_row(&rows, &hash).expect("listed");
    eprintln!("rnsd row via A: {row_via_a:?}");
    assert_eq!(row_via_a.hops, 1, "{row_via_a:?}");
    assert_eq!(row_via_a.interface, a_name, "{row_via_a:?}");
    assert_eq!(
        next_hop_interface_name(&endpoint, &key, &hash, PATH_LIVE_CALL).await.expect("next hop"),
        Some(a_name.clone())
    );
    // rnsd held no row, so the first request was fanned out, not answered.
    let since_request = rnsd.log.snapshot();
    let since_request = &since_request[log_before.min(since_request.len())..];
    assert!(
        !path_recipe_lines_for(since_request, PATH_RECIPE_FORWARDED_LINE, &hex).is_empty(),
        "the first request is forwarded upstream"
    );
    assert!(path_recipe_lines_for(since_request, PATH_RECIPE_KNOWN_LINE, &hex).is_empty());
    rnsd.assert_running("after the path via A");

    // B's listener appears; rnsd's reconnect loop brings B online. R1's
    // own row is untouched.
    roaming_listen(&r1, b_port).await;
    let (b_entry, b_online_after) =
        roaming_wait_online(&endpoint, &key, "B", true, ROAMING_B_ONLINE).await;
    assert_eq!(b_entry.name, b_name, "{b_entry:?}");
    eprintln!("B online {b_online_after:?} after its listener appeared");
    let r1_path = r1.path_status(&hash).await;
    assert_eq!(r1_path.interface, Some(r1_to_r2), "{r1_path:?}");
    assert_eq!(r1_path.hops, Some(2), "{r1_path:?}");

    RoamingTopology {
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
        c_hops_via_a,
        row_via_a,
    }
}

/// The whole-test budget as a panic that still shows rnsd's output: the
/// scenario's rnsd lives in `rnsd_slot` on the test's frame, so dropping the
/// timed-out future does not take the log tail with it (that tail prints
/// only when its own drop runs during a panic).
#[cfg(unix)]
fn roaming_expect_in_budget<T>(
    outcome: Result<T, tokio::time::error::Elapsed>,
    rnsd: &Option<SharedInstanceRpcRnsd>,
    what: &str,
) -> T {
    outcome.unwrap_or_else(|_| {
        if let Some(rnsd) = rnsd {
            eprintln!("rnsd output at the budget:\n{}", rnsd.log.snapshot());
        }
        panic!("{what} did not finish within its budget")
    })
}

#[cfg(unix)]
#[deny(clippy::await_holding_invalid_type)]
async fn roaming_scenario(
    started: tokio::time::Instant,
    config_dir: &std::path::Path,
    rnsd_slot: &mut Option<SharedInstanceRpcRnsd>,
) {
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
        endpoint,
        key,
        c_hops_via_a,
        row_via_a,
        ..
    } = roaming_topology("roaming", config_dir, SharedInstanceOptions::default(), rnsd_slot).await;
    let rnsd = rnsd_slot.as_mut().expect("the fixture spawned rnsd");

    // A goes down: the stats entry reads offline, the row does not move.
    let (_, offline_after) =
        roaming_take_a_down(&d, a_server, &endpoint, &key, &hash, &a_name).await;
    assert_eq!(
        next_hop_interface_name(&endpoint, &key, &hash, PATH_LIVE_CALL).await.expect("next hop"),
        Some(a_name.clone())
    );

    // A link over the dead row times out; C expires its own row on the
    // establishment timeout and queues no rediscovery while attached to a
    // shared instance, so one explicit request follows and rnsd answers it
    // from the stale entry.
    let desc = c.destination_desc(&hash).await.expect("C holds D's description");
    let mut link_events = c.out_link_events();
    let link_started = tokio::time::Instant::now();
    let link = c.link(desc).await.expect("Ok: a path exists");
    let reason = wait_for_out_link_closed(&mut link_events, &link, ROAMING_LINK).await;
    let link_timeout_after = link_started.elapsed();
    eprintln!("link over the dead row closed {link_timeout_after:?} after the request: {reason:?}");
    assert_eq!(reason, Some(LinkCloseReason::Timeout));
    roaming_wait_path(&c, &hash, false, ROAMING_REPATH, "C drops the row of the timed-out link")
        .await;
    let mut announces = c.recv_announces().await;
    let log_before = rnsd.log.snapshot().len();
    let dispatch = c.request_path(&hash, None, None).await;
    assert!(dispatch.sent_ifaces + dispatch.queued_ifaces >= 1, "{dispatch:?}");
    let (c_hops_stale, _, path_response) =
        path_recipe_wait_announce(&mut announces, hash, ROAMING_REPATH).await;
    eprintln!("C's answer over the dead row: hops={c_hops_stale} path_response={path_response}");
    assert_eq!(c_hops_stale, 1, "the stale entry is handed back");
    assert!(c.has_path(&hash).await);
    assert_eq!(c.hops_to(&hash).await, 1);
    let known = rnsd
        .log
        .wait_for("the known-path answer", ROAMING_REPATH, |line| {
            (line.contains(PATH_RECIPE_KNOWN_LINE) && line.contains(&hex)).then(|| line.to_string())
        })
        .await;
    eprintln!("rnsd: {known}");
    let since_link = rnsd.log.snapshot();
    let since_link = &since_link[log_before.min(since_link.len())..];
    let forwarded = path_recipe_lines_for(since_link, PATH_RECIPE_FORWARDED_LINE, &hex);
    assert!(
        forwarded.is_empty(),
        "with the stale row on file the request is answered, never forwarded: {forwarded:?}"
    );

    // The recipe: settle the drop, one request, the row moves to B.
    let b_entry = roaming_stats_entry(&endpoint, &key, "B").await;
    assert!(b_entry.online, "{b_entry:?}");
    let recovery_started = tokio::time::Instant::now();
    let settled =
        drop_path_settled(&endpoint, &key, &hash, PATH_LIVE_CALL).await.expect("drop_path_settled");
    assert!(settled, "the stale row settles");
    assert!(c.expire_path(&hash).await, "C forgets the stale answer");
    let mut announces = c.recv_announces().await;
    let dispatch = c.request_path(&hash, None, None).await;
    assert!(dispatch.sent_ifaces + dispatch.queued_ifaces >= 1, "{dispatch:?}");
    let row_via_b = roaming_wait_row(&endpoint, &key, &hash, 3, &b_name, ROAMING_BRING_UP).await;
    eprintln!("rnsd row via B: {row_via_b:?}");
    assert_eq!(row_via_b.interface, b_entry.name, "{row_via_b:?} vs {b_entry:?}");
    let (c_hops_via_b, _, path_response) =
        path_recipe_wait_announce(&mut announces, hash, ROAMING_BRING_UP).await;
    eprintln!("C's answer over B: hops={c_hops_via_b} path_response={path_response}");
    assert_eq!(c_hops_via_b, 3, "D, R2, R1, rnsd; C takes the shared-instance hop back");
    assert_eq!(c.hops_to(&hash).await, 3);
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
        "OBSERVED: A={a_name} B={b_name} rnsd_hops via_A={} via_B={} C_hops first={c_hops_via_a} \
         stale={c_hops_stale} via_B={c_hops_via_b} A_offline_detected={offline_after:?} \
         link_timeout={link_timeout_after:?} recovery={recovery_after:?} total={:?}",
        row_via_a.hops,
        row_via_b.hops,
        started.elapsed()
    );
}

#[cfg(unix)]
#[tokio::test]
#[ignore = "requires local Python Reticulum checkout"]
async fn python_rnsd_roaming_keeps_the_dead_one_hop_row_until_drop_settled_moves_it_to_the_live_route(
) {
    let _interop_guard = python_interop_guard().await;
    let started = tokio::time::Instant::now();
    let temp = tempfile::tempdir().expect("tempdir");
    let config_dir = temp.path().join("python-rnsd-roaming");
    let mut rnsd = None;
    let outcome =
        tokio::time::timeout(ROAMING_BUDGET, roaming_scenario(started, &config_dir, &mut rnsd))
            .await;
    roaming_expect_in_budget(outcome, &rnsd, "the roaming scenario");
}
