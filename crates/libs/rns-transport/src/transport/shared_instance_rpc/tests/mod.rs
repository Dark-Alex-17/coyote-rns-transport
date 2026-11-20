use std::io::Write;
use std::time::{Duration, Instant};

use rmpv::Value;

use super::framing::{recv_frame, send_frame};
use super::handshake::{deliver_challenge, mac, verify_mac, ChallengeForm, Digest};
use super::*;

mod mock_server;
use mock_server::{Answer, MockServer, Reply, Script, ServerForm};

const WAIT: Duration = Duration::from_secs(5);
const KEY_HEX: &str = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";

fn key() -> RpcKey {
    RpcKey::from_hex(KEY_HEX).expect("test key")
}

fn map(pairs: Vec<(&str, Value)>) -> Value {
    Value::Map(pairs.into_iter().map(|(key, value)| (Value::from(key), value)).collect())
}

fn pack(value: &Value) -> Vec<u8> {
    let mut packed = Vec::new();
    rmpv::encode::write_value(&mut packed, value).expect("pack");
    packed
}

/// Three interfaces the way `get_interface_stats` emits them
/// (`RNS/Reticulum.py:1409-1410`, `:1501-1503`, `:1527-1566`), plus the
/// top-level totals.
fn canned_reply() -> Value {
    let auto = map(vec![
        ("name", Value::from("AutoInterface[Default Interface]")),
        ("short_name", Value::from("Default Interface")),
        ("hash", Value::Binary(vec![0xab; 16])),
        ("type", Value::from("AutoInterface")),
        ("mtu", Value::from(1064)),
        ("rxb", Value::from(1200)),
        ("txb", Value::from(340)),
        ("status", Value::from(true)),
        ("mode", Value::from(1)),
        ("clients", Value::Nil),
        ("peers", Value::from(0)),
    ]);
    let tcp_server = map(vec![
        ("name", Value::from("TCPServerInterface[Backbone]")),
        ("short_name", Value::from("Backbone")),
        ("hash", Value::Binary(vec![0xcd; 16])),
        ("type", Value::from("TCPServerInterface")),
        ("mtu", Value::from(1064)),
        ("rxb", Value::from(99)),
        ("txb", Value::from(7)),
        ("status", Value::from(true)),
        ("mode", Value::from(1)),
        ("clients", Value::from(3)),
    ]);
    let nil_peers = map(vec![
        ("name", Value::from("UDPInterface[Lan]")),
        ("short_name", Value::from("Lan")),
        ("hash", Value::Binary(vec![0xef; 16])),
        ("type", Value::from("UDPInterface")),
        ("mtu", Value::from(1064)),
        ("rxb", Value::from(0)),
        ("txb", Value::from(0)),
        ("status", Value::from(false)),
        ("mode", Value::from(1)),
        ("clients", Value::Nil),
        ("peers", Value::Nil),
    ]);
    map(vec![
        ("interfaces", Value::Array(vec![auto, tcp_server, nil_peers])),
        ("rxb", Value::from(1299)),
        ("txb", Value::from(347)),
        ("rxs", Value::from(12)),
        ("txs", Value::from(3)),
    ])
}

fn exchange(form: ServerForm, answer: Answer) -> Script {
    Script::Exchange {
        form,
        key: key().as_bytes().to_vec(),
        answer,
        reply: Reply::Frame(pack(&canned_reply())),
    }
}

async fn call(server: &MockServer) -> Result<InterfaceStats, RpcError> {
    interface_stats(&RpcEndpoint::Tcp(server.addr()), &key(), WAIT).await
}

fn assert_decoded(stats: &InterfaceStats) {
    assert_eq!(stats.interfaces.len(), 3);
    let auto = &stats.interfaces[0];
    assert_eq!(auto.kind, "AutoInterface");
    assert_eq!(auto.short_name, "Default Interface");
    assert!(auto.online);
    assert_eq!(auto.peers, Some(0));
    assert_eq!(auto.clients, None);
    assert_eq!((auto.rxb, auto.txb), (1200, 340));
    assert_eq!(
        auto.raw.as_map().and_then(|map| map.iter().find(|(key, _)| key.as_str() == Some("hash"))),
        Some(&(Value::from("hash"), Value::Binary(vec![0xab; 16])))
    );
    let tcp_server = &stats.interfaces[1];
    assert_eq!(tcp_server.peers, None);
    assert_eq!(tcp_server.clients, Some(3));
    let nil_peers = &stats.interfaces[2];
    assert_eq!(nil_peers.peers, None);
    assert!(!nil_peers.online);
    assert_eq!(stats.raw, canned_reply());
    assert!(stats.raw.as_map().expect("map").iter().any(|(key, _)| key.as_str() == Some("rxs")));
}

