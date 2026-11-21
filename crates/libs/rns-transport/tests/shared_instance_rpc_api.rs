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
//! `UnexpectedEof` kind behind `io_kind()`; so is one that hangs up mid-frame, while one
//! that resets after a whole challenge is `Io` or `Connect` of a retryable kind; one that
//! writes garbage is `Protocol` and one that stays silent is `Timeout`, both with no io
//! kind; and `io_kind()` is `Some` for exactly `Io` and `Connect`. The path calls
//! (`path_table`, `next_hop_interface_name`, `first_hop_timeout`, `drop_path`,
//! `drop_all_via`, `drop_path_settled`) are held, against a loopback server that completes
//! the stdlib handshake honestly and answers one canned reply per connection, to the
//! request bytes rnsd's `rpc_loop` reads (`max_hops` always present and nil for `None`,
//! `RNS/Reticulum.py:1283-1285`; the hash as a 16-byte msgpack bin, the form umsgpack
//! gives the `bytes` keys of `Transport.path_table`, `RNS/Transport.py:2459`,
//! `RNS/vendor/umsgpack.py:505-506`) and to rnsd's reply shapes (`PathRow`s read by name
//! from `get_path_table`'s maps, `RNS/Reticulum.py:1638-1648`, with the int `0` timestamp
//! of a dropped path, `RNS/Transport.py:3219-3226`; the str `"None"` as no next hop,
//! `RNS/Reticulum.py:1715`; int or float seconds); their futures are `Send`; and
//! `is_transient()` agrees with the README's retry rule as a consumer writes it over every
//! variant and kind.

use std::error::Error;
use std::fmt::Debug;
use std::path::Path;
use std::time::{Duration, Instant};

use hmac::{Hmac, Mac};
use rmpv::Value;
use sha2::Sha256;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use rns_transport::hash::AddressHash;
use rns_transport::transport::shared_instance_rpc::{
    drop_all_via, drop_path, drop_path_settled, first_hop_timeout, interface_stats,
    next_hop_interface_name, path_table, InterfaceStat, InterfaceStats, PathRow, RpcEndpoint,
    RpcError, RpcKey,
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

/// The README's retry rule as a consumer writes it: the kind through `io_kind()`, not the
/// variant.
fn readme_retryable(err: &RpcError) -> bool {
    use std::io::ErrorKind::{BrokenPipe, ConnectionAborted, ConnectionReset, UnexpectedEof};
    matches!(err, RpcError::Timeout)
        || matches!(
            err.io_kind(),
            Some(ConnectionReset | ConnectionAborted | BrokenPipe | UnexpectedEof)
        )
}

#[test]
fn io_kind_is_some_for_exactly_io_and_connect() {
    use std::io::ErrorKind;
    let io = RpcError::Io(std::io::Error::new(ErrorKind::ConnectionReset, "peer reset"));
    assert_eq!(io.io_kind(), Some(ErrorKind::ConnectionReset));
    assert!(readme_retryable(&io), "{io:?}");
    let text = io.to_string();
    assert!(text.contains("io error after connect"), "{text}");
    assert!(text.contains("peer reset"), "{text}");
    assert_eq!(io.source().map(ToString::to_string).as_deref(), Some("peer reset"), "{io:?}");
    for kind in [ErrorKind::ConnectionAborted, ErrorKind::BrokenPipe, ErrorKind::UnexpectedEof] {
        let err = RpcError::Io(std::io::Error::from(kind));
        assert_eq!(err.io_kind(), Some(kind));
        assert!(readme_retryable(&err), "{err:?}");
    }
    let refused = RpcError::Connect(std::io::Error::from(ErrorKind::ConnectionRefused));
    assert_eq!(refused.io_kind(), Some(ErrorKind::ConnectionRefused));
    assert!(!readme_retryable(&refused), "{refused:?}");
    let reset_at_connect = RpcError::Connect(std::io::Error::from(ErrorKind::ConnectionReset));
    assert_eq!(reset_at_connect.io_kind(), Some(ErrorKind::ConnectionReset));
    assert!(readme_retryable(&reset_at_connect), "{reset_at_connect:?}");
    assert!(readme_retryable(&RpcError::Timeout));
    for err in [
        RpcError::Authentication,
        RpcError::Protocol("short".into()),
        RpcError::Decode("shape".into()),
        RpcError::UnsupportedDigest("sha1".into()),
        RpcError::InvalidKeyMaterial,
        RpcError::Timeout,
    ] {
        assert_eq!(err.io_kind(), None, "{err:?}");
    }
    assert!(!readme_retryable(&RpcError::Authentication));
    assert!(!readme_retryable(&RpcError::Protocol("short".into())));
}

/// Every variant, and every kind the README names, behind `Io` and `Connect`.
fn every_variant_and_kind() -> Vec<RpcError> {
    use std::io::{Error, ErrorKind};
    let mut errors = Vec::new();
    for kind in [
        ErrorKind::ConnectionReset,
        ErrorKind::ConnectionAborted,
        ErrorKind::BrokenPipe,
        ErrorKind::UnexpectedEof,
    ] {
        errors.push(RpcError::Io(Error::from(kind)));
        errors.push(RpcError::Connect(Error::from(kind)));
    }
    errors.extend([
        RpcError::Connect(Error::from(ErrorKind::ConnectionRefused)),
        RpcError::Io(Error::from(ErrorKind::Other)),
        RpcError::Timeout,
        RpcError::Authentication,
        RpcError::Protocol("short".into()),
        RpcError::Decode("shape".into()),
        RpcError::UnsupportedDigest("sha1".into()),
        RpcError::InvalidKeyMaterial,
    ]);
    errors
}

#[test]
fn is_transient_agrees_with_the_readme_retry_rule() {
    let errors = every_variant_and_kind();
    for err in &errors {
        assert_eq!(err.is_transient(), readme_retryable(err), "{err:?}");
    }
    // Four kinds on each of Io and Connect, and Timeout; the rest, ConnectionRefused included, stay.
    assert_eq!(errors.iter().filter(|err| err.is_transient()).count(), 9);
    assert!(RpcError::Timeout.is_transient());
    assert!(!RpcError::Connect(std::io::ErrorKind::ConnectionRefused.into()).is_transient());
}

/// A loopback listener that writes `bytes` to the first client and then does `after`.
async fn one_shot_server<F, Fut>(
    bytes: Vec<u8>,
    after: F,
) -> (std::net::SocketAddr, tokio::task::JoinHandle<()>)
where
    F: FnOnce(tokio::net::TcpStream) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = ()> + Send,
{
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind loopback");
    let addr = listener.local_addr().expect("listener address");
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("accept");
        stream.write_all(&bytes).await.expect("write");
        stream.flush().await.expect("flush");
        after(stream).await;
    });
    (addr, server)
}

