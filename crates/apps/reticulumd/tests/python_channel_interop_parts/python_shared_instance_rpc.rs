// The library's shared-instance RPC client against a live Python rnsd: the
// `multiprocessing.connection.Listener` rnsd opens at
// `RNS/Reticulum.py:363-367` answers `interface_stats` for the interfaces
// rnsd runs, here its own `LocalServerInterface` and one `AutoInterface`
// from a top-level `[interfaces]` section (`RNS/Reticulum.py:748-752`).

use rns_transport::transport::shared_instance_rpc::{
    drop_all_via, drop_path, drop_path_settled, first_hop_timeout, interface_stats,
    next_hop_interface_name, path_table, InterfaceStat, InterfaceStats, RpcEndpoint, RpcError,
    RpcKey,
};

const SHARED_INSTANCE_RPC_RNSD_START: Duration = Duration::from_secs(30);
const SHARED_INSTANCE_RPC_IDENTITY_FILE: Duration = Duration::from_secs(30);
const SHARED_INSTANCE_RPC_IDENTITY_POLL: Duration = Duration::from_millis(100);
const SHARED_INSTANCE_RPC_LISTENER: Duration = Duration::from_secs(15);
const SHARED_INSTANCE_RPC_LISTENER_RETRY: Duration = Duration::from_millis(250);
const SHARED_INSTANCE_RPC_CALL: Duration = Duration::from_secs(5);
const SHARED_INSTANCE_RPC_AUTO_INTERFACE_NAME: &str = "coyote-live-auto";
const SHARED_INSTANCE_RPC_CONFIGURED_KEY: &str =
    "c0ffee00c0ffee00c0ffee00c0ffee00c0ffee00c0ffee00c0ffee00c0ffee00";

// The retry set of the crate README: an io kind behind `Io` or `Connect`
// that a dial landing on rnsd's serial `rpc_loop` (`RNS/Reticulum.py:1274-1334`,
// whose `Listener` at `:364` keeps the stdlib default `backlog=1` of
// `multiprocessing.connection` `Listener.__init__`) or a server departing
// mid-call can produce.
fn shared_instance_rpc_retryable(err: &RpcError) -> bool {
    use std::io::ErrorKind::{BrokenPipe, ConnectionAborted, ConnectionReset, UnexpectedEof};
    matches!(err.io_kind(), Some(ConnectionReset | ConnectionAborted | BrokenPipe | UnexpectedEof))
}

// The kind-only predicate above is `RpcError::is_transient()` without its `Timeout` arm:
// the two agree on every variant but `Timeout`, which `poll_interface_stats` returns as
// the reply instead of dialling again.
#[test]
fn shared_instance_rpc_retryable_is_is_transient_without_the_timeout_arm() {
    use std::io::{Error, ErrorKind};
    for kind in [
        ErrorKind::ConnectionReset,
        ErrorKind::ConnectionAborted,
        ErrorKind::BrokenPipe,
        ErrorKind::UnexpectedEof,
        ErrorKind::ConnectionRefused,
        ErrorKind::Other,
    ] {
        for err in [RpcError::Io(Error::from(kind)), RpcError::Connect(Error::from(kind))] {
            assert_eq!(shared_instance_rpc_retryable(&err), err.is_transient(), "{err:?}");
        }
    }
    for err in [
        RpcError::Authentication,
        RpcError::Protocol("short".into()),
        RpcError::Decode("shape".into()),
        RpcError::UnsupportedDigest("sha1".into()),
        RpcError::InvalidKeyMaterial,
    ] {
        assert_eq!(shared_instance_rpc_retryable(&err), err.is_transient(), "{err:?}");
    }
    assert!(RpcError::Timeout.is_transient());
    assert!(!shared_instance_rpc_retryable(&RpcError::Timeout));
}

// The AutoInterface's data socket is bound without address reuse
// (`RNS/Interfaces/AutoInterface.py:334-337`) and the bind error reaches the
// catch-all that panics rnsd (`RNS/Reticulum.py:1120-1124`), so a host rnsd
// on the default data port would take the whole instance down; the
// discovery sockets set `SO_REUSEPORT` and keep their default. A port free
// on the IPv6 wildcard is free on every link-local address the interface
// adopts.
fn shared_instance_rpc_free_udp_port() -> u16 {
    std::net::UdpSocket::bind("[::]:0")
        .expect("bind ephemeral udp port")
        .local_addr()
        .expect("local addr")
        .port()
}

// The group id is unique per run so the host's own rnsd, or a leftover one,
// cannot peer with this AutoInterface and raise its peer count.
fn shared_instance_rpc_auto_interface_section() -> String {
    format!(
        "[interfaces]\n  [[{SHARED_INSTANCE_RPC_AUTO_INTERFACE_NAME}]]\n    type = AutoInterface\n    enabled = yes\n    group_id = {}\n    data_port = {}\n",
        live_run_tag(),
        shared_instance_rpc_free_udp_port()
    )
}

