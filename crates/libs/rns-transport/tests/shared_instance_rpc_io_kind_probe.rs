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
//! README describes, without Python), and a unix path nobody listens on; then the
//! bytes-wrong edges a consumer meets from outside the crate (an oversized, a wide, a
//! negative and an empty frame header, all `Protocol` with no io kind), a server that
//! resets instead of closing before its first byte, and a unix socket file that exists
//! with nobody behind it (`Connect` of kind `ConnectionRefused`); then the stage
//! boundaries a retrying caller must not confuse: a header left half-written is
//! `Timeout` (not `Io`), a verdict other than `#WELCOME#` after a correct answer is
//! `Authentication` (CPython's `answer_challenge` parity, not `Io`), an oversized reply
//! header after an honest handshake is `Protocol`, a well-framed reply that is not the
//! stats shape is `Decode` (none with an io kind), and an honest handshake followed by a
//! reset instead of a reply is `Io` of a retryable kind, the Rust twin of CPython `honest_rst`.

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

/// Serve the stdlib's mutual handshake honestly (`deliver_challenge`, then
/// `answer_challenge`) and return the request frame the client sends next.
async fn serve_honest_handshake(stream: &mut TcpStream) -> Vec<u8> {
    let authkey = authkey();
    // deliver_challenge: our challenge, the client's answer, our welcome.
    let message = server_challenge();
    let mut challenge = CHALLENGE.to_vec();
    challenge.extend_from_slice(&message);
    stream.write_all(&frame(&challenge)).await.expect("write challenge");
    let response = recv_frame(stream).await.expect("the client's answer");
    assert_eq!(response, answer(&authkey, &message), "the client's answer to our challenge");
    stream.write_all(&frame(WELCOME)).await.expect("write welcome");
    // answer_challenge: the client's challenge, our answer, the client's welcome.
    let theirs = recv_frame(stream).await.expect("the client's challenge");
    assert!(theirs.starts_with(CHALLENGE), "{theirs:?}");
    let ours = answer(&authkey, &theirs[CHALLENGE.len()..]);
    stream.write_all(&frame(&ours)).await.expect("write our answer");
    let welcome = recv_frame(stream).await.expect("the client's verdict");
    assert_eq!(welcome, WELCOME, "the client rejected a correct answer");
    // The request.
    let request = recv_frame(stream).await.expect("the request");
    assert!(!request.is_empty());
    request
}

/// An honest server: both challenges pass, the request is read, then a plain FIN instead
/// of a reply. The reply read meets EOF, so the call is `Io` of kind `UnexpectedEof`,
/// the FIN twin of the CPython `honest_rst` pin.
#[tokio::test]
async fn honest_handshake_then_fin_instead_of_a_reply_is_io_unexpected_eof() {
    let (listener, endpoint) = loopback_listener().await;
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("accept");
        // The whole handshake and the request, then FIN without a reply.
        serve_honest_handshake(&mut stream).await;
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

/// The rnsd `rpc_loop` shape without Python: a listener at the stdlib default `backlog=1`
/// that never accepts, four concurrent dials under a short budget. Whatever the kernel
/// does with the overflow (queue it, drop the SYN, or reset it at `connect()` as macOS
/// does), a consumer must be able to classify every outcome by `Timeout` or `io_kind()`
/// alone: never `Protocol`, never `Authentication`, never `ConnectionRefused`. Not on
/// Windows: Winsock answers a SYN past the backlog with a RST, which the dial reads as
/// `ConnectionRefused`, a platform rendering of the overflow rather than a client defect.
#[cfg(not(windows))]
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

/// A listener that writes `bytes` to the first client and then holds the socket until
/// the client leaves, so the client sees exactly those bytes and nothing else.
async fn server_writing_then_holding(bytes: Vec<u8>) -> (RpcEndpoint, tokio::task::JoinHandle<()>) {
    let (listener, endpoint) = loopback_listener().await;
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("accept");
        stream.write_all(&bytes).await.expect("write");
        stream.flush().await.expect("flush");
        let mut sink = [0u8; 256];
        while let Ok(n) = stream.read(&mut sink).await {
            if n == 0 {
                break;
            }
        }
    });
    (endpoint, server)
}