fn assert_request(observed: &mock_server::Observed) {
    let request = observed.request.as_ref().expect("request frame");
    let value = rmpv::decode::read_value(&mut request.as_slice()).expect("request decodes");
    assert_eq!(value, map(vec![("get", Value::from("interface_stats"))]));
}

#[tokio::test]
async fn modern_sha256_server_round_trip() {
    let server =
        MockServer::spawn(exchange(ServerForm::Modern(Digest::Sha256), Answer::Correct)).await;
    let stats = call(&server).await.expect("interface_stats");
    assert_decoded(&stats);
    let observed = server.finish().await;
    let answer = observed.answer.as_ref().expect("answer");
    assert!(answer.starts_with(b"{sha256}"), "answer {answer:?}");
    assert_eq!(answer.len(), "{sha256}".len() + 32);
    let challenge = observed.challenge.as_ref().expect("challenge");
    assert!(challenge.starts_with(b"#CHALLENGE#{sha256}"), "challenge {challenge:?}");
    assert_eq!(challenge.len(), "#CHALLENGE#{sha256}".len() + 40);
    assert_eq!(observed.verdict.as_deref(), Some(b"#WELCOME#".as_slice()));
    assert_request(&observed);
}

#[tokio::test]
async fn legacy_md5_server_round_trip() {
    let server = MockServer::spawn(exchange(ServerForm::Legacy, Answer::Correct)).await;
    let stats = call(&server).await.expect("interface_stats");
    assert_decoded(&stats);
    let observed = server.finish().await;
    let answer = observed.answer.as_ref().expect("answer");
    assert_eq!(answer.len(), 16, "a legacy answer is the bare HMAC-MD5: {answer:?}");
    let challenge = observed.challenge.as_ref().expect("challenge");
    let body = challenge.strip_prefix(b"#CHALLENGE#").expect("challenge prefix");
    assert_eq!(body.len(), 20, "a legacy challenge is 20 bare bytes: {body:?}");
    assert_eq!(observed.verdict.as_deref(), Some(b"#WELCOME#".as_slice()));
    assert_request(&observed);
}

#[cfg(unix)]
#[tokio::test]
async fn filesystem_unix_socket_round_trip() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("rpc");
    let server = MockServer::spawn_unix(
        &path,
        exchange(ServerForm::Modern(Digest::Sha256), Answer::Correct),
    );
    let endpoint = RpcEndpoint::Unix(LocalUnixEndpoint::Filesystem(path));
    let stats = interface_stats(&endpoint, &key(), WAIT).await.expect("interface_stats");
    assert_decoded(&stats);
    assert_request(&server.finish().await);
}

#[cfg(any(target_os = "linux", target_os = "android"))]
#[tokio::test]
async fn abstract_unix_socket_round_trip() {
    let unix = LocalUnixEndpoint::abstract_name(format!("coyote-rpc-test-{}", std::process::id()));
    let server = MockServer::spawn_unix(
        &unix.tokio_path(),
        exchange(ServerForm::Modern(Digest::Sha256), Answer::Correct),
    );
    let stats =
        interface_stats(&RpcEndpoint::Unix(unix), &key(), WAIT).await.expect("interface_stats");
    assert_decoded(&stats);
    assert_request(&server.finish().await);
}

#[tokio::test]
async fn modern_sha3_384_server_round_trip() {
    let server =
        MockServer::spawn(exchange(ServerForm::Modern(Digest::Sha3_384), Answer::Correct)).await;
    let stats = call(&server).await.expect("interface_stats");
    assert_decoded(&stats);
    let observed = server.finish().await;
    let answer = observed.answer.as_ref().expect("answer");
    assert!(answer.starts_with(b"{sha3_384}"), "answer {answer:?}");
    assert_eq!(answer.len(), "{sha3_384}".len() + 48);
    let challenge = observed.challenge.as_ref().expect("challenge");
    assert!(challenge.starts_with(b"#CHALLENGE#{sha256}"), "our challenge stays sha256");
}

