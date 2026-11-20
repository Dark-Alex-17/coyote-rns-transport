//! The rnsd RPC client used from outside the crate, through
//! `rns_transport::transport::shared_instance_rpc` only: `RpcError` has its eight
//! variants and stays open to more, and is a `Send + Sync + 'static` error;
//! `RpcEndpoint` has `Tcp` and, on unix, `Unix` and stays open to more,
//! `RpcEndpoint::default_tcp()` is `127.0.0.1:37429` and `for_instance("default")` is the
//! abstract unix socket `rns/default/rpc` on Linux and Android and `default_tcp()`
//! elsewhere; `RpcKey::from_hex` accepts an even run of hex digits and refuses an empty,
//! odd or non-hex one, `from_transport_identity_file` reads a 64-byte file, and neither
//! leaks bytes through `Debug`; `InterfaceStat` and `InterfaceStats` are `Clone + Debug +
//! PartialEq` values whose fields are read by name; the `interface_stats` future is
//! `Send`; and a call against a closed loopback port is `Err(RpcError::Connect(_))` well
//! inside its budget. The README recipe's lines run here; the test supplies the endpoint.
//! A server that accepts and hangs up without a byte is `Err(RpcError::Io(_))` with the
//! `UnexpectedEof` kind behind `io_kind()`.

use std::error::Error;
use std::fmt::Debug;
use std::path::Path;
use std::time::{Duration, Instant};

use tokio::io::AsyncWriteExt;

use rns_transport::transport::shared_instance_rpc::{
    interface_stats, InterfaceStat, InterfaceStats, RpcEndpoint, RpcError, RpcKey,
};

#[path = "support/free_loopback_port.rs"]
mod free_loopback_port;
use free_loopback_port::free_loopback_port;

const WAIT: Duration = Duration::from_secs(5);
const KEY_HEX: &str = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";

fn assert_send<T: Send>(_: T) {}

fn assert_error<E: Error + Send + Sync + 'static>() {}

fn assert_value<T: Clone + Debug + PartialEq>() {}

fn error_label(err: &RpcError) -> &'static str {
    match err {
        RpcError::Connect(_) => "connect",
        RpcError::Authentication => "authentication",
        RpcError::Protocol(_) => "protocol",
        RpcError::Io(_) => "io",
        RpcError::Decode(_) => "decode",
        RpcError::UnsupportedDigest(_) => "unsupported digest",
        RpcError::InvalidKeyMaterial => "invalid key material",
        RpcError::Timeout => "timeout",
        _ => "a later variant",
    }
}

fn endpoint_label(endpoint: &RpcEndpoint) -> &'static str {
    match endpoint {
        RpcEndpoint::Tcp(_) => "tcp",
        #[cfg(unix)]
        RpcEndpoint::Unix(_) => "unix",
        _ => "a later variant",
    }
}

/// The README recipe's lines, from the identity file to the per-interface peer read; the
/// test supplies the endpoint.
async fn readme_recipe(configdir: &Path, endpoint: RpcEndpoint) -> Result<(), RpcError> {
    let key = RpcKey::from_transport_identity_file(&configdir.join("storage/transport_identity"))?;
    let stats = interface_stats(&endpoint, &key, Duration::from_secs(5)).await?;
    for iface in stats.interfaces.iter().filter(|iface| iface.kind == "AutoInterface") {
        log::info!("{} peers={:?} online={}", iface.short_name, iface.peers, iface.online);
    }
    describe(&stats);
    Ok(())
}

fn describe(stats: &InterfaceStats) {
    // The `..` is what non_exhaustive demands of a destructure outside the crate.
    let InterfaceStats { interfaces, raw, .. } = stats;
    for stat in interfaces {
        let InterfaceStat { name, short_name, kind, online, peers, clients, rxb, txb, raw, .. } =
            stat;
        log::debug!("{name} {short_name} {kind} {online} {peers:?} {clients:?} {rxb} {txb} {raw}");
    }
    log::debug!("{raw}");
}

#[test]
fn rpc_error_is_a_send_sync_error_with_eight_named_variants() {
    assert_error::<RpcError>();
    let err = RpcError::InvalidKeyMaterial;
    assert_eq!(error_label(&err), "invalid key material");
    assert_eq!(error_label(&RpcError::Timeout), "timeout");
    assert_eq!(error_label(&RpcError::Authentication), "authentication");
    assert_eq!(error_label(&RpcError::Protocol("short".to_string())), "protocol");
    assert_eq!(
        error_label(&RpcError::Io(std::io::Error::from(std::io::ErrorKind::UnexpectedEof))),
        "io"
    );
    assert_eq!(error_label(&RpcError::Decode("shape".to_string())), "decode");
    assert_eq!(error_label(&RpcError::UnsupportedDigest("sha1".to_string())), "unsupported digest");
    assert!(err.to_string().contains("key"), "{err}");
}

#[test]
fn default_tcp_is_the_python_control_port() {
    let endpoint = RpcEndpoint::default_tcp();
    assert_eq!(endpoint, RpcEndpoint::Tcp("127.0.0.1:37429".parse().expect("addr")));
    assert_eq!(endpoint_label(&endpoint), "tcp");
}