/// The README's `Protocol`: the bytes were wrong, no io kind, no retry, and the verdict is
/// quick (a bad length must be rejected on the header, not waited out to the budget).
fn assert_protocol_without_io_kind(result: &Result<impl std::fmt::Debug, RpcError>, what: &str) {
    match result {
        Err(err @ RpcError::Protocol(_)) => {
            assert_eq!(err.io_kind(), None, "{what}: {err:?}");
            assert!(!retryable(err), "{what}: {err:?}");
        }
        other => panic!("{what}: expected RpcError::Protocol, got {other:?}"),
    }
}

/// A frame header announcing `i32::MAX` bytes is an oversized frame: `Protocol`, no io
/// kind, rejected before any payload read, so well inside the budget.
#[tokio::test]
async fn oversized_frame_header_is_protocol_without_an_io_kind() {
    let (endpoint, server) = server_writing_then_holding(i32::MAX.to_be_bytes().to_vec()).await;
    let started = Instant::now();
    let result = interface_stats(&endpoint, &key(), WAIT).await;
    assert_protocol_without_io_kind(&result, "oversized header");
    assert!(started.elapsed() < Duration::from_secs(2), "{:?}", started.elapsed());
    server.await.expect("server task");
}

/// A negative frame header other than the stdlib's `-1` wide marker
/// (`connection.py _recv_bytes`: `-1` means "an 8-byte `!Q` length follows") is a bad
/// length: `Protocol`, no io kind.
#[tokio::test]
async fn negative_frame_header_is_protocol_without_an_io_kind() {
    let (endpoint, server) = server_writing_then_holding((-2i32).to_be_bytes().to_vec()).await;
    let started = Instant::now();
    let result = interface_stats(&endpoint, &key(), WAIT).await;
    assert_protocol_without_io_kind(&result, "negative header");
    assert!(started.elapsed() < Duration::from_secs(2), "{:?}", started.elapsed());
    server.await.expect("server task");
}

/// The wide form of an oversized frame: the `-1` marker followed by a `u64::MAX` length.
/// Still `Protocol`, no io kind, rejected on the header.
#[tokio::test]
async fn oversized_wide_frame_header_is_protocol_without_an_io_kind() {
    let mut bytes = (-1i32).to_be_bytes().to_vec();
    bytes.extend_from_slice(&u64::MAX.to_be_bytes());
    let (endpoint, server) = server_writing_then_holding(bytes).await;
    let started = Instant::now();
    let result = interface_stats(&endpoint, &key(), WAIT).await;
    assert_protocol_without_io_kind(&result, "oversized wide header");
    assert!(started.elapsed() < Duration::from_secs(2), "{:?}", started.elapsed());
    server.await.expect("server task");
}

/// An empty frame where the challenge should be is an unexpected handshake token:
/// `Protocol`, no io kind.
#[tokio::test]
async fn empty_challenge_frame_is_protocol_without_an_io_kind() {
    let (endpoint, server) = server_writing_then_holding(frame(b"")).await;
    let started = Instant::now();
    let result = interface_stats(&endpoint, &key(), WAIT).await;
    assert_protocol_without_io_kind(&result, "empty challenge frame");
    assert!(started.elapsed() < Duration::from_secs(2), "{:?}", started.elapsed());
    server.await.expect("server task");
}

/// A server that accepts and then closes mid-handshake, here with a reset (`SO_LINGER 0`)
/// rather than a FIN, before it has written a byte. Usually the client's first read meets
/// the reset or EOF and the call is `Io`; on macOS the reset can land in the same reactor
/// tick as the accept, before the dial's `connect()` has returned, and the call is then
/// `Connect` of kind `ConnectionReset`. A consumer classifies by `io_kind()`, so either
/// variant is admitted as long as the kind is in the retry set: never `Protocol`, never
/// `Authentication`, never `ConnectionRefused`.
#[tokio::test]
async fn accept_then_reset_before_any_byte_is_a_retryable_kind_on_either_variant() {
    let (listener, endpoint) = loopback_listener().await;
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("accept");
        socket2::SockRef::from(&stream).set_linger(Some(Duration::ZERO)).expect("linger zero");
        drop(stream);
    });
    let started = Instant::now();
    let result = interface_stats(&endpoint, &key(), WAIT).await;
    let err = result.as_ref().expect_err("the server never replies");
    assert!(matches!(err, RpcError::Io(_) | RpcError::Connect(_)), "accept then RST: {err:?}");
    assert!(retryable(err), "accept then RST: not classifiable by the README rule: {err:?}");
    assert!(
        matches!(
            err.io_kind(),
            Some(std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::UnexpectedEof)
        ),
        "accept then RST: {err:?}"
    );
    eprintln!("accept then RST: {err:?}");
    assert!(started.elapsed() < Duration::from_secs(2), "{:?}", started.elapsed());
    server.await.expect("server task");
}