#[tokio::test]
async fn modern_md5_server_gets_a_prefixed_md5_answer() {
    let server =
        MockServer::spawn(exchange(ServerForm::Modern(Digest::Md5), Answer::Correct)).await;
    call(&server).await.expect("interface_stats");
    let observed = server.finish().await;
    let answer = observed.answer.as_ref().expect("answer");
    assert!(answer.starts_with(b"{md5}"), "answer {answer:?}");
    assert_eq!(answer.len(), "{md5}".len() + 16);
}

#[tokio::test]
async fn wrong_key_is_authentication() {
    let server = MockServer::spawn(Script::Exchange {
        form: ServerForm::Modern(Digest::Sha256),
        key: vec![0x42; 32],
        answer: Answer::Correct,
        reply: Reply::Frame(pack(&canned_reply())),
    })
    .await;
    assert!(matches!(call(&server).await, Err(RpcError::Authentication)));
    let observed = server.finish().await;
    assert_eq!(observed.after_failure, Some(true), "the client hangs up at #FAILURE#");
}

#[tokio::test]
async fn bad_server_answer_is_authentication_and_sends_failure() {
    let server =
        MockServer::spawn(exchange(ServerForm::Modern(Digest::Sha256), Answer::Wrong)).await;
    assert!(matches!(call(&server).await, Err(RpcError::Authentication)));
    let observed = server.finish().await;
    assert_eq!(observed.verdict.as_deref(), Some(b"#FAILURE#".as_slice()));
    assert_eq!(observed.after_failure, Some(true), "the client hangs up after #FAILURE#");
}

#[tokio::test]
async fn legacy_server_with_bad_answer_is_authentication() {
    let server = MockServer::spawn(exchange(ServerForm::Legacy, Answer::Wrong)).await;
    assert!(matches!(call(&server).await, Err(RpcError::Authentication)));
    let observed = server.finish().await;
    assert_eq!(observed.verdict.as_deref(), Some(b"#FAILURE#".as_slice()));
    assert_eq!(observed.after_failure, Some(true), "the client hangs up after #FAILURE#");
}

#[tokio::test]
async fn server_answer_in_another_digest_is_authentication() {
    let server = MockServer::spawn(exchange(
        ServerForm::Modern(Digest::Sha256),
        Answer::OtherDigest(Digest::Sha384),
    ))
    .await;
    assert!(matches!(call(&server).await, Err(RpcError::Authentication)));
    let observed = server.finish().await;
    let answer = observed.answer.as_ref().expect("answer");
    assert!(answer.starts_with(b"{sha256}"), "the client challenged in sha256: {answer:?}");
    assert_eq!(observed.verdict.as_deref(), Some(b"#FAILURE#".as_slice()));
    assert_eq!(observed.after_failure, Some(true), "the client hangs up after #FAILURE#");
}

#[tokio::test]
async fn server_answer_in_legacy_form_to_a_modern_challenge_is_authentication() {
    let server = MockServer::spawn(exchange(
        ServerForm::Modern(Digest::Sha256),
        Answer::OtherDigest(Digest::Md5),
    ))
    .await;
    assert!(matches!(call(&server).await, Err(RpcError::Authentication)));
    let observed = server.finish().await;
    assert_eq!(observed.verdict.as_deref(), Some(b"#FAILURE#".as_slice()));
    assert_eq!(observed.after_failure, Some(true), "the client hangs up after #FAILURE#");
}

/// A server that answers wrong and hangs up before reading the verdict
/// still gets `Authentication` from the client, whatever became of the
/// `#FAILURE#` write to the departed peer.
#[tokio::test]
async fn bad_server_answer_from_a_departed_server_is_still_authentication() {
    let server =
        MockServer::spawn(exchange(ServerForm::Modern(Digest::Sha256), Answer::WrongThenHangUp))
            .await;
    let result = call(&server).await;
    assert!(matches!(result, Err(RpcError::Authentication)), "{result:?}");
    let observed = server.finish().await;
    assert_eq!(observed.verdict, None, "the server never read a verdict");
}

