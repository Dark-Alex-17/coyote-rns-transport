// The library's shared-instance RPC client against a live Python rnsd: the
// `multiprocessing.connection.Listener` rnsd opens at
// `RNS/Reticulum.py:363-367` answers `interface_stats` for the interfaces
// rnsd runs, here its own `LocalServerInterface` and one `AutoInterface`
// from a top-level `[interfaces]` section (`RNS/Reticulum.py:748-752`).

use rns_transport::transport::shared_instance_rpc::{
    interface_stats, InterfaceStat, InterfaceStats, RpcEndpoint, RpcError, RpcKey,
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
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock after the epoch")
        .as_nanos();
    format!(
        "[interfaces]\n  [[{SHARED_INSTANCE_RPC_AUTO_INTERFACE_NAME}]]\n    type = AutoInterface\n    enabled = yes\n    group_id = coyote-live-{}-{nanos}\n    data_port = {}\n",
        std::process::id(),
        shared_instance_rpc_free_udp_port()
    )
}

// Both are held for their Drop: the guard is declared before the tail so it
// is dropped first and the kill gives the drain threads their EOF, as with
// the siblings' locals; the tail prints rnsd's output when the test panics.
struct SharedInstanceRpcRnsd {
    _guard: ChildGuard,
    _log: ProcessLogTail,
    identity_file: std::path::PathBuf,
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
        Self {
            _guard: guard,
            _log: log,
            identity_file: config_dir.join("storage/transport_identity"),
        }
    }

    /// Python's default `authkey`, the SHA-256 of the transport identity
    /// `Transport.start` writes to `<configdir>/storage/transport_identity`
    /// (`RNS/Transport.py:320-327`, `RNS/Reticulum.py:360-361`). The file is
    /// opened and then written (`RNS/Identity.py:665-666`), so it is read
    /// only once it holds the 64 bytes.
    #[deny(clippy::await_holding_invalid_type)]
    async fn identity_key(&self) -> RpcKey {
        let deadline = tokio::time::Instant::now() + SHARED_INSTANCE_RPC_IDENTITY_FILE;
        while fs::metadata(&self.identity_file).map(|meta| meta.len()).unwrap_or(0) != 64 {
            assert!(
                tokio::time::Instant::now() < deadline,
                "{} never held the 64-byte identity",
                self.identity_file.display()
            );
            tokio::time::sleep(SHARED_INSTANCE_RPC_IDENTITY_POLL).await;
        }
        RpcKey::from_transport_identity_file(&self.identity_file)
            .expect("rpc key from the transport identity")
    }

    /// rnsd binds the RPC listener only after `Transport.start`
    /// (`RNS/Reticulum.py:351-367`), which runs after the shared instance
    /// log line and the AutoInterface's peering wait in `final_init`
    /// (`RNS/Reticulum.py:1009`, `RNS/Interfaces/AutoInterface.py:327-348`),
    /// so early dials are refused. The listener's backlog is one, so a dial
    /// can also be reset mid-handshake; both are retried, anything else is
    /// returned as the first reply.
    #[deny(clippy::await_holding_invalid_type)]
    async fn poll_interface_stats(
        &self,
        endpoint: &RpcEndpoint,
        key: &RpcKey,
    ) -> Result<InterfaceStats, RpcError> {
        let deadline = tokio::time::Instant::now() + SHARED_INSTANCE_RPC_LISTENER;
        loop {
            let result = interface_stats(endpoint, key, SHARED_INSTANCE_RPC_CALL).await;
            let retry = match &result {
                Err(RpcError::Connect(_)) => true,
                Err(RpcError::Protocol(reason)) => reason.to_ascii_lowercase().contains("reset"),
                _ => false,
            };
            if !retry {
                return result;
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "the rpc listener never answered within {SHARED_INSTANCE_RPC_LISTENER:?}: {result:?}"
            );
            tokio::time::sleep(SHARED_INSTANCE_RPC_LISTENER_RETRY).await;
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
/// `LocalServerInterface` has no `peers` attribute, so that key is nil
/// (`RNS/Reticulum.py:1501-1503`), and it starts with no clients
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
}

#[deny(clippy::await_holding_invalid_type)]
async fn run_python_rnsd_shared_instance_rpc_reports_and_refuses(
    instance: LiveSharedInstance,
    endpoint_for: impl FnOnce(&LiveSharedInstance, u16) -> RpcEndpoint,
    config_dir_name: &str,
) {
    let temp = tempfile::tempdir().expect("tempdir");
    let control_port = free_tcp_port();
    let rnsd = SharedInstanceRpcRnsd::spawn(
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
/// `LocalServerInterface` entry with a nil `peers` key
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
    let rnsd = SharedInstanceRpcRnsd::spawn(
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
