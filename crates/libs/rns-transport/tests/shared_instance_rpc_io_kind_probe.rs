//! `RpcError::Io` and `RpcError::io_kind()` held to the README's promise, written from
//! that promise and not from the implementation: every io failure after the dial
//! succeeded is `Io`, `Protocol` means only that the bytes were wrong, `io_kind()` reads
//! the kind behind `Io` and `Connect` and is `None` otherwise, and the kinds worth a retry
//! are `ConnectionReset`, `ConnectionAborted`, `BrokenPipe` and `UnexpectedEof`, beside
//! `Timeout`. The departed servers here are the ones the shipped pins do not model from
//! outside the crate: a server that sends a valid challenge and hangs up without reading
//! the answer (no wrong answer, so the `#FAILURE#` carve-out must not fire), an honest
//! server that completes the whole handshake and reads the request but hangs up with a
//! FIN instead of a reply, the same accept-then-close over a unix socket, a `backlog=1`
//! listener that never accepts under four concurrent dials (the rnsd `rpc_loop` shape the
//! README describes, without Python), and a unix path nobody listens on.

use std::error::Error;
use std::time::{Duration, Instant};

use hmac::{Hmac, Mac};
use sha2::Sha256;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

#[cfg(unix)]
use rns_transport::iface::local::LocalUnixEndpoint;
use rns_transport::transport::shared_instance_rpc::{
    interface_stats, RpcEndpoint, RpcError, RpcKey,
};

const WAIT: Duration = Duration::from_secs(5);
const KEY_HEX: &str = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";

const CHALLENGE: &[u8] = b"#CHALLENGE#";
const WELCOME: &[u8] = b"#WELCOME#";

/// The README's retry rule as a consumer writes it.
fn retryable(err: &RpcError) -> bool {
    use std::io::ErrorKind::{BrokenPipe, ConnectionAborted, ConnectionReset, UnexpectedEof};
    matches!(err, RpcError::Timeout)
        || matches!(
            err.io_kind(),
            Some(ConnectionReset | ConnectionAborted | BrokenPipe | UnexpectedEof)
        )
}

fn key() -> RpcKey {
    RpcKey::from_hex(KEY_HEX).expect("hex key")
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

fn authkey() -> Vec<u8> {
    (0..KEY_HEX.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&KEY_HEX[i..i + 2], 16).expect("hex byte"))
        .collect()
}

async fn loopback_listener() -> (TcpListener, RpcEndpoint) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind loopback");
    let endpoint = RpcEndpoint::Tcp(listener.local_addr().expect("listener address"));
    (listener, endpoint)
}

fn assert_io_retryable(result: &Result<impl std::fmt::Debug, RpcError>, what: &str) {
    match result {
        Err(err @ RpcError::Io(io)) => {
            assert!(retryable(err), "{what}: Io of a kind outside the retry set: {err:?}");
            assert_eq!(
                err.io_kind(),
                Some(io.kind()),
                "{what}: io_kind() disagrees with the source"
            );
            let source = err.source().expect("Io carries its io::Error as source");
            assert!(source.is::<std::io::Error>(), "{what}: source is not an io::Error: {source}");
            assert!(err.to_string().contains("io error after connect"), "{what}: {err}");
        }
        other => panic!("{what}: expected RpcError::Io, got {other:?}"),
    }
}

/// A server that delivers a valid challenge and hangs up without reading the answer. No
/// wrong answer was ever given, so this is a departed server, not a rejected key: the
/// call is `Io` of a retryable kind, never `Authentication` or `Protocol`.
#[tokio::test]
async fn valid_challenge_then_fin_without_reading_is_io_retryable() {
    let (listener, endpoint) = loopback_listener().await;
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("accept");
        let mut challenge = CHALLENGE.to_vec();
        challenge.extend_from_slice(&server_challenge());
        stream.write_all(&frame(&challenge)).await.expect("write challenge");
        stream.flush().await.expect("flush");
        stream.shutdown().await.expect("fin");
        drop(stream);
    });
    let started = Instant::now();
    let result = interface_stats(&endpoint, &key(), WAIT).await;
    assert_io_retryable(&result, "valid challenge then FIN");
    eprintln!("valid challenge then FIN: {:?}", result.as_ref().err().and_then(RpcError::io_kind));
    assert!(started.elapsed() < WAIT, "{:?} ran to the budget", started.elapsed());
    server.await.expect("server task");
}