/// Hold the socket open until the client goes away, so the client's read sees whatever
/// the server wrote and nothing more until its budget runs out.
async fn hold_until_client_leaves(mut stream: tokio::net::TcpStream) {
    let mut sink = [0u8; 1024];
    while let Ok(n) = stream.read(&mut sink).await {
        if n == 0 {
            break;
        }
    }
}

#[tokio::test]
async fn server_that_writes_garbage_is_protocol_with_no_io_kind() {
    let (addr, server) =
        one_shot_server(b"not a frame at all, just bytes".to_vec(), hold_until_client_leaves).await;
    let key = RpcKey::from_hex(KEY_HEX).expect("hex key");
    let result = interface_stats(&RpcEndpoint::Tcp(addr), &key, WAIT).await;
    match &result {
        Err(err @ RpcError::Protocol(_)) => {
            assert_eq!(err.io_kind(), None, "{err:?}");
            assert!(!readme_retryable(err), "{err:?}");
        }
        other => panic!("expected Protocol, got {other:?}"),
    }
    server.await.expect("server task");
}

#[tokio::test]
async fn server_that_stays_silent_is_timeout_with_no_io_kind() {
    let (addr, server) = one_shot_server(Vec::new(), hold_until_client_leaves).await;
    let key = RpcKey::from_hex(KEY_HEX).expect("hex key");
    let started = Instant::now();
    let result = interface_stats(&RpcEndpoint::Tcp(addr), &key, Duration::from_millis(700)).await;
    let elapsed = started.elapsed();
    match &result {
        Err(err @ RpcError::Timeout) => {
            assert_eq!(err.io_kind(), None);
            assert!(readme_retryable(err));
        }
        other => panic!("expected Timeout, got {other:?}"),
    }
    assert!(elapsed >= Duration::from_millis(600), "{elapsed:?}");
    assert!(elapsed < Duration::from_secs(4), "{elapsed:?}");
    server.await.expect("server task");
}

#[tokio::test]
async fn server_that_hangs_up_mid_frame_is_io_unexpected_eof() {
    // A 48-byte header followed by ten bytes and a FIN: the bytes read were fine, the
    // stream ended early.
    let mut bytes = 48i32.to_be_bytes().to_vec();
    bytes.extend_from_slice(b"#CHALLENGE");
    let (addr, server) = one_shot_server(bytes, |mut stream| async move {
        stream.shutdown().await.expect("shutdown mid-frame");
    })
    .await;
    let key = RpcKey::from_hex(KEY_HEX).expect("hex key");
    let result = interface_stats(&RpcEndpoint::Tcp(addr), &key, WAIT).await;
    match &result {
        Err(err @ RpcError::Io(io)) => {
            assert_eq!(io.kind(), std::io::ErrorKind::UnexpectedEof, "{err:?}");
            assert_eq!(err.io_kind(), Some(std::io::ErrorKind::UnexpectedEof));
            assert!(readme_retryable(err), "{err:?}");
        }
        other => panic!("expected Io, got {other:?}"),
    }
    server.await.expect("server task");
}