#[cfg(any(target_os = "linux", target_os = "android"))]
#[test]
fn for_instance_is_the_abstract_socket_on_linux() {
    use rns_transport::iface::local::LocalUnixEndpoint;
    // The README recipe and its docs pin grep for this exact spelling of
    // `RpcEndpoint::for_instance("default")`; a rename moves with them.
    let endpoint = RpcEndpoint::for_instance("default");
    assert_eq!(endpoint, RpcEndpoint::Unix(LocalUnixEndpoint::abstract_name("rns/default/rpc")));
    assert_eq!(endpoint_label(&endpoint), "unix");
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
#[test]
fn for_instance_is_the_default_tcp_port_elsewhere() {
    // The README recipe and its docs pin grep for this exact spelling of
    // `RpcEndpoint::for_instance("default")`; a rename moves with them.
    let endpoint = RpcEndpoint::for_instance("default");
    assert_eq!(endpoint, RpcEndpoint::default_tcp());
    // "whatever the name": the README promises default_tcp() off Linux and Android for any
    // instance name.
    assert_eq!(RpcEndpoint::for_instance("probe-other-name"), RpcEndpoint::default_tcp());
    assert_eq!(RpcEndpoint::for_instance(""), RpcEndpoint::default_tcp());
}

#[test]
fn hex_key_parses_even_hex_and_redacts_debug() {
    let key = RpcKey::from_hex(KEY_HEX).expect("hex key");
    let debug = format!("{key:?}");
    assert!(debug.contains("redacted"), "{debug}");
    assert!(!debug.contains(KEY_HEX), "{debug}");
    assert!(!debug.contains("0011"), "{debug}");
    let clone = key.clone();
    assert!(format!("{clone:?}").contains("redacted"));
    for bad in ["", "abc", "xyz", "ab cd", " abcd", "ab cd "] {
        assert!(matches!(RpcKey::from_hex(bad), Err(RpcError::InvalidKeyMaterial)), "{bad:?}");
    }
}

#[test]
fn identity_file_key_needs_sixty_four_bytes() {
    let dir = tempfile::tempdir().expect("tempdir");
    let storage = dir.path().join("storage");
    std::fs::create_dir_all(&storage).expect("storage dir");
    let identity = storage.join("transport_identity");
    std::fs::write(&identity, [7u8; 64]).expect("write identity");
    let key = RpcKey::from_transport_identity_file(&identity).expect("identity key");
    assert!(format!("{key:?}").contains("redacted"));
    std::fs::write(&identity, [7u8; 32]).expect("rewrite identity");
    assert!(matches!(
        RpcKey::from_transport_identity_file(&identity),
        Err(RpcError::InvalidKeyMaterial)
    ));
}

#[test]
fn stats_structs_are_comparable_values() {
    assert_value::<InterfaceStat>();
    assert_value::<InterfaceStats>();
}

#[tokio::test]
async fn closed_port_is_connect_and_the_future_is_send() {
    let port = free_loopback_port().await;
    let endpoint = RpcEndpoint::Tcp(([127, 0, 0, 1], port).into());
    let key = RpcKey::from_hex(KEY_HEX).expect("hex key");
    assert_send(interface_stats(&endpoint, &key, WAIT));
    let started = Instant::now();
    let result = interface_stats(&endpoint, &key, WAIT).await;
    assert!(matches!(result, Err(RpcError::Connect(_))), "{result:?}");
    assert!(started.elapsed() < Duration::from_secs(2), "{:?}", started.elapsed());
}

#[tokio::test]
async fn closed_port_connect_has_the_refused_io_kind() {
    let port = free_loopback_port().await;
    let endpoint = RpcEndpoint::Tcp(([127, 0, 0, 1], port).into());
    let key = RpcKey::from_hex(KEY_HEX).expect("hex key");
    let result = interface_stats(&endpoint, &key, WAIT).await;
    assert!(matches!(result, Err(RpcError::Connect(_))), "{result:?}");
    assert_eq!(
        result.as_ref().err().and_then(RpcError::io_kind),
        Some(std::io::ErrorKind::ConnectionRefused)
    );
}

#[tokio::test]
async fn server_that_hangs_up_at_once_is_io() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind loopback");
    let addr = listener.local_addr().expect("listener address");
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("accept");
        stream.shutdown().await.expect("shutdown at once");
    });
    let key = RpcKey::from_hex(KEY_HEX).expect("hex key");
    let result = interface_stats(&RpcEndpoint::Tcp(addr), &key, WAIT).await;
    assert!(!matches!(result, Err(RpcError::Protocol(_))), "{result:?}");
    match &result {
        Err(err @ RpcError::Io(io)) => {
            assert_eq!(io.kind(), std::io::ErrorKind::UnexpectedEof);
            assert!(err.source().is_some(), "{err:?}");
        }
        other => panic!("expected Io, got {other:?}"),
    }
    assert_eq!(
        result.as_ref().err().and_then(RpcError::io_kind),
        Some(std::io::ErrorKind::UnexpectedEof)
    );
    server.await.expect("server task");
}

#[tokio::test]
async fn readme_recipe_runs_against_a_closed_port() {
    let dir = tempfile::tempdir().expect("tempdir");
    let storage = dir.path().join("storage");
    std::fs::create_dir_all(&storage).expect("storage dir");
    std::fs::write(storage.join("transport_identity"), [7u8; 64]).expect("write identity");
    let port = free_loopback_port().await;
    let result = readme_recipe(dir.path(), RpcEndpoint::Tcp(([127, 0, 0, 1], port).into())).await;
    assert!(matches!(result, Err(RpcError::Connect(_))), "{result:?}");
}