/// The `#FAILURE#` write itself failing does not change the verdict: over an
/// in-memory pipe whose far end has already gone, the write is a broken
/// pipe and `deliver_challenge` still returns `Authentication`.
#[tokio::test(flavor = "current_thread")]
async fn failure_notice_on_a_broken_pipe_is_still_authentication() {
    let (mut client, mut server) = tokio::io::duplex(1024);
    let key = key();
    let client_key = key.clone();
    let client_task = tokio::spawn(async move {
        deliver_challenge(&mut client, client_key.as_bytes(), ChallengeForm::Modern).await
    });
    let challenge = recv_frame(&mut server, 256).await.expect("recv challenge");
    let body = challenge.strip_prefix(b"#CHALLENGE#").expect("challenge prefix");
    let mut answer = b"{sha256}".to_vec();
    answer.extend(mac(Digest::Sha256, key.as_bytes(), body).into_iter().map(|byte| !byte));
    // The answer write completes without yielding, so the client cannot run
    // between it and the drop (on the current-thread test runtime): its
    // #FAILURE# meets a pipe with no reader.
    send_frame(&mut server, &answer).await.expect("send wrong answer");
    drop(server);
    let verdict = client_task.await.expect("client task");
    assert!(matches!(verdict, Err(RpcError::Authentication)), "{verdict:?}");
}

#[tokio::test]
async fn unknown_challenge_digest_is_unsupported_digest() {
    let mut frame = b"#CHALLENGE#{sha1}".to_vec();
    frame.extend_from_slice(&[7u8; 40]);
    let server = MockServer::spawn(Script::Frame(frame)).await;
    match call(&server).await {
        Err(RpcError::UnsupportedDigest(name)) => assert_eq!(name, "sha1"),
        other => panic!("expected UnsupportedDigest, got {other:?}"),
    }
    server.finish().await;
}

/// `_get_digest_name_and_payload` looks for the closing brace with
/// `message.find(b'}', 1, _MAX_DIGEST_LEN + 2)` (`connection.py:906-907`):
/// an eight-letter name closes at index 9, the last index in the window, and
/// is read as a digest name.
#[tokio::test]
async fn eight_letter_challenge_digest_is_read_as_a_name() {
    let mut frame = b"#CHALLENGE#{sha1xxxx}".to_vec();
    frame.extend_from_slice(&[7u8; 40]);
    let server = MockServer::spawn(Script::Frame(frame)).await;
    match call(&server).await {
        Err(RpcError::UnsupportedDigest(name)) => assert_eq!(name, "sha1xxxx"),
        other => panic!("expected UnsupportedDigest, got {other:?}"),
    }
    server.finish().await;
}

/// A nine-letter name closes at index 10, one past the window
/// (`connection.py:906-907`), so there is no digest prefix at all and
/// Python raises `AuthenticationError('unsupported message length')`.
#[tokio::test]
async fn nine_letter_challenge_digest_is_protocol() {
    let mut frame = b"#CHALLENGE#{sha3_3840}".to_vec();
    frame.extend_from_slice(&[7u8; 40]);
    let server = MockServer::spawn(Script::Frame(frame)).await;
    let result = call(&server).await;
    assert!(matches!(result, Err(RpcError::Protocol(_))), "{result:?}");
    server.finish().await;
}

#[tokio::test]
async fn short_challenge_body_is_protocol() {
    let mut frame = b"#CHALLENGE#".to_vec();
    frame.extend_from_slice(&[7u8; 10]);
    let server = MockServer::spawn(Script::Frame(frame)).await;
    assert!(matches!(call(&server).await, Err(RpcError::Protocol(_))));
    server.finish().await;
}

#[tokio::test]
async fn frame_header_is_big_endian() {
    let mut bytes = 57i32.to_be_bytes().to_vec();
    bytes.extend_from_slice(b"#CHALLENGE#{sha1}");
    bytes.extend_from_slice(&[7u8; 40]);
    let server = MockServer::spawn(Script::RawBytes(bytes)).await;
    match call(&server).await {
        Err(RpcError::UnsupportedDigest(name)) => assert_eq!(name, "sha1"),
        other => panic!("expected UnsupportedDigest, got {other:?}"),
    }
    server.finish().await;
}

