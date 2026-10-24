use super::*;

use crate::bootstrap::select_tcp_server_bind;
use rns_transport::identity_bridge::to_transport_private_identity;
use rns_transport::transport::TransportConfig;

fn attach_test_args() -> Args {
    Args {
        rpc: None,
        db: std::path::PathBuf::from("reticulum.db"),
        config: None,
        identity: None,
        announce_interval_secs: 0,
        transport: None,
        strict_interface_startup: false,
        rpc_tls_cert: None,
        rpc_tls_key: None,
        rpc_tls_client_ca: None,
        rpc_token_issuer: None,
        rpc_token_audience: None,
        rpc_token_secret_env: None,
        rpc_token_jti_ttl_ms: 60_000,
        rpc_token_clock_skew_ms: 5_000,
        rpc_unix: None,
        #[cfg(feature = "zmq-pipeline-rpc")]
        zmq_rpc_command: None,
        zmq_durable_broker: false,
        zmq_broker_budget_bytes: 512 * 1024 * 1024,
        zmq_broker_restored: false,
        #[cfg(feature = "zmq-pipeline-rpc")]
        zmq_rpc_endpoint: None,
        meshchat_bind: None,
        meshchat_assets: None,
    }
}

fn test_transport() -> Transport {
    let identity = rns_core::identity::PrivateIdentity::new_from_rand(rand_core::OsRng);
    let transport_identity = to_transport_private_identity(&identity);
    Transport::new(TransportConfig::new("test", &transport_identity, true))
}

fn records_for(config: &DaemonConfig) -> Vec<InterfaceRecord> {
    config
        .interfaces
        .iter()
        .map(|iface| InterfaceRecord {
            kind: iface.kind.clone(),
            enabled: iface.enabled(),
            host: iface.host.clone(),
            port: iface.port,
            name: iface.name.clone(),
            settings: iface.settings_json(),
        })
        .collect()
}

async fn run_startup(
    args: &Args,
    config: &DaemonConfig,
    transport: &Transport,
    records: &mut [InterfaceRecord],
) -> InterfaceStartupBatch {
    let selection = select_tcp_server_bind(args, Some(config)).expect("tcp server selection");
    let manager = transport.iface_manager();
    startup_configured_interfaces(
        args,
        config,
        &selection,
        transport,
        &manager,
        None,
        records,
        std::path::Path::new("."),
        None,
        None,
    )
    .await
}

fn runtime_field<'a>(record: &'a InterfaceRecord, key: &str) -> Option<&'a str> {
    record
        .settings
        .as_ref()
        .and_then(|settings| settings.get("_runtime"))
        .and_then(|runtime| runtime.get(key))
        .and_then(serde_json::Value::as_str)
}

async fn assert_slot_holds_attached_record(transport: &Transport, record: &InterfaceRecord) {
    assert_eq!(runtime_field(record, "startup_status"), Some("attached"));
    let attached = transport.shared_instance().expect("shared instance slot filled by attach");
    assert_eq!(runtime_field(record, "iface"), Some(attached.to_string().as_str()));
    assert!(transport.connected_to_shared_instance().await);
}

#[tokio::test]
async fn local_tcp_attach_fills_transport_shared_instance_slot() {
    let shared_listener =
        std::net::TcpListener::bind("127.0.0.1:0").expect("bind shared local instance");
    let port = shared_listener.local_addr().expect("shared local addr").port();
    let config = DaemonConfig::from_toml(&format!(
        r#"
interfaces = [
  {{ type = "LocalInterface", enabled = true, name = "local-attach", shared_instance_type = "tcp", shared_instance_port = {port} }}
]
"#
    ))
    .expect("local tcp config");
    let args = attach_test_args();
    let transport = test_transport();
    let mut records = records_for(&config);

    let batch = run_startup(&args, &config, &transport, &mut records).await;

    assert_eq!(batch.startup_successes, 1);
    assert!(batch.startup_failures.is_empty(), "failures={:?}", batch.startup_failures);
    assert!(batch.tunnel_synth_ifaces.is_empty());
    assert_slot_holds_attached_record(&transport, &records[0]).await;
}