#[tokio::test]
async fn server_that_resets_after_a_whole_challenge_is_a_retryable_kind_on_either_variant() {
    // A complete legacy challenge (`#CHALLENGE#` + 20 bytes) and then `SO_LINGER 0`: the
    // bytes were right, the peer departed mid-call, so whichever of the client's next
    // write or read meets the reset, the call is `Io` of a kind worth a retry. On macOS
    // the reset can land in the same reactor tick as the accept, before the dial's
    // `connect()` has returned, and the call is then `Connect` of kind `ConnectionReset`;
    // a consumer classifies by `io_kind()`, so either variant is admitted, never
    // `Protocol`, `Authentication` or `ConnectionRefused`.
    let mut bytes = 31i32.to_be_bytes().to_vec();
    bytes.extend_from_slice(b"#CHALLENGE#");
    bytes.extend_from_slice(&[0x5au8; 20]);
    let (addr, server) = one_shot_server(bytes, |stream| async move {
        socket2::SockRef::from(&stream).set_linger(Some(Duration::ZERO)).expect("linger zero");
        drop(stream);
    })
    .await;
    let key = RpcKey::from_hex(KEY_HEX).expect("hex key");
    let result = interface_stats(&RpcEndpoint::Tcp(addr), &key, WAIT).await;
    let err = result.as_ref().expect_err("the server never replies");
    assert!(matches!(err, RpcError::Io(_) | RpcError::Connect(_)), "{err:?}");
    assert!(readme_retryable(err), "not a retryable kind: {err:?}");
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

// ---------------------------------------------------------------------------------------
// The edges the documented contract promises that the pins below leave to the reader:
// one budget shared across every dial of `drop_path_settled`, a nil next hop, the
// silence-after-handshake form of `Timeout` on a path call, a `via` of the wrong length,
// and a wrong-shaped poll reply surfacing at once.
// ---------------------------------------------------------------------------------------

/// A loopback listener that answers `first` to its first connection and `rest` to every
/// connection after that, forever; the counter is the number of connections served. The
/// task never ends on its own: abort it.
async fn endless_server(
    first: Value,
    rest: Value,
) -> (RpcEndpoint, std::sync::Arc<std::sync::atomic::AtomicUsize>, tokio::task::JoinHandle<()>) {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind loopback");
    let endpoint = RpcEndpoint::Tcp(listener.local_addr().expect("listener address"));
    let served = std::sync::Arc::new(AtomicUsize::new(0));
    let counter = served.clone();
    let server = tokio::spawn(async move {
        let mut reply = first;
        loop {
            let (mut stream, _) = listener.accept().await.expect("accept");
            let request = serve_honest_handshake(&mut stream).await;
            let n = counter.fetch_add(1, Ordering::SeqCst);
            if n == 0 {
                assert_eq!(request, hash_request(DROP_PATH_REQUEST_HEAD), "first dial is the drop");
            } else {
                assert_eq!(request, hash_request(NEXT_HOP_REQUEST_HEAD), "dial {n} is a poll");
            }
            stream.write_all(&frame(&pack(&reply))).await.expect("write reply");
            stream.flush().await.expect("flush");
            stream.shutdown().await.expect("fin");
            reply = rest.clone();
        }
    });
    (endpoint, served, server)
}

/// Plan (b): `drop_path`, then poll `next_hop_interface_name` every 50 ms until it reads
/// `None` or the SAME `timeout` budget (shared across all dials) runs out -> `Timeout`.
/// A next hop that never reads `None` (the destination was re-learned, as the rustdoc
/// warns) must end in `Timeout` close to the one budget, after several polls at the
/// 50 ms cadence: never a success, never a per-dial budget that multiplies the wait.
#[tokio::test]
async fn drop_path_settled_that_never_settles_is_timeout_under_one_shared_budget() {
    let (endpoint, served, server) =
        endless_server(Value::from(true), Value::from(INTERFACE_NAME)).await;
    let key = RpcKey::from_hex(KEY_HEX).expect("hex key");
    let budget = Duration::from_millis(700);
    let started = Instant::now();
    let result = drop_path_settled(&endpoint, &key, &destination(), budget).await;
    let elapsed = started.elapsed();
    server.abort();
    match &result {
        Err(err @ RpcError::Timeout) => {
            assert_eq!(err.io_kind(), None, "{err:?}");
            assert!(err.is_transient(), "{err:?}");
        }
        other => panic!("a path that never settles: expected Timeout, got {other:?}"),
    }
    assert!(elapsed >= Duration::from_millis(600), "{elapsed:?} returned before the budget");
    assert!(elapsed < Duration::from_millis(1200), "{elapsed:?} is more than one budget");
    let dials = served.load(std::sync::atomic::Ordering::SeqCst);
    // One drop and then polls at a 50 ms cadence: 700 ms holds a dozen; one drop and at
    // least four polls (five dials) rules out a single poll followed by a sleep to the budget.
    assert!(dials >= 5, "only {dials} dials in {elapsed:?}: no 50 ms poll cadence");
    eprintln!("drop_path_settled never settles: {dials} dials in {elapsed:?} -> {result:?}");
}

/// Plan: `next_hop_interface_name` reads nil as `None`, beside the str `"None"`.
#[tokio::test]
async fn next_hop_interface_name_reads_nil_as_none() {
    let key = RpcKey::from_hex(KEY_HEX).expect("hex key");
    let (endpoint, server) = canned_server(vec![Value::Nil]).await;
    let name = next_hop_interface_name(&endpoint, &key, &destination(), WAIT).await;
    assert_eq!(name.expect("next_hop_interface_name"), None);
    assert_eq!(served_request(server).await, hash_request(NEXT_HOP_REQUEST_HEAD));
}

/// Plan: `max_hops` is `Value::from(u8)`, so a 255 crosses the wire as a msgpack uint
/// rnsd's `call["max_hops"]` reads as the int 255, and a 0 as the int 0, not nil. Above
/// the positive fixint range the value takes the uint 8 form: 200 is `\xcc\xc8`, the
/// same int to rnsd.
#[tokio::test]
async fn path_table_max_hops_edges_cross_the_wire_as_their_integer() {
    let key = RpcKey::from_hex(KEY_HEX).expect("hex key");
    for max_hops in [0u8, 255u8] {
        let (endpoint, server) = canned_server(vec![Value::Array(vec![])]).await;
        path_table(&endpoint, &key, Some(max_hops), WAIT).await.expect("path_table");
        let request = served_request(server).await;
        let value = rmpv::decode::read_value(&mut request.as_slice()).expect("a msgpack value");
        assert_eq!(value["get"].as_str(), Some("path_table"), "{value:?}");
        assert_eq!(value["max_hops"].as_u64(), Some(u64::from(max_hops)), "{value:?}");
    }
    let (endpoint, server) = canned_server(vec![Value::Array(vec![])]).await;
    path_table(&endpoint, &key, Some(200), WAIT).await.expect("path_table");
    assert_eq!(
        served_request(server).await,
        b"\x82\xa3get\xaapath_table\xa8max_hops\xcc\xc8",
        "200 is the uint 8 form"
    );
}

/// An honest server that reads the request and then says nothing: every call wraps its
/// whole exchange in the one `timeout`, so the reply read is `Timeout`, not a hang.
async fn silent_after_the_request() -> (RpcEndpoint, tokio::task::JoinHandle<Vec<u8>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind loopback");
    let endpoint = RpcEndpoint::Tcp(listener.local_addr().expect("listener address"));
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("accept");
        let request = serve_honest_handshake(&mut stream).await;
        hold_until_client_leaves(stream).await;
        request
    });
    (endpoint, server)
}