/// An honest server: both challenges pass, the request is read, then a plain FIN instead
/// of a reply. The reply read meets EOF, so the call is `Io` of kind `UnexpectedEof`,
/// the FIN twin of the CPython `honest_rst` pin.
#[tokio::test]
async fn honest_handshake_then_fin_instead_of_a_reply_is_io_unexpected_eof() {
    let (listener, endpoint) = loopback_listener().await;
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("accept");
        let authkey = authkey();
        // deliver_challenge: our challenge, the client's answer, our welcome.
        let message = server_challenge();
        let mut challenge = CHALLENGE.to_vec();
        challenge.extend_from_slice(&message);
        stream.write_all(&frame(&challenge)).await.expect("write challenge");
        let response = recv_frame(&mut stream).await.expect("the client's answer");
        assert_eq!(response, answer(&authkey, &message), "the client's answer to our challenge");
        stream.write_all(&frame(WELCOME)).await.expect("write welcome");
        // answer_challenge: the client's challenge, our answer, the client's welcome.
        let theirs = recv_frame(&mut stream).await.expect("the client's challenge");
        assert!(theirs.starts_with(CHALLENGE), "{theirs:?}");
        let ours = answer(&authkey, &theirs[CHALLENGE.len()..]);
        stream.write_all(&frame(&ours)).await.expect("write our answer");
        let welcome = recv_frame(&mut stream).await.expect("the client's verdict");
        assert_eq!(welcome, WELCOME, "the client rejected a correct answer");
        // The request, then FIN without a reply.
        let request = recv_frame(&mut stream).await.expect("the request");
        assert!(!request.is_empty());
        stream.shutdown().await.expect("fin");
        drop(stream);
    });
    let result = interface_stats(&endpoint, &key(), WAIT).await;
    assert_io_retryable(&result, "honest handshake then FIN");
    assert_eq!(
        result.as_ref().err().and_then(RpcError::io_kind),
        Some(std::io::ErrorKind::UnexpectedEof),
        "{result:?}"
    );
    server.await.expect("server task");
}

/// The same accept-then-close over a unix socket: the variant does not depend on the
/// endpoint kind.
#[cfg(unix)]
#[tokio::test]
async fn unix_server_that_closes_at_once_is_io_unexpected_eof() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("rpc.sock");
    let listener = tokio::net::UnixListener::bind(&path).expect("bind unix");
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("accept");
        stream.shutdown().await.expect("shutdown at once");
    });
    let endpoint = RpcEndpoint::Unix(LocalUnixEndpoint::filesystem(path));
    let result = interface_stats(&endpoint, &key(), WAIT).await;
    match &result {
        Err(err @ RpcError::Io(_)) => {
            assert_eq!(err.io_kind(), Some(std::io::ErrorKind::UnexpectedEof), "{err:?}");
            assert!(retryable(err));
        }
        other => panic!("expected Io over unix, got {other:?}"),
    }
    server.await.expect("server task");
}

/// A unix path nobody listens on is the dial failing, `Connect`, and `io_kind()` reads
/// its kind (`NotFound` on a missing path) so a consumer sees it is outside the retry set
/// without reading the text.
#[cfg(unix)]
#[tokio::test]
async fn missing_unix_path_is_connect_with_a_non_retryable_io_kind() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("nobody-listens.sock");
    let endpoint = RpcEndpoint::Unix(LocalUnixEndpoint::filesystem(path));
    let result = interface_stats(&endpoint, &key(), WAIT).await;
    match &result {
        Err(err @ RpcError::Connect(_)) => {
            let kind = err.io_kind().expect("Connect carries an io kind");
            assert_eq!(kind, std::io::ErrorKind::NotFound, "{err:?}");
            assert!(!retryable(err), "{err:?}");
        }
        other => panic!("expected Connect, got {other:?}"),
    }
}

/// The rnsd `rpc_loop` shape without Python: a `backlog=1` listener that never accepts,
/// four concurrent dials under a short budget. Whatever the kernel does with the overflow
/// (queue it, drop the SYN, or reset it at `connect()` as macOS does), a consumer must be
/// able to classify every outcome by `Timeout` or `io_kind()` alone: never `Protocol`,
/// never `Authentication`, never `ConnectionRefused`.
#[tokio::test]
async fn backlog_one_listener_overflow_is_timeout_or_a_retryable_kind_on_either_variant() {
    use socket2::{Domain, Protocol, Socket, Type};
    let socket = Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP)).expect("socket");
    socket
        .bind(&"127.0.0.1:0".parse::<std::net::SocketAddr>().expect("addr").into())
        .expect("bind");
    socket.listen(1).expect("listen(1)");
    let addr = socket.local_addr().expect("local addr").as_socket().expect("inet addr");
    let endpoint = RpcEndpoint::Tcp(addr);
    let key = key();
    let budget = Duration::from_millis(1500);
    let results = tokio::join!(
        interface_stats(&endpoint, &key, budget),
        interface_stats(&endpoint, &key, budget),
        interface_stats(&endpoint, &key, budget),
        interface_stats(&endpoint, &key, budget),
    );
    let results = [results.0, results.1, results.2, results.3];
    let mut kinds = Vec::new();
    for (i, result) in results.iter().enumerate() {
        let err = result.as_ref().expect_err("nobody accepts, so nobody replies");
        assert!(retryable(err), "dial {i}: not classifiable by the README rule: {err:?}");
        assert!(
            !matches!(err, RpcError::Protocol(_) | RpcError::Authentication),
            "dial {i}: {err:?}"
        );
        assert_ne!(err.io_kind(), Some(std::io::ErrorKind::ConnectionRefused), "dial {i}: {err:?}");
        kinds.push(match err {
            RpcError::Timeout => "Timeout".to_string(),
            RpcError::Connect(_) => format!("Connect({:?})", err.io_kind()),
            RpcError::Io(_) => format!("Io({:?})", err.io_kind()),
            other => format!("{other:?}"),
        });
    }
    eprintln!("backlog=1 overflow outcomes: {kinds:?}");
    drop(socket);
}