#[tokio::test]
async fn local_client_tcp_attach_fills_transport_shared_instance_slot() {
    let shared_listener =
        std::net::TcpListener::bind("127.0.0.1:0").expect("bind shared local instance");
    let port = shared_listener.local_addr().expect("shared local addr").port();
    let config = DaemonConfig::from_toml(&format!(
        r#"
interfaces = [
  {{ type = "LocalClientInterface", enabled = true, name = "local-client", shared_instance_type = "tcp", shared_instance_port = {port} }}
]
"#
    ))
    .expect("local_client tcp config");
    let args = attach_test_args();
    let transport = test_transport();
    let mut records = records_for(&config);

    let batch = run_startup(&args, &config, &transport, &mut records).await;

    assert_eq!(batch.startup_successes, 1);
    assert!(batch.startup_failures.is_empty(), "failures={:?}", batch.startup_failures);
    assert_slot_holds_attached_record(&transport, &records[0]).await;
}

/// Taking the resolver's first answer would yield `[::1]:1` for the first case, so the pick
/// has to look past it to the IPv4 loopback the shared instance listens on.
#[test]
fn local_client_localhost_host_resolves_to_the_ipv4_loopback_address() {
    let v6_loopback: SocketAddr = "[::1]:1".parse().expect("v6 loopback");
    let v4_loopback: SocketAddr = "127.0.0.1:1".parse().expect("v4 loopback");
    let v4_remote: SocketAddr = "10.0.0.1:1".parse().expect("v4 remote");

    assert_eq!(
        pick_shared_instance_tcp_addr(&[v6_loopback, v4_loopback]),
        Some(v4_loopback),
        "the IPv4 loopback wins over an earlier IPv6 loopback"
    );
    assert_eq!(
        pick_shared_instance_tcp_addr(&[v6_loopback, v4_remote]),
        Some(v4_remote),
        "any IPv4 address wins over an earlier IPv6 one"
    );
    assert_eq!(
        pick_shared_instance_tcp_addr(&[v6_loopback]),
        Some(v6_loopback),
        "an IPv6-only answer is used as is"
    );
    assert_eq!(pick_shared_instance_tcp_addr(&[]), None);
}

#[tokio::test]
async fn local_client_localhost_host_attaches_to_the_ipv4_shared_instance() {
    if let Err(err) = tokio::net::lookup_host("localhost:1").await {
        eprintln!("skipping: localhost does not resolve on this host: {err}");
        return;
    }
    let shared_listener =
        std::net::TcpListener::bind("127.0.0.1:0").expect("bind shared local instance");
    let port = shared_listener.local_addr().expect("shared local addr").port();
    let config = DaemonConfig::from_toml(&format!(
        r#"
interfaces = [
  {{ type = "LocalClientInterface", enabled = true, name = "localhost-client", shared_instance_type = "tcp", host = "localhost", shared_instance_port = {port} }}
]
"#
    ))
    .expect("localhost local_client config");
    let args = attach_test_args();
    let transport = test_transport();
    let mut records = records_for(&config);

    let batch = run_startup(&args, &config, &transport, &mut records).await;

    assert_eq!(batch.startup_successes, 1);
    assert!(batch.startup_failures.is_empty(), "failures={:?}", batch.startup_failures);
    assert_slot_holds_attached_record(&transport, &records[0]).await;
}

#[cfg(unix)]
#[tokio::test]
async fn local_client_unix_attach_fills_transport_shared_instance_slot() {
    let temp = tempfile::TempDir::new().expect("temp dir");
    let socket_path = temp.path().join("shared.sock");
    let _shared_listener =
        tokio::net::UnixListener::bind(&socket_path).expect("bind shared unix instance");
    let config = DaemonConfig::from_toml(&format!(
        r#"
interfaces = [
  {{ type = "LocalClientInterface", enabled = true, name = "local-unix-client", shared_instance_type = "unix", socket_path = "{}" }}
]
"#,
        socket_path.display()
    ))
    .expect("local_client unix config");
    let args = attach_test_args();
    let transport = test_transport();
    let mut records = records_for(&config);

    let batch = run_startup(&args, &config, &transport, &mut records).await;

    assert_eq!(batch.startup_successes, 1);
    assert!(batch.startup_failures.is_empty(), "failures={:?}", batch.startup_failures);
    assert_slot_holds_attached_record(&transport, &records[0]).await;
}