// Both are held for their Drop: the guard is declared before the tail so it
// is dropped first and the kill gives the drain threads their EOF, as with
// the siblings' locals; the tail prints rnsd's output when the test panics.
struct SharedInstanceRpcRnsd {
    guard: ChildGuard,
    _log: ProcessLogTail,
    identity_file: std::path::PathBuf,
}

fn shared_instance_rpc_rnsd_exited(
    rnsd: &mut std::process::Child,
) -> Option<std::process::ExitStatus> {
    rnsd.try_wait().expect("poll the rnsd child")
}

impl SharedInstanceRpcRnsd {
    #[deny(clippy::await_holding_invalid_type)]
    async fn spawn(
        instance: &LiveSharedInstance,
        control_port: u16,
        config_dir: &std::path::Path,
    ) -> Self {
        let paths = python_channel_interop_paths();
        write_live_shared_instance_config(
            config_dir,
            &instance.config_with_interfaces(
                control_port,
                &shared_instance_rpc_auto_interface_section(),
            ),
        );
        let mut rnsd = paths.spawn_rnsd(config_dir);
        let log = ProcessLogTail::follow("rnsd", &mut rnsd);
        let guard = ChildGuard { child: Some(rnsd) };
        log.wait_for("the shared instance interface", SHARED_INSTANCE_RPC_RNSD_START, |line| {
            line.contains("Started shared instance interface").then_some(())
        })
        .await;
        Self { guard, _log: log, identity_file: config_dir.join("storage/transport_identity") }
    }

    /// Python's default `authkey`, the SHA-256 of the transport identity
    /// `Transport.start` writes to `<configdir>/storage/transport_identity`
    /// (`RNS/Transport.py:320-327`, `RNS/Reticulum.py:360-361`). The file is
    /// opened and then written (`RNS/Identity.py:665-666`), so it is read
    /// only once it holds the 64 bytes. An rnsd that has exited fails the
    /// poll at once with its exit status instead of being watched to the
    /// deadline.
    #[deny(clippy::await_holding_invalid_type)]
    async fn identity_key(&mut self) -> RpcKey {
        let rnsd = self.guard.child.as_mut().expect("rnsd is held until drop");
        let identity_file = &self.identity_file;
        let poll = async {
            while fs::metadata(identity_file).map(|meta| meta.len()).unwrap_or(0) != 64 {
                if let Some(status) = shared_instance_rpc_rnsd_exited(rnsd) {
                    panic!("rnsd exited with {status} before writing {}", identity_file.display());
                }
                tokio::time::sleep(SHARED_INSTANCE_RPC_IDENTITY_POLL).await;
            }
        };
        assert!(
            tokio::time::timeout(SHARED_INSTANCE_RPC_IDENTITY_FILE, poll).await.is_ok(),
            "{} never held the 64-byte identity",
            identity_file.display()
        );
        RpcKey::from_transport_identity_file(&self.identity_file)
            .expect("rpc key from the transport identity")
    }

    /// Fails at once, naming rnsd's exit status and `context`, when the
    /// fixture has exited; a dead rnsd otherwise reads as a refused dial.
    #[cfg(unix)]
    fn assert_running(&mut self, context: &str) {
        let rnsd = self.guard.child.as_mut().expect("rnsd is held until drop");
        if let Some(status) = shared_instance_rpc_rnsd_exited(rnsd) {
            panic!("rnsd exited with {status}: {context}");
        }
    }

    /// rnsd binds the RPC listener only after `Transport.start`
    /// (`RNS/Reticulum.py:351-367`), which runs after the shared instance
    /// log line and the AutoInterface's peering wait in `final_init`
    /// (`RNS/Reticulum.py:1009`, `RNS/Interfaces/AutoInterface.py:327-348`),
    /// so early dials are refused. The listener's backlog is one and rnsd
    /// serves one dial at a time on `rpc_loop` (`RNS/Reticulum.py:1274-1334`),
    /// so a dial that lands while another is served can be queued, dropped
    /// or reset; an io error of a retryable kind (`ConnectionReset`,
    /// `ConnectionAborted`, `BrokenPipe`, `UnexpectedEof`, read via
    /// `io_kind()`) is retried like a refused connect until
    /// `SHARED_INSTANCE_RPC_LISTENER` runs out, anything else is returned as
    /// the first reply. An rnsd that has exited fails the poll at once with
    /// its exit status instead of being dialled to the deadline.
    #[deny(clippy::await_holding_invalid_type)]
    async fn poll_interface_stats(
        &mut self,
        endpoint: &RpcEndpoint,
        key: &RpcKey,
    ) -> Result<InterfaceStats, RpcError> {
        let rnsd = self.guard.child.as_mut().expect("rnsd is held until drop");
        let mut last_retried = None;
        let poll = async {
            loop {
                let result = interface_stats(endpoint, key, SHARED_INSTANCE_RPC_CALL).await;
                let retry = match &result {
                    Err(RpcError::Connect(_)) => true,
                    Err(err @ RpcError::Io(_)) => shared_instance_rpc_retryable(err),
                    _ => false,
                };
                if !retry {
                    return result;
                }
                if let Some(status) = shared_instance_rpc_rnsd_exited(rnsd) {
                    panic!(
                        "rnsd exited with {status} before the rpc listener answered: {result:?}"
                    );
                }
                last_retried = Some(result);
                tokio::time::sleep(SHARED_INSTANCE_RPC_LISTENER_RETRY).await;
            }
        };
        let outcome = tokio::time::timeout(SHARED_INSTANCE_RPC_LISTENER, poll).await;
        match outcome {
            Ok(result) => result,
            Err(_) => panic!(
                "the rpc listener never answered within {SHARED_INSTANCE_RPC_LISTENER:?}: {last_retried:?}"
            ),
        }
    }
}