#[tokio::test]
async fn path_table_against_a_server_silent_after_the_request_is_timeout() {
    let key = RpcKey::from_hex(KEY_HEX).expect("hex key");
    let (endpoint, server) = silent_after_the_request().await;
    let started = Instant::now();
    let result = path_table(&endpoint, &key, None, Duration::from_millis(700)).await;
    let elapsed = started.elapsed();
    assert!(matches!(result, Err(RpcError::Timeout)), "{result:?}");
    assert!(elapsed >= Duration::from_millis(600), "{elapsed:?}");
    assert!(elapsed < Duration::from_secs(4), "{elapsed:?}");
    assert_eq!(server.await.expect("server task"), PATH_TABLE_REQUEST_NO_MAX_HOPS);
}

/// The drop dial itself spends the helper's budget: silence after the drop request is
/// `Timeout` inside that one budget, with no poll ever sent.
#[tokio::test]
async fn drop_path_settled_against_a_server_silent_after_the_drop_is_timeout() {
    let key = RpcKey::from_hex(KEY_HEX).expect("hex key");
    let (endpoint, server) = silent_after_the_request().await;
    let started = Instant::now();
    let result =
        drop_path_settled(&endpoint, &key, &destination(), Duration::from_millis(700)).await;
    let elapsed = started.elapsed();
    assert!(matches!(result, Err(RpcError::Timeout)), "{result:?}");
    assert!(elapsed >= Duration::from_millis(600), "{elapsed:?}");
    assert!(elapsed < Duration::from_millis(1200), "{elapsed:?} is more than one budget");
    assert_eq!(server.await.expect("server task"), hash_request(DROP_PATH_REQUEST_HEAD));
}

/// Plan: 16-byte bins -> `AddressHash`; wrong length -> `Decode`. `via` is an
/// `AddressHash` too, so a 15-byte `via` is `Decode` just like a 15-byte `hash`.
#[tokio::test]
async fn path_table_via_of_the_wrong_length_is_decode() {
    let key = RpcKey::from_hex(KEY_HEX).expect("hex key");
    let short_via = map(vec![
        ("hash", Value::Binary(DESTINATION.to_vec())),
        ("timestamp", Value::F64(1.0)),
        ("via", Value::Binary(vec![0x5a; 15])),
        ("hops", Value::from(1)),
        ("expires", Value::F64(2.0)),
        ("interface", Value::from(INTERFACE_NAME)),
    ]);
    let (endpoint, server) = canned_server(vec![Value::Array(vec![short_via])]).await;
    let result = path_table(&endpoint, &key, None, WAIT).await;
    match &result {
        Err(err @ RpcError::Decode(_)) => {
            assert_eq!(err.io_kind(), None, "{err:?}");
            assert!(!err.is_transient(), "{err:?}");
        }
        other => panic!("15-byte via: expected Decode, got {other:?}"),
    }
    served_request(server).await;
}

/// A poll that reads a reply of the wrong shape is a `Decode` the helper surfaces at
/// once: it does not keep polling a server that answers nonsense until the budget runs
/// out, since `Decode` is not transient.
#[tokio::test]
async fn drop_path_settled_surfaces_a_wrong_shaped_poll_reply_as_decode_at_once() {
    let key = RpcKey::from_hex(KEY_HEX).expect("hex key");
    let (endpoint, server) = canned_server(vec![Value::from(true), Value::from(7)]).await;
    let started = Instant::now();
    let result = drop_path_settled(&endpoint, &key, &destination(), WAIT).await;
    let elapsed = started.elapsed();
    eprintln!("drop_path_settled wrong-shaped poll: {result:?} in {elapsed:?}");
    assert!(matches!(result, Err(RpcError::Decode(_))), "{result:?}");
    assert!(elapsed < Duration::from_secs(2), "{elapsed:?}");
    let requests = server.await.expect("server task");
    assert_eq!(requests.len(), 2, "{requests:?}");
}

/// A hash of the right length but the wrong msgpack type (a sixteen-char str where
/// rnsd writes a 16-byte bin, `RNS/vendor/umsgpack.py:505-506`) is `Decode`, for `hash`
/// and for `via` alike: the typed row is bytes-only, never a str read as bytes.
#[tokio::test]
async fn path_table_hash_or_via_as_a_sixteen_char_str_is_decode() {
    let key = RpcKey::from_hex(KEY_HEX).expect("hex key");
    let sixteen = "0123456789abcdef";
    for (label, hash, via) in [
        ("hash", Value::from(sixteen), Value::Binary(VIA.to_vec())),
        ("via", Value::Binary(DESTINATION.to_vec()), Value::from(sixteen)),
    ] {
        let row = map(vec![
            ("hash", hash),
            ("timestamp", Value::F64(1.0)),
            ("via", via),
            ("hops", Value::from(1)),
            ("expires", Value::F64(2.0)),
            ("interface", Value::from(INTERFACE_NAME)),
        ]);
        let (endpoint, server) = canned_server(vec![Value::Array(vec![row])]).await;
        let result = path_table(&endpoint, &key, None, WAIT).await;
        match &result {
            Err(err @ RpcError::Decode(message)) => {
                assert!(message.contains(label), "{label}: {err:?}");
                assert_eq!(err.io_kind(), None, "{label}: {err:?}");
                assert!(!err.is_transient(), "{label}: {err:?}");
            }
            other => panic!("{label} as a 16-char str: expected Decode, got {other:?}"),
        }
        served_request(server).await;
    }
}

/// A mistyped drop reply inside `drop_path_settled` (an int where `expire_path` returns
/// a bool, `RNS/Transport.py:3219-3226`) is `Decode` after exactly one dial: the helper
/// polls only after a `true` drop.
#[tokio::test]
async fn drop_path_settled_with_a_mistyped_drop_reply_is_decode_without_polling() {
    let key = RpcKey::from_hex(KEY_HEX).expect("hex key");
    let (endpoint, server) = canned_server(vec![Value::from(1)]).await;
    let started = Instant::now();
    let result = drop_path_settled(&endpoint, &key, &destination(), WAIT).await;
    assert!(matches!(result, Err(RpcError::Decode(_))), "{result:?}");
    assert!(started.elapsed() < Duration::from_secs(1), "no poll: {:?}", started.elapsed());
    assert_eq!(served_request(server).await, hash_request(DROP_PATH_REQUEST_HEAD));
}