#[tokio::test]
async fn second_client_posture_entry_is_recorded_as_already_attached() {
    let shared_listener =
        std::net::TcpListener::bind("127.0.0.1:0").expect("bind shared local instance");
    let port = shared_listener.local_addr().expect("shared local addr").port();
    let config = DaemonConfig::from_toml(&format!(
        r#"
interfaces = [
  {{ type = "LocalClientInterface", enabled = true, name = "first-client", shared_instance_type = "tcp", shared_instance_port = {port} }},
  {{ type = "LocalClientInterface", enabled = true, name = "second-client", shared_instance_type = "tcp", shared_instance_port = {port} }}
]
"#
    ))
    .expect("two local_client config");
    let args = attach_test_args();
    let transport = test_transport();
    let mut records = records_for(&config);

    let batch = run_startup(&args, &config, &transport, &mut records).await;

    assert_eq!(batch.startup_successes, 1);
    assert_eq!(batch.startup_failures.len(), 1);
    assert_eq!(batch.startup_failures[0].label, "second-client");
    assert!(
        batch.startup_failures[0].error.contains("already attached"),
        "error={}",
        batch.startup_failures[0].error
    );
    let attached =
        records.iter().filter(|record| runtime_field(record, "startup_status") == Some("attached"));
    assert_eq!(attached.count(), 1);
    assert_slot_holds_attached_record(&transport, &records[0]).await;
    assert_eq!(runtime_field(&records[1], "startup_status"), Some("failed"));
    assert!(runtime_field(&records[1], "startup_error")
        .is_some_and(|error| error.contains("already attached")));
}

/// A client-posture entry naming a remote host never reaches the attach: the daemon's
/// config refuses it first ("host must be loopback for local"), so the library's own
/// `SharedInstanceEndpointNotLoopback` is defence in depth behind that gate and the daemon
/// has no need of `allow_remote_endpoint`.
#[test]
fn non_loopback_local_client_host_is_refused_by_the_config_before_any_attach() {
    for kind in ["LocalClientInterface", "LocalInterface"] {
        let refused = DaemonConfig::from_toml(&format!(
            r#"
interfaces = [
  {{ type = "{kind}", enabled = true, name = "remote-client", shared_instance_type = "tcp", host = "10.255.255.1", shared_instance_port = 1 }}
]
"#
        ));
        let err =
            refused.err().unwrap_or_else(|| panic!("{kind} with a remote host must be refused"));
        assert!(
            err.to_string().contains("host must be loopback for local"),
            "{kind}: the config names the loopback rule: {err}"
        );
    }
}

/// The hosts the config lets through are the ones the library's loopback rule accepts
/// without the opt-in, so a config that parses is a config whose attach is not refused for
/// its address: `::1` attaches like `127.0.0.1`, filling the slot and setting the posture.
#[tokio::test]
async fn local_client_ipv6_loopback_host_attaches_without_the_opt_in() {
    let shared_listener = match std::net::TcpListener::bind("[::1]:0") {
        Ok(listener) => listener,
        Err(err) => {
            eprintln!("skipping: no IPv6 loopback on this host: {err}");
            return;
        }
    };
    let port = shared_listener.local_addr().expect("shared local addr").port();
    let config = DaemonConfig::from_toml(&format!(
        r#"
interfaces = [
  {{ type = "LocalClientInterface", enabled = true, name = "v6-client", shared_instance_type = "tcp", host = "::1", shared_instance_port = {port} }}
]
"#
    ))
    .expect("ipv6 loopback local_client config");
    let args = attach_test_args();
    let transport = test_transport();
    let mut records = records_for(&config);

    let batch = run_startup(&args, &config, &transport, &mut records).await;

    assert_eq!(batch.startup_successes, 1);
    assert!(batch.startup_failures.is_empty(), "failures={:?}", batch.startup_failures);
    assert_slot_holds_attached_record(&transport, &records[0]).await;
}