fn shared_instance_rpc_entry<'a>(stats: &'a InterfaceStats, kind: &str) -> &'a InterfaceStat {
    stats
        .interfaces
        .iter()
        .find(|iface| iface.kind == kind)
        .unwrap_or_else(|| panic!("no {kind} entry in {:?}", stats.interfaces))
}

/// The AutoInterface is added and marked online whether or not a link-local
/// address was found (`RNS/Reticulum.py:1008`,
/// `RNS/Interfaces/AutoInterface.py:350`), and nobody shares its group id,
/// so its entry is present with zero peers. The shared instance's own
/// `LocalServerInterface` has no `peers` attribute, so the key is left out
/// of its entry (`RNS/Reticulum.py:1501-1503`), and it starts with no clients
/// (`RNS/Interfaces/LocalInterface.py:363`, `:371`, `:486-487`).
fn assert_shared_instance_rpc_entries(stats: &InterfaceStats) {
    for iface in &stats.interfaces {
        eprintln!(
            "rnsd interface kind={} online={} peers={:?}",
            iface.kind, iface.online, iface.peers
        );
    }
    let auto = shared_instance_rpc_entry(stats, "AutoInterface");
    assert_eq!(auto.short_name, SHARED_INSTANCE_RPC_AUTO_INTERFACE_NAME, "{auto:?}");
    assert!(auto.online, "{auto:?}");
    assert_eq!(auto.peers, Some(0), "{auto:?}");

    let local = shared_instance_rpc_entry(stats, "LocalServerInterface");
    assert!(local.online, "{local:?}");
    assert_eq!(local.peers, None, "{local:?}");
    assert_eq!(local.clients, Some(0), "{local:?}");
    assert_eq!(local.short_name, "Reticulum", "{local:?}");
    assert!(local.name.starts_with("Shared Instance["), "{local:?}");

    let top = stats.raw.as_map().expect("the reply is a map");
    assert!(
        top.iter().any(|(key, _)| key.as_str() == Some("interfaces")),
        "no interfaces key in {:?}",
        stats.raw
    );
    // The totals `get_interface_stats` sums over the interfaces
    // (`RNS/Reticulum.py:1405-1628`) are integers, and each typed entry's
    // byte counters are read from its own map.
    assert!(stats.raw["rxb"].is_u64(), "no integer rxb total in {:?}", stats.raw);
    assert!(stats.raw["txb"].is_u64(), "no integer txb total in {:?}", stats.raw);
    for iface in &stats.interfaces {
        assert_eq!(iface.raw["rxb"].as_u64(), Some(iface.rxb), "{iface:?}");
        assert_eq!(iface.raw["txb"].as_u64(), Some(iface.txb), "{iface:?}");
    }
}

#[deny(clippy::await_holding_invalid_type)]
async fn run_python_rnsd_shared_instance_rpc_reports_and_refuses(
    instance: LiveSharedInstance,
    endpoint_for: impl FnOnce(&LiveSharedInstance, u16) -> RpcEndpoint,
    config_dir_name: &str,
) {
    let temp = tempfile::tempdir().expect("tempdir");
    let control_port = free_tcp_port();
    let mut rnsd = SharedInstanceRpcRnsd::spawn(
        &instance,
        control_port,
        &temp.path().join(format!("{config_dir_name}-rnsd")),
    )
    .await;
    let endpoint = endpoint_for(&instance, control_port);
    let key = rnsd.identity_key().await;

    let stats =
        rnsd.poll_interface_stats(&endpoint, &key).await.expect("the first interface_stats reply");
    assert_shared_instance_rpc_entries(&stats);

    let wrong = RpcKey::from_hex(&"00".repeat(32)).expect("hex key");
    let refused = interface_stats(&endpoint, &wrong, SHARED_INSTANCE_RPC_CALL).await;
    assert!(matches!(refused, Err(RpcError::Authentication)), "{refused:?}");

    let again = interface_stats(&endpoint, &key, SHARED_INSTANCE_RPC_CALL)
        .await
        .expect("the server keeps serving after a #FAILURE#");
    assert_shared_instance_rpc_entries(&again);
}