const CHALLENGE: &[u8] = b"#CHALLENGE#";
const WELCOME: &[u8] = b"#WELCOME#";

const DESTINATION: [u8; 16] = [
    0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e, 0x1f,
];
const VIA: [u8; 16] = [0x5a; 16];
const INTERFACE_NAME: &str = "TCPInterface[A/127.0.0.1:4242]";

const PATH_TABLE_REQUEST_NO_MAX_HOPS: &[u8] = b"\x82\xa3get\xaapath_table\xa8max_hops\xc0";
const PATH_TABLE_REQUEST_MAX_HOPS_FIVE: &[u8] = b"\x82\xa3get\xaapath_table\xa8max_hops\x05";
const NEXT_HOP_REQUEST_HEAD: &[u8] = b"\x82\xa3get\xb0next_hop_if_name\xb0destination_hash\xc4\x10";
const DROP_PATH_REQUEST_HEAD: &[u8] = b"\x82\xa4drop\xa4path\xb0destination_hash\xc4\x10";
const DROP_ALL_VIA_REQUEST_HEAD: &[u8] = b"\x82\xa4drop\xa7all_via\xb0destination_hash\xc4\x10";
const FIRST_HOP_TIMEOUT_REQUEST_HEAD: &[u8] =
    b"\x82\xa3get\xb1first_hop_timeout\xb0destination_hash\xc4\x10";

fn destination() -> AddressHash {
    AddressHash::new(DESTINATION)
}

/// `head` is the fixmap up to and including the bin8 header `\xc4\x10`; the destination's
/// 16 bytes follow.
fn hash_request(head: &[u8]) -> Vec<u8> {
    let mut bytes = head.to_vec();
    bytes.extend_from_slice(&DESTINATION);
    bytes
}

fn authkey() -> Vec<u8> {
    (0..KEY_HEX.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&KEY_HEX[i..i + 2], 16).expect("hex byte"))
        .collect()
}

/// One stdlib `send_bytes` frame: signed big-endian 32-bit length, then the payload.
fn frame(payload: &[u8]) -> Vec<u8> {
    let mut out = i32::try_from(payload.len()).expect("frame fits").to_be_bytes().to_vec();
    out.extend_from_slice(payload);
    out
}

async fn recv_frame(stream: &mut TcpStream) -> std::io::Result<Vec<u8>> {
    let mut header = [0u8; 4];
    stream.read_exact(&mut header).await?;
    let len = usize::try_from(i32::from_be_bytes(header)).expect("non-negative length");
    let mut payload = vec![0u8; len];
    stream.read_exact(&mut payload).await?;
    Ok(payload)
}

/// The stdlib's `deliver_challenge` message in the modern form: `{sha256}` + 40 bytes.
fn server_challenge() -> Vec<u8> {
    let mut message = b"{sha256}".to_vec();
    message.extend_from_slice(&[0x42u8; 40]);
    message
}

/// The stdlib's `_create_response` for a `{sha256}`-prefixed message: the MAC covers the
/// whole message, header included, and the reply carries the same prefix.
fn answer(authkey: &[u8], message: &[u8]) -> Vec<u8> {
    assert!(message.starts_with(b"{sha256}"), "the client challenges with {{sha256}}: {message:?}");
    let mut mac = Hmac::<Sha256>::new_from_slice(authkey).expect("hmac key");
    mac.update(message);
    let mut out = b"{sha256}".to_vec();
    out.extend_from_slice(&mac.finalize().into_bytes());
    out
}

/// Serve the stdlib's mutual handshake honestly (`deliver_challenge`, then
/// `answer_challenge`) and return the request frame the client sends next.
async fn serve_honest_handshake(stream: &mut TcpStream) -> Vec<u8> {
    let authkey = authkey();
    let message = server_challenge();
    let mut challenge = CHALLENGE.to_vec();
    challenge.extend_from_slice(&message);
    stream.write_all(&frame(&challenge)).await.expect("write challenge");
    let response = recv_frame(stream).await.expect("the client's answer");
    assert_eq!(response, answer(&authkey, &message), "the client's answer to our challenge");
    stream.write_all(&frame(WELCOME)).await.expect("write welcome");
    let theirs = recv_frame(stream).await.expect("the client's challenge");
    assert!(theirs.starts_with(CHALLENGE), "{theirs:?}");
    let ours = answer(&authkey, &theirs[CHALLENGE.len()..]);
    stream.write_all(&frame(&ours)).await.expect("write our answer");
    let welcome = recv_frame(stream).await.expect("the client's verdict");
    assert_eq!(welcome, WELCOME, "the client rejected a correct answer");
    recv_frame(stream).await.expect("the request")
}

fn pack(value: &Value) -> Vec<u8> {
    let mut bytes = Vec::new();
    rmpv::encode::write_value(&mut bytes, value).expect("pack into a vec");
    bytes
}

/// A loopback listener that takes one connection per canned reply: the honest handshake,
/// the request, the reply, then the close `rpc_loop` does. The task yields every request
/// read, in connection order; a dial after the last reply is refused.
async fn canned_server(
    replies: Vec<Value>,
) -> (RpcEndpoint, tokio::task::JoinHandle<Vec<Vec<u8>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind loopback");
    let endpoint = RpcEndpoint::Tcp(listener.local_addr().expect("listener address"));
    let server = tokio::spawn(async move {
        let mut requests = Vec::with_capacity(replies.len());
        for reply in replies {
            let (mut stream, _) = listener.accept().await.expect("accept");
            requests.push(serve_honest_handshake(&mut stream).await);
            stream.write_all(&frame(&pack(&reply))).await.expect("write reply");
            stream.flush().await.expect("flush");
            stream.shutdown().await.expect("fin");
        }
        requests
    });
    (endpoint, server)
}