#[tokio::test]
async fn truncated_frame_is_io_unexpected_eof() {
    let mut bytes = 48i32.to_be_bytes().to_vec();
    bytes.extend_from_slice(b"#CHALLENGE");
    let server = MockServer::spawn(Script::RawBytes(bytes)).await;
    let result = call(&server).await;
    match &result {
        Err(RpcError::Io(err)) => assert_eq!(err.kind(), std::io::ErrorKind::UnexpectedEof),
        other => panic!("expected Io, got {other:?}"),
    }
    assert_eq!(
        result.as_ref().err().and_then(RpcError::io_kind),
        Some(std::io::ErrorKind::UnexpectedEof)
    );
    server.finish().await;
}

#[tokio::test]
async fn server_that_closes_at_once_is_io_unexpected_eof() {
    let server = MockServer::spawn(Script::Close).await;
    let result = call(&server).await;
    match &result {
        Err(RpcError::Io(err)) => assert_eq!(err.kind(), std::io::ErrorKind::UnexpectedEof),
        other => panic!("expected Io, got {other:?}"),
    }
    assert_eq!(
        result.as_ref().err().and_then(RpcError::io_kind),
        Some(std::io::ErrorKind::UnexpectedEof)
    );
    server.finish().await;
}

#[tokio::test]
async fn garbage_and_oversized_frames_are_protocol_without_an_io_kind() {
    // "not " reads as a positive 32-bit length far past the 256-byte auth cap.
    let garbage = b"not a frame at all, just bytes".to_vec();
    for bytes in [300i32.to_be_bytes().to_vec(), garbage] {
        let server = MockServer::spawn(Script::RawBytes(bytes.clone())).await;
        match call(&server).await {
            Err(err @ RpcError::Protocol(_)) => assert_eq!(err.io_kind(), None, "{bytes:?}"),
            other => panic!("expected Protocol for {bytes:?}, got {other:?}"),
        }
        server.finish().await;
    }
}

#[test]
fn io_kind_is_none_for_the_non_io_variants() {
    use std::io::ErrorKind;
    for err in [
        RpcError::Authentication,
        RpcError::Protocol("x".into()),
        RpcError::Decode("x".into()),
        RpcError::UnsupportedDigest("x".into()),
        RpcError::InvalidKeyMaterial,
        RpcError::Timeout,
    ] {
        assert_eq!(err.io_kind(), None, "{err:?}");
    }
    let connect = RpcError::Connect(std::io::Error::from(ErrorKind::ConnectionRefused));
    assert_eq!(connect.io_kind(), Some(ErrorKind::ConnectionRefused));
    let io = RpcError::Io(std::io::Error::from(ErrorKind::ConnectionReset));
    assert_eq!(io.io_kind(), Some(ErrorKind::ConnectionReset));
}

#[tokio::test]
async fn oversized_auth_frame_is_protocol() {
    let server = MockServer::spawn(Script::RawBytes(300i32.to_be_bytes().to_vec())).await;
    assert!(matches!(call(&server).await, Err(RpcError::Protocol(_))));
    server.finish().await;
}

#[tokio::test]
async fn wide_frame_header_carries_a_u64_length() {
    let mut payload = b"#CHALLENGE#{sha1}".to_vec();
    payload.extend_from_slice(&[7u8; 40]);
    let mut bytes = (-1i32).to_be_bytes().to_vec();
    let len = u64::try_from(payload.len()).expect("length");
    bytes.extend_from_slice(&len.to_be_bytes());
    bytes.extend_from_slice(&payload);
    let server = MockServer::spawn(Script::RawBytes(bytes)).await;
    match call(&server).await {
        Err(RpcError::UnsupportedDigest(name)) => assert_eq!(name, "sha1"),
        other => panic!("expected UnsupportedDigest, got {other:?}"),
    }
    server.finish().await;
}