/// A unix socket file that exists with no listener behind it: the dial fails, `Connect`,
/// and `io_kind()` reads `ConnectionRefused` just as on a closed TCP port, so the
/// not-yet-listening case is told apart from a retryable kind on either endpoint type.
#[cfg(unix)]
#[tokio::test]
async fn unix_socket_file_with_no_listener_is_connect_connection_refused() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("stale.sock");
    // Bind, then drop the listener: the file stays, nobody accepts.
    let listener = tokio::net::UnixListener::bind(&path).expect("bind unix");
    drop(listener);
    assert!(path.exists(), "the socket file must survive the listener");
    let endpoint = RpcEndpoint::Unix(LocalUnixEndpoint::filesystem(path));
    let started = Instant::now();
    let result = interface_stats(&endpoint, &key(), WAIT).await;
    match &result {
        Err(err @ RpcError::Connect(_)) => {
            assert_eq!(err.io_kind(), Some(std::io::ErrorKind::ConnectionRefused), "{err:?}");
            assert!(!retryable(err), "{err:?}");
        }
        other => panic!("expected Connect, got {other:?}"),
    }
    assert!(started.elapsed() < Duration::from_secs(2), "{:?}", started.elapsed());
}

/// A header left half-written and then silence: the read is short but still pending, so
/// the budget decides, not the stream. `Timeout`, no io kind, worth a retry by variant.
/// A caller must not see this as `Io(UnexpectedEof)` (the stream never ended) nor as
/// `Protocol` (no bytes were wrong).
#[tokio::test]
async fn partial_header_then_silence_is_timeout_without_an_io_kind() {
    let (endpoint, server) = server_writing_then_holding(vec![0x00, 0x00]).await;
    let budget = Duration::from_millis(700);
    let started = Instant::now();
    let result = interface_stats(&endpoint, &key(), budget).await;
    let elapsed = started.elapsed();
    match &result {
        Err(err @ RpcError::Timeout) => {
            assert_eq!(err.io_kind(), None, "{err:?}");
            assert!(retryable(err), "{err:?}");
        }
        other => panic!("partial header then silence: expected Timeout, got {other:?}"),
    }
    assert!(elapsed >= Duration::from_millis(600), "{elapsed:?} returned before the budget");
    assert!(elapsed < Duration::from_secs(4), "{elapsed:?} overran the budget");
    server.await.expect("server task");
}

/// A correct answer met with a verdict that is neither `#WELCOME#` nor `#FAILURE#`.
/// CPython's `answer_challenge` raises `AuthenticationError` for any response other
/// than `#WELCOME#` (`connection.py`, "digest sent was rejected"), and so does the
/// client here: `Authentication`, no io kind, not a retry. The point for a retrying
/// caller is that it is not `Io`: no io failure happened, the server simply said no.
#[tokio::test]
async fn non_welcome_verdict_after_a_correct_answer_is_authentication_without_an_io_kind() {
    let (listener, endpoint) = loopback_listener().await;
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("accept");
        let message = server_challenge();
        let mut challenge = CHALLENGE.to_vec();
        challenge.extend_from_slice(&message);
        stream.write_all(&frame(&challenge)).await.expect("write challenge");
        let response = recv_frame(&mut stream).await.expect("the client's answer");
        assert_eq!(response, answer(&authkey(), &message), "the client's answer");
        stream.write_all(&frame(b"#NOPE#")).await.expect("write the wrong token");
        stream.flush().await.expect("flush");
        let mut sink = [0u8; 256];
        while let Ok(n) = stream.read(&mut sink).await {
            if n == 0 {
                break;
            }
        }
    });
    let started = Instant::now();
    let result = interface_stats(&endpoint, &key(), WAIT).await;
    match &result {
        Err(err @ RpcError::Authentication) => {
            assert_eq!(err.io_kind(), None, "{err:?}");
            assert!(!retryable(err), "{err:?}");
        }
        other => panic!("non-WELCOME verdict: expected Authentication, got {other:?}"),
    }
    assert!(started.elapsed() < Duration::from_secs(2), "{:?}", started.elapsed());
    server.await.expect("server task");
}