/// A Python rnsd (RNS 1.5.2) is the shared instance on a free TCP port with
/// one `AutoInterface` configured, and this crate reads its interface
/// statistics through `shared_instance_rpc::interface_stats` at the
/// instance's `instance_control_port` (`RNS/Reticulum.py:357`, `:495`).
/// Proven: the default key derived from `storage/transport_identity`
/// (`RNS/Reticulum.py:360-361`) passes the `multiprocessing` handshake, the
/// reply decodes with the `AutoInterface` entry online at zero peers and the
/// `LocalServerInterface` entry with no `peers` key (read as `None`)
/// (`RNS/Reticulum.py:1405-1566`), a wrong key is answered with `#FAILURE#`
/// and surfaces as `RpcError::Authentication`, and the server goes on
/// serving the next connection (`RNS/Reticulum.py:1274-1334`).
#[tokio::test]
#[ignore = "requires local Python Reticulum checkout"]
async fn python_rnsd_shared_instance_rpc_tcp_reports_auto_interface_peers_and_refuses_a_wrong_key()
{
    let _interop_guard = python_interop_guard().await;
    run_python_rnsd_shared_instance_rpc_reports_and_refuses(
        live_tcp_shared_instance(),
        |_, control_port| RpcEndpoint::Tcp(SocketAddr::from(([127, 0, 0, 1], control_port))),
        "python-rnsd-shared-instance-rpc-tcp",
    )
    .await;
}

/// A configured `rpc_key = <hex>` replaces the identity-derived default
/// (`RNS/Reticulum.py:499-505`, `:360-361`): `RpcKey::from_hex` of the same
/// digits authenticates and the key read from `storage/transport_identity`
/// is now refused.
#[tokio::test]
#[ignore = "requires local Python Reticulum checkout"]
async fn python_rnsd_shared_instance_rpc_configured_key_is_honoured() {
    let _interop_guard = python_interop_guard().await;
    let temp = tempfile::tempdir().expect("tempdir");
    let instance = live_tcp_shared_instance()
        .with_reticulum_line(&format!("rpc_key = {SHARED_INSTANCE_RPC_CONFIGURED_KEY}"));
    let control_port = free_tcp_port();
    let mut rnsd = SharedInstanceRpcRnsd::spawn(
        &instance,
        control_port,
        &temp.path().join("python-rnsd-shared-instance-rpc-key-rnsd"),
    )
    .await;
    let endpoint = RpcEndpoint::Tcp(SocketAddr::from(([127, 0, 0, 1], control_port)));
    let identity_key = rnsd.identity_key().await;
    let configured = RpcKey::from_hex(SHARED_INSTANCE_RPC_CONFIGURED_KEY).expect("hex key");

    let stats = rnsd
        .poll_interface_stats(&endpoint, &configured)
        .await
        .expect("the configured key authenticates");
    assert_shared_instance_rpc_entries(&stats);

    let refused = interface_stats(&endpoint, &identity_key, SHARED_INSTANCE_RPC_CALL).await;
    assert!(matches!(refused, Err(RpcError::Authentication)), "{refused:?}");
}

/// The abstract `AF_UNIX` form of
/// [`python_rnsd_shared_instance_rpc_tcp_reports_auto_interface_peers_and_refuses_a_wrong_key`]:
/// with `shared_instance_type = unix` on Linux rnsd binds the RPC listener
/// at `\0rns/<instance_name>/rpc` and the control port goes unused
/// (`RNS/Reticulum.py:353-358`, `:725-727`), which is the endpoint
/// `RpcEndpoint::for_instance` builds there. Linux only because Python's
/// `use_af_unix()` is true on Linux and Android alone
/// (`RNS/vendor/platformutils.py:59-61`).
#[cfg(target_os = "linux")]
#[tokio::test]
#[ignore = "requires local Python Reticulum checkout"]
async fn python_rnsd_shared_instance_rpc_unix_abstract_endpoint_reports_the_same() {
    let _interop_guard = python_interop_guard().await;
    run_python_rnsd_shared_instance_rpc_reports_and_refuses(
        live_unix_shared_instance(),
        |instance, _| {
            RpcEndpoint::for_instance(
                instance.instance_name.as_deref().expect("the unix instance has a name"),
            )
        },
        "python-rnsd-shared-instance-rpc-unix",
    )
    .await;
}