#[tokio::test]
async fn oversized_wide_auth_frame_is_protocol() {
    let mut bytes = (-1i32).to_be_bytes().to_vec();
    bytes.extend_from_slice(&300u64.to_be_bytes());
    let server = MockServer::spawn(Script::RawBytes(bytes)).await;
    assert!(matches!(call(&server).await, Err(RpcError::Protocol(_))));
    server.finish().await;
}

#[tokio::test]
async fn negative_frame_length_is_protocol() {
    let server = MockServer::spawn(Script::RawBytes((-2i32).to_be_bytes().to_vec())).await;
    assert!(matches!(call(&server).await, Err(RpcError::Protocol(_))));
    server.finish().await;
}

#[tokio::test]
async fn first_frame_that_is_not_a_challenge_is_protocol() {
    let server = MockServer::spawn(Script::Frame(b"#WELCOME#".to_vec())).await;
    assert!(matches!(call(&server).await, Err(RpcError::Protocol(_))));
    server.finish().await;
}

#[tokio::test]
async fn challenge_without_digest_prefix_is_protocol() {
    for prefix in [b"#CHALLENGE#".as_slice(), b"#CHALLENGE#{}"] {
        let mut frame = prefix.to_vec();
        frame.extend_from_slice(&[7u8; 30]);
        let server = MockServer::spawn(Script::Frame(frame)).await;
        assert!(
            matches!(call(&server).await, Err(RpcError::Protocol(_))),
            "{}",
            prefix.escape_ascii()
        );
        server.finish().await;
    }
}

#[tokio::test]
async fn reply_that_is_not_msgpack_is_decode() {
    let server = MockServer::spawn(Script::Exchange {
        form: ServerForm::Modern(Digest::Sha256),
        key: key().as_bytes().to_vec(),
        answer: Answer::Correct,
        reply: Reply::Frame(vec![0xc1]),
    })
    .await;
    assert!(matches!(call(&server).await, Err(RpcError::Decode(_))));
    server.finish().await;
}

#[tokio::test]
async fn reply_over_the_cap_is_protocol() {
    let server = MockServer::spawn(Script::Exchange {
        form: ServerForm::Modern(Digest::Sha256),
        key: key().as_bytes().to_vec(),
        answer: Answer::Correct,
        reply: Reply::BareHeader(16 * 1024 * 1024 + 1),
    })
    .await;
    match call(&server).await {
        Err(RpcError::Protocol(message)) => {
            assert!(message.contains("exceeds cap"), "{message}");
        }
        other => panic!("expected the cap to reject the header, got {other:?}"),
    }
    assert_request(&server.finish().await);
}

#[tokio::test]
async fn silent_server_is_timeout_within_budget() {
    let server = MockServer::spawn(Script::Silent).await;
    let started = Instant::now();
    let result =
        interface_stats(&RpcEndpoint::Tcp(server.addr()), &key(), Duration::from_millis(300)).await;
    assert!(matches!(result, Err(RpcError::Timeout)), "{result:?}");
    assert!(started.elapsed() < Duration::from_secs(2), "{:?}", started.elapsed());
    server.finish().await;
}

#[test]
fn identity_file_key_is_sha256_of_the_file() {
    let private_key: Vec<u8> = (0u8..64).collect();
    let mut file = tempfile::NamedTempFile::new().expect("tempfile");
    file.write_all(&private_key).expect("write identity");
    let from_file = RpcKey::from_transport_identity_file(file.path()).expect("identity key");
    let digest = <sha2::Sha256 as sha2::Digest>::digest(&private_key);
    let from_hex = RpcKey::from_hex(&hex::encode(digest)).expect("hex key");
    assert_eq!(from_file.as_bytes(), from_hex.as_bytes());
    assert_eq!(from_file.as_bytes().len(), 32);
}

#[test]
fn identity_file_of_the_wrong_length_or_unreadable_is_invalid_key_material() {
    let mut file = tempfile::NamedTempFile::new().expect("tempfile");
    file.write_all(&[1u8; 63]).expect("write identity");
    assert!(matches!(
        RpcKey::from_transport_identity_file(file.path()),
        Err(RpcError::InvalidKeyMaterial)
    ));
    let dir = tempfile::tempdir().expect("tempdir");
    assert!(matches!(
        RpcKey::from_transport_identity_file(&dir.path().join("missing")),
        Err(RpcError::InvalidKeyMaterial)
    ));
}