/// The one request a single-reply [`canned_server`] read.
async fn served_request(server: tokio::task::JoinHandle<Vec<Vec<u8>>>) -> Vec<u8> {
    let mut requests = server.await.expect("server task");
    assert_eq!(requests.len(), 1, "{requests:?}");
    requests.remove(0)
}

fn map(pairs: Vec<(&str, Value)>) -> Value {
    Value::Map(pairs.into_iter().map(|(key, value)| (Value::from(key), value)).collect())
}

/// A row as `get_path_table` builds it (`RNS/Reticulum.py:1638-1648`), with `extra`
/// keys appended as a newer server might add them.
fn row(hash: Vec<u8>, timestamp: Value, hops: u8, extra: Vec<(&str, Value)>) -> Value {
    let mut pairs = vec![
        ("hash", Value::Binary(hash)),
        ("timestamp", timestamp),
        ("via", Value::Binary(VIA.to_vec())),
        ("hops", Value::from(hops)),
        ("expires", Value::F64(1_700_000_600.5)),
        ("interface", Value::from(INTERFACE_NAME)),
    ];
    pairs.extend(extra);
    map(pairs)
}

/// The request carries `max_hops` as nil for `None` and as the int otherwise, and an
/// empty reply list, what a fresh rnsd with no peers sends (`RNS/Reticulum.py:1638`
/// over an empty `Transport.path_table`), is `Ok` and empty, not a decode error.
#[tokio::test]
async fn path_table_always_sends_max_hops_nil_for_none() {
    let key = RpcKey::from_hex(KEY_HEX).expect("hex key");
    let (endpoint, server) = canned_server(vec![Value::Array(vec![])]).await;
    let rows = path_table(&endpoint, &key, None, WAIT).await.expect("path_table");
    assert!(rows.is_empty(), "{rows:?}");
    assert_eq!(served_request(server).await, PATH_TABLE_REQUEST_NO_MAX_HOPS);

    let (endpoint, server) = canned_server(vec![Value::Array(vec![])]).await;
    path_table(&endpoint, &key, Some(5), WAIT).await.expect("path_table");
    let request = served_request(server).await;
    assert_eq!(request, PATH_TABLE_REQUEST_MAX_HOPS_FIVE);
}

#[tokio::test]
async fn path_table_rows_are_read_by_name_and_keep_the_raw_entry() {
    assert_value::<PathRow>();
    let learned = row(DESTINATION.to_vec(), Value::F64(1_700_000_000.25), 3, vec![]);
    // The int 0 is what `expire_path` leaves in the timestamp slot
    // (`RNS/Transport.py:3219-3226`).
    let dropped = row(vec![0x03; 16], Value::from(0), 0, vec![("extra", Value::from("unknown"))]);
    let (endpoint, server) =
        canned_server(vec![Value::Array(vec![learned.clone(), dropped.clone()])]).await;
    let key = RpcKey::from_hex(KEY_HEX).expect("hex key");
    let rows = path_table(&endpoint, &key, None, WAIT).await.expect("path_table");
    served_request(server).await;

    assert_eq!(rows.len(), 2, "{rows:?}");
    let PathRow { hash, timestamp, via, hops, expires, interface, raw, .. } = &rows[0];
    assert_eq!(*hash, destination());
    assert_eq!(*timestamp, 1_700_000_000.25);
    assert_eq!(*via, AddressHash::new(VIA));
    assert_eq!(*hops, 3);
    assert_eq!(*expires, 1_700_000_600.5);
    assert_eq!(interface, INTERFACE_NAME);
    assert_eq!(*raw, learned);

    assert_eq!(rows[1].hash, AddressHash::new([0x03; 16]));
    assert_eq!(rows[1].timestamp, 0.0);
    assert_eq!(rows[1].hops, 0);
    assert_eq!(rows[1].raw, dropped);
    assert_eq!(rows[1].raw["extra"].as_str(), Some("unknown"), "{:?}", rows[1].raw);
    assert_ne!(rows[0], rows[1]);
}

#[tokio::test]
async fn hash_calls_send_the_destination_as_a_sixteen_byte_bin() {
    let key = RpcKey::from_hex(KEY_HEX).expect("hex key");

    let (endpoint, server) = canned_server(vec![Value::from("None")]).await;
    let name = next_hop_interface_name(&endpoint, &key, &destination(), WAIT).await;
    assert_eq!(name.expect("next_hop_interface_name"), None);
    assert_eq!(served_request(server).await, hash_request(NEXT_HOP_REQUEST_HEAD));

    let (endpoint, server) = canned_server(vec![Value::from(6)]).await;
    let seconds = first_hop_timeout(&endpoint, &key, &destination(), WAIT).await;
    assert_eq!(seconds.expect("first_hop_timeout"), 6.0);
    assert_eq!(served_request(server).await, hash_request(FIRST_HOP_TIMEOUT_REQUEST_HEAD));

    let (endpoint, server) = canned_server(vec![Value::from(true)]).await;
    let dropped = drop_path(&endpoint, &key, &destination(), WAIT).await;
    assert!(dropped.expect("drop_path"));
    assert_eq!(served_request(server).await, hash_request(DROP_PATH_REQUEST_HEAD));

    let (endpoint, server) = canned_server(vec![Value::from(2)]).await;
    let count = drop_all_via(&endpoint, &key, &destination(), WAIT).await;
    assert_eq!(count.expect("drop_all_via"), 2);
    assert_eq!(served_request(server).await, hash_request(DROP_ALL_VIA_REQUEST_HEAD));
}