/// An honest handshake, the request read, then a reply header announcing `i32::MAX`
/// bytes: the cap rejects the reply on its header. Bytes wrong at the reply stage are
/// `Protocol` with no io kind, exactly as at the challenge stage, and quick.
#[tokio::test]
async fn honest_handshake_then_oversized_reply_header_is_protocol_without_an_io_kind() {
    let (listener, endpoint) = loopback_listener().await;
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("accept");
        serve_honest_handshake(&mut stream).await;
        stream.write_all(&i32::MAX.to_be_bytes()).await.expect("write the oversized header");
        stream.flush().await.expect("flush");
        let mut sink = [0u8; 256];
        while let Ok(n) = stream.read(&mut sink).await {
            if n == 0 {
                break;
            }
        }
    });
    let started = Instant::now();
    let result = interface_stats(&endpoint, &key(), WAIT).await;
    assert_protocol_without_io_kind(&result, "oversized reply header after an honest handshake");
    assert!(started.elapsed() < Duration::from_secs(2), "{:?}", started.elapsed());
    server.await.expect("server task");
}

/// An honest handshake, the request read, then `SO_LINGER 0` instead of a reply: the
/// Rust twin of the CPython `honest_rst` pin. The reply read meets the reset (or, if the
/// FIN raced ahead, EOF); either way `Io` of a retryable kind, never `Protocol` or
/// `Authentication`.
#[tokio::test]
async fn honest_handshake_then_reset_instead_of_a_reply_is_io_retryable() {
    let (listener, endpoint) = loopback_listener().await;
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("accept");
        serve_honest_handshake(&mut stream).await;
        socket2::SockRef::from(&stream).set_linger(Some(Duration::ZERO)).expect("linger zero");
        drop(stream);
    });
    let started = Instant::now();
    let result = interface_stats(&endpoint, &key(), WAIT).await;
    assert_io_retryable(&result, "honest handshake then RST");
    let kind = result.as_ref().err().and_then(RpcError::io_kind);
    eprintln!("honest handshake then RST: {kind:?}");
    assert!(
        matches!(
            kind,
            Some(std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::UnexpectedEof)
        ),
        "{result:?}"
    );
    assert!(started.elapsed() < Duration::from_secs(2), "{:?}", started.elapsed());
    server.await.expect("server task");
}

/// An honest handshake and a well-framed reply whose payload is not the stats shape: the
/// README's `Decode`, with no io kind, so a retrying caller reports it rather than
/// dialing again.
#[tokio::test]
async fn honest_handshake_then_a_reply_of_the_wrong_shape_is_decode_without_an_io_kind() {
    let (listener, endpoint) = loopback_listener().await;
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("accept");
        serve_honest_handshake(&mut stream).await;
        // A valid frame around a msgpack string, not the interface_stats map.
        let mut payload = vec![0xa5u8];
        payload.extend_from_slice(b"oops!");
        stream.write_all(&frame(&payload)).await.expect("write the wrong-shaped reply");
        stream.flush().await.expect("flush");
        let mut sink = [0u8; 256];
        while let Ok(n) = stream.read(&mut sink).await {
            if n == 0 {
                break;
            }
        }
    });
    let started = Instant::now();
    let result = interface_stats(&endpoint, &key(), WAIT).await;
    match &result {
        Err(err @ RpcError::Decode(_)) => {
            assert_eq!(err.io_kind(), None, "{err:?}");
            assert!(!retryable(err), "{err:?}");
        }
        other => panic!("wrong-shaped reply: expected Decode, got {other:?}"),
    }
    assert!(started.elapsed() < Duration::from_secs(2), "{:?}", started.elapsed());
    server.await.expect("server task");
}