#[test]
fn hex_key_rejects_empty_odd_and_non_hex() {
    for bad in ["", "abc", "zz", "0g", "ab cd", " abcd", "ab cd "] {
        assert!(matches!(RpcKey::from_hex(bad), Err(RpcError::InvalidKeyMaterial)), "{bad:?}");
    }
    assert_eq!(RpcKey::from_hex("ab").expect("one byte").as_bytes(), &[0xab]);
    assert_eq!(key().as_bytes(), hex::decode(KEY_HEX).expect("hex").as_slice());
}

#[test]
fn key_debug_is_redacted() {
    let debug = format!("{:?}", key());
    assert!(debug.contains("redacted"), "{debug}");
    assert!(!debug.contains("0001"), "{debug}");
    assert!(!debug.contains(KEY_HEX), "{debug}");
}

#[test]
fn default_tcp_is_the_python_control_port() {
    assert_eq!(
        RpcEndpoint::default_tcp(),
        RpcEndpoint::Tcp("127.0.0.1:37429".parse().expect("addr"))
    );
}

#[cfg(any(target_os = "linux", target_os = "android"))]
#[test]
fn for_instance_is_the_abstract_socket_on_linux() {
    // The README recipe and its docs pin grep for this exact spelling of
    // `RpcEndpoint::for_instance("default")`; a rename moves with them.
    assert_eq!(
        RpcEndpoint::for_instance("default"),
        RpcEndpoint::Unix(LocalUnixEndpoint::abstract_name("rns/default/rpc"))
    );
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
#[test]
fn for_instance_is_the_default_tcp_port_elsewhere() {
    // The README recipe and its docs pin grep for this exact spelling of
    // `RpcEndpoint::for_instance("default")`; a rename moves with them.
    assert_eq!(RpcEndpoint::for_instance("default"), RpcEndpoint::default_tcp());
}

// Vectors produced by CPython's multiprocessing.connection._create_response
// (connection.py:917-942) with the key above; modern forms MAC the whole
// prefixed message and the legacy form is the raw HMAC-MD5 of a 20-byte body.
// Regenerate under CPython 3.14 with
//   from multiprocessing.connection import _create_response as r
//   k = bytes.fromhex(KEY_HEX); r(k, b"{sha256}" + bytes(range(40))).hex()
// and r(k, bytes(range(20))).hex() for the legacy vector.
#[test]
fn mac_matches_the_cpython_stdlib_vectors() {
    let key = hex::decode(KEY_HEX).expect("hex");
    let msg40: Vec<u8> = (0u8..40).collect();
    let leg20: Vec<u8> = (0u8..20).collect();
    let vectors = [
        (
            Digest::Sha256,
            "7b7368613235367d0988643d19a1da3f98648ba4eec9b8b09b61d95da64bc38ba809c6b7e7bcaf3c",
        ),
        (
            Digest::Sha384,
            "7b7368613338347d258efc885620a24ad71c5d7dd232b7064c369203e7960dfa8c2a5588d69d6ded3fc5e41c4e861722b0e415c8f7852874",
        ),
        (
            Digest::Sha3_256,
            "7b736861335f3235367d9a4785174dcc662b9f374d29e2e045cfbb41d70cd8d00fcf89fdf6766df7819b",
        ),
        (
            Digest::Sha3_384,
            "7b736861335f3338347da0e8ca92ae090c74115c865a149931c06b1ea024715fc3184d9d771c8e3906d5b82cbf560ec563ae6f64dc67757d471d",
        ),
        (Digest::Md5, "7b6d64357d071a06d752d2e7b767cf0b345775467f"),
    ];
    for (digest, response_hex) in vectors {
        let response = hex::decode(response_hex).expect("vector");
        let prefix = digest.prefix();
        assert_eq!(&response[..prefix.len()], prefix.as_slice(), "{digest:?}");
        let expected = &response[prefix.len()..];
        let message = [prefix.as_slice(), msg40.as_slice()].concat();
        assert_eq!(mac(digest, &key, &message), expected, "{digest:?}");
        assert!(verify_mac(digest, &key, &message, expected), "{digest:?}");
    }

    let legacy = hex::decode("7e57151ba744ee63c52e1497fbef19d0").expect("vector");
    assert_eq!(mac(Digest::Md5, &key, &leg20), legacy);
    assert!(verify_mac(Digest::Md5, &key, &leg20, &legacy));
}

// hmac's `new_from_slice` takes a key of any length, so `from_hex` is the
// only place a configured `rpc_key` can be `InvalidKeyMaterial`: a one-byte
// key and one longer than every digest's block size both tag and verify.
#[test]
fn mac_and_verify_mac_take_a_key_of_any_length() {
    let message = b"#CHALLENGE#{sha256}a key of any length";
    let long_key: Vec<u8> = (0..200u32).map(|i| (i * 7 % 251) as u8).collect();
    for digest in [Digest::Md5, Digest::Sha256, Digest::Sha384, Digest::Sha3_256, Digest::Sha3_384]
    {
        for key in [&[0xab_u8][..], long_key.as_slice()] {
            let tag = mac(digest, key, message);
            assert!(verify_mac(digest, key, message, &tag), "{digest:?} key_len={}", key.len());
            let mut flipped = tag.clone();
            flipped[0] ^= 0x01;
            assert!(
                !verify_mac(digest, key, message, &flipped),
                "{digest:?} key_len={}",
                key.len()
            );
        }
    }
}

fn decoded(value: Value) -> Result<InterfaceStats, RpcError> {
    decode::interface_stats(value)
}

fn entry_with(overrides: Vec<(&str, Option<Value>)>) -> Value {
    let mut pairs = vec![
        ("name", Value::from("AutoInterface[x]")),
        ("short_name", Value::from("x")),
        ("type", Value::from("AutoInterface")),
        ("status", Value::from(true)),
        ("rxb", Value::from(1)),
        ("txb", Value::from(2)),
    ];
    for (key, value) in overrides {
        pairs.retain(|(name, _)| *name != key);
        if let Some(value) = value {
            pairs.push((key, value));
        }
    }
    map(pairs)
}

fn reply_with(entry: Value) -> Value {
    map(vec![("interfaces", Value::Array(vec![entry]))])
}

#[test]
fn decode_requires_status() {
    let result = decoded(reply_with(entry_with(vec![("status", None)])));
    match result {
        Err(RpcError::Decode(message)) => assert!(message.contains("status"), "{message}"),
        other => panic!("expected Decode, got {other:?}"),
    }
}

#[test]
fn decode_rejects_a_string_peers() {
    let result = decoded(reply_with(entry_with(vec![("peers", Some(Value::from("many")))])));
    match result {
        Err(RpcError::Decode(message)) => assert!(message.contains("peers"), "{message}"),
        other => panic!("expected Decode, got {other:?}"),
    }
}

#[test]
fn decode_rejects_a_negative_rxb() {
    let result = decoded(reply_with(entry_with(vec![("rxb", Some(Value::from(-1)))])));
    match result {
        Err(RpcError::Decode(message)) => assert!(message.contains("rxb"), "{message}"),
        other => panic!("expected Decode, got {other:?}"),
    }
}

#[test]
fn decode_rejects_a_malformed_top_level_or_entry() {
    assert!(matches!(decoded(Value::Array(vec![])), Err(RpcError::Decode(_))));
    assert!(matches!(decoded(map(vec![("rxb", Value::from(1))])), Err(RpcError::Decode(_))));
    assert!(matches!(
        decoded(map(vec![("interfaces", Value::Array(vec![Value::from(1)]))])),
        Err(RpcError::Decode(_))
    ));
}

#[test]
fn decode_reads_absent_and_nil_counts_as_none() {
    let stats = decoded(reply_with(entry_with(vec![
        ("peers", Some(Value::Nil)),
        ("clients", Some(Value::from(4))),
    ])))
    .expect("decodes");
    assert_eq!(stats.interfaces[0].peers, None);
    assert_eq!(stats.interfaces[0].clients, Some(4));
    assert_eq!(
        stats.interfaces[0].raw,
        entry_with(vec![("peers", Some(Value::Nil)), ("clients", Some(Value::from(4))),])
    );
}