#[tokio::test]
async fn hash_call_replies_read_as_rnsd_writes_them() {
    let key = RpcKey::from_hex(KEY_HEX).expect("hex key");

    let (endpoint, server) = canned_server(vec![Value::from(INTERFACE_NAME)]).await;
    let name = next_hop_interface_name(&endpoint, &key, &destination(), WAIT).await;
    assert_eq!(name.expect("next_hop_interface_name"), Some(INTERFACE_NAME.to_string()));
    served_request(server).await;

    let (endpoint, server) = canned_server(vec![Value::from(false)]).await;
    let dropped = drop_path(&endpoint, &key, &destination(), WAIT).await;
    assert!(!dropped.expect("drop_path"));
    served_request(server).await;

    let (endpoint, server) = canned_server(vec![Value::F64(6.0128)]).await;
    let seconds = first_hop_timeout(&endpoint, &key, &destination(), WAIT).await;
    assert_eq!(seconds.expect("first_hop_timeout"), 6.0128);
    served_request(server).await;
}

#[tokio::test]
async fn wrong_shaped_path_replies_are_decode_without_an_io_kind() {
    let key = RpcKey::from_hex(KEY_HEX).expect("hex key");

    let not_an_array = map(vec![("interfaces", Value::Array(vec![]))]);
    let (endpoint, server) = canned_server(vec![not_an_array]).await;
    let result = path_table(&endpoint, &key, None, WAIT).await;
    match &result {
        Err(err @ RpcError::Decode(_)) => {
            assert_eq!(err.io_kind(), None, "{err:?}");
            assert!(!err.is_transient(), "{err:?}");
        }
        other => panic!("non-array path_table reply: expected Decode, got {other:?}"),
    }
    served_request(server).await;

    let short_hash = row(vec![0x01; 15], Value::F64(1.0), 1, vec![]);
    let (endpoint, server) = canned_server(vec![Value::Array(vec![short_hash])]).await;
    let result = path_table(&endpoint, &key, None, WAIT).await;
    assert!(matches!(result, Err(RpcError::Decode(_))), "15-byte hash: {result:?}");
    served_request(server).await;
}

#[tokio::test]
async fn drop_path_settled_polls_the_next_hop_until_it_reads_none() {
    let (endpoint, server) =
        canned_server(vec![Value::from(true), Value::from(INTERFACE_NAME), Value::from("None")])
            .await;
    let key = RpcKey::from_hex(KEY_HEX).expect("hex key");
    let started = Instant::now();
    let settled = drop_path_settled(&endpoint, &key, &destination(), WAIT).await;
    assert!(settled.expect("drop_path_settled"));
    // One poll read a name before the second read "None": one sleep of the documented
    // 50 ms cadence, which tokio's sleep never cuts short.
    assert!(
        started.elapsed() >= Duration::from_millis(50),
        "{:?} is shorter than one poll interval",
        started.elapsed()
    );
    assert!(started.elapsed() < WAIT, "{:?}", started.elapsed());
    let requests = server.await.expect("server task");
    assert_eq!(requests.len(), 3, "{requests:?}");
    assert_eq!(requests[0], hash_request(DROP_PATH_REQUEST_HEAD));
    assert_eq!(requests[1], hash_request(NEXT_HOP_REQUEST_HEAD));
    assert_eq!(requests[2], hash_request(NEXT_HOP_REQUEST_HEAD));
}

#[tokio::test]
async fn drop_path_settled_without_a_path_dials_exactly_once() {
    let (endpoint, server) = canned_server(vec![Value::from(false)]).await;
    let key = RpcKey::from_hex(KEY_HEX).expect("hex key");
    let settled = drop_path_settled(&endpoint, &key, &destination(), WAIT).await;
    assert!(!settled.expect("drop_path_settled"));
    assert_eq!(served_request(server).await, hash_request(DROP_PATH_REQUEST_HEAD));
}

#[tokio::test]
async fn path_call_futures_are_send() {
    let port = free_loopback_port().await;
    let endpoint = RpcEndpoint::Tcp(([127, 0, 0, 1], port).into());
    let key = RpcKey::from_hex(KEY_HEX).expect("hex key");
    let destination = destination();
    assert_send(path_table(&endpoint, &key, None, WAIT));
    assert_send(next_hop_interface_name(&endpoint, &key, &destination, WAIT));
    assert_send(first_hop_timeout(&endpoint, &key, &destination, WAIT));
    assert_send(drop_path(&endpoint, &key, &destination, WAIT));
    assert_send(drop_all_via(&endpoint, &key, &destination, WAIT));
    assert_send(drop_path_settled(&endpoint, &key, &destination, WAIT));
    let result = drop_path(&endpoint, &key, &destination, WAIT).await;
    assert!(matches!(result, Err(RpcError::Connect(_))), "{result:?}");
}

// ---------------------------------------------------------------------------------------
// The typed-decode edges the documented contract implies: `hops` is a `u8`, so an int
// outside a byte is `Decode` (never a silent wrap); a row short of a named field and a
// non-map row are `Decode`; the hash calls' scalar replies are typed, so a bool count, a
// negative count, a str timeout and an int drop are
// `Decode`; concurrent calls each take their own connection and never cross replies;
// and a `drop_path_settled` whose poll can no longer reach the server never reports a
// settled drop.
// ---------------------------------------------------------------------------------------

/// Plan: `PathRow { hops: u8, .. }`. rnsd's `hops` is a Python int; a value the field
/// cannot hold is a `Decode`, with no io kind and not transient, never a wrapped byte.
#[tokio::test]
async fn path_table_hops_outside_a_byte_is_decode() {
    let key = RpcKey::from_hex(KEY_HEX).expect("hex key");
    for hops in [Value::from(256), Value::from(-1)] {
        let pairs = vec![
            ("hash", Value::Binary(DESTINATION.to_vec())),
            ("timestamp", Value::F64(1.0)),
            ("via", Value::Binary(VIA.to_vec())),
            ("hops", hops.clone()),
            ("expires", Value::F64(2.0)),
            ("interface", Value::from(INTERFACE_NAME)),
        ];
        let (endpoint, server) = canned_server(vec![Value::Array(vec![map(pairs)])]).await;
        let result = path_table(&endpoint, &key, None, WAIT).await;
        match &result {
            Err(err @ RpcError::Decode(_)) => {
                assert_eq!(err.io_kind(), None, "{err:?}");
                assert!(!err.is_transient(), "{err:?}");
            }
            other => panic!("hops {hops:?}: expected Decode, got {other:?}"),
        }
        served_request(server).await;
    }
}

/// Plan: rows are read by name from `get_path_table`'s six-key maps. A row short of one
/// of the six, and a list element that is not a map at all, are `Decode`.
#[tokio::test]
async fn path_table_row_short_of_a_field_or_not_a_map_is_decode() {
    let key = RpcKey::from_hex(KEY_HEX).expect("hex key");
    let no_expires = map(vec![
        ("hash", Value::Binary(DESTINATION.to_vec())),
        ("timestamp", Value::F64(1.0)),
        ("via", Value::Binary(VIA.to_vec())),
        ("hops", Value::from(1)),
        ("interface", Value::from(INTERFACE_NAME)),
    ]);
    let good = row(DESTINATION.to_vec(), Value::F64(1.0), 1, vec![]);
    for reply in
        [Value::Array(vec![no_expires]), Value::Array(vec![good, Value::from("not a row")])]
    {
        let (endpoint, server) = canned_server(vec![reply.clone()]).await;
        let result = path_table(&endpoint, &key, None, WAIT).await;
        assert!(matches!(result, Err(RpcError::Decode(_))), "{reply:?} -> {result:?}");
        served_request(server).await;
    }
}

/// Plan: `drop_all_via -> u64`, `first_hop_timeout -> f64`, `drop_path -> bool`. A reply
/// of another type is `Decode` for each of them, not a coerced value.
#[tokio::test]
async fn wrong_shaped_hash_call_replies_are_decode() {
    let key = RpcKey::from_hex(KEY_HEX).expect("hex key");
    let destination = destination();

    for reply in [Value::from(true), Value::from(-1)] {
        let (endpoint, server) = canned_server(vec![reply.clone()]).await;
        let result = drop_all_via(&endpoint, &key, &destination, WAIT).await;
        assert!(matches!(result, Err(RpcError::Decode(_))), "all_via {reply:?} -> {result:?}");
        served_request(server).await;
    }

    let (endpoint, server) = canned_server(vec![Value::from("6")]).await;
    let result = first_hop_timeout(&endpoint, &key, &destination, WAIT).await;
    assert!(matches!(result, Err(RpcError::Decode(_))), "first_hop_timeout str -> {result:?}");
    served_request(server).await;

    let (endpoint, server) = canned_server(vec![Value::from(1)]).await;
    let result = drop_path(&endpoint, &key, &destination, WAIT).await;
    assert!(matches!(result, Err(RpcError::Decode(_))), "drop_path int -> {result:?}");
    served_request(server).await;
}

/// Plan: one call per connection. Two calls in flight at once each take their own
/// connection and read their own reply: the server answers by what each request asks.
#[tokio::test]
async fn concurrent_path_calls_take_one_connection_each_and_never_cross_replies() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind loopback");
    let endpoint = RpcEndpoint::Tcp(listener.local_addr().expect("listener address"));
    let server = tokio::spawn(async move {
        let mut workers = Vec::new();
        for _ in 0..2 {
            let (mut stream, _) = listener.accept().await.expect("accept");
            workers.push(tokio::spawn(async move {
                let request = serve_honest_handshake(&mut stream).await;
                let reply = if request == hash_request(DROP_PATH_REQUEST_HEAD) {
                    Value::from(true)
                } else if request == hash_request(FIRST_HOP_TIMEOUT_REQUEST_HEAD) {
                    Value::from(6)
                } else {
                    panic!("unexpected request {request:?}");
                };
                stream.write_all(&frame(&pack(&reply))).await.expect("write reply");
                stream.shutdown().await.expect("fin");
            }));
        }
        for worker in workers {
            worker.await.expect("worker");
        }
    });
    let key = RpcKey::from_hex(KEY_HEX).expect("hex key");
    let destination = destination();
    let (dropped, seconds) = tokio::join!(
        drop_path(&endpoint, &key, &destination, WAIT),
        first_hop_timeout(&endpoint, &key, &destination, WAIT),
    );
    assert!(dropped.expect("drop_path"));
    assert_eq!(seconds.expect("first_hop_timeout"), 6.0);
    server.await.expect("server task");
}

/// Plan: the helper returns the drop's bool only once a poll reads `None`. When the
/// server goes away after answering the first poll with a name, the helper must not
/// report `Ok(true)`: whatever error it surfaces, the path was never seen gone.
#[tokio::test]
async fn drop_path_settled_whose_server_vanishes_mid_poll_never_reports_settled() {
    let key = RpcKey::from_hex(KEY_HEX).expect("hex key");
    let (endpoint, server) =
        canned_server(vec![Value::from(true), Value::from(INTERFACE_NAME)]).await;
    let started = Instant::now();
    let result =
        drop_path_settled(&endpoint, &key, &destination(), Duration::from_millis(700)).await;
    let elapsed = started.elapsed();
    eprintln!("drop_path_settled with a vanished server: {result:?} in {elapsed:?}");
    assert!(result.is_err(), "a poll that never read None reported {result:?}");
    assert!(elapsed < Duration::from_millis(1200), "{elapsed:?} is more than one budget");
    let requests = server.await.expect("server task");
    assert_eq!(requests.len(), 2, "{requests:?}");
}
