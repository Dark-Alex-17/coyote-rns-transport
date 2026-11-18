use std::io::Write;
use std::time::{Duration, Instant};

use rmpv::Value;

use super::handshake::Digest;
use super::*;

mod mock_server;
use mock_server::{Answer, MockServer, Script, ServerForm};

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
    Script::Exchange { form, key: key().as_bytes().to_vec(), answer, reply: pack(&canned_reply()) }
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
    assert_ne!(answer.first(), Some(&b'{'));
    let challenge = observed.challenge.as_ref().expect("challenge");
    let body = challenge.strip_prefix(b"#CHALLENGE#").expect("challenge prefix");
    assert_eq!(body.len(), 20, "a legacy challenge is 20 bare bytes: {body:?}");
    assert_ne!(body.first(), Some(&b'{'));
    assert_eq!(observed.verdict.as_deref(), Some(b"#WELCOME#".as_slice()));
    assert_request(&observed);
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
        reply: pack(&canned_reply()),
    })
    .await;
    assert!(matches!(call(&server).await, Err(RpcError::Authentication)));
    let observed = server.finish().await;
    assert!(observed.challenge.is_none(), "the client stops at #FAILURE#");
}

#[tokio::test]
async fn bad_server_answer_is_authentication_and_sends_failure() {
    let server =
        MockServer::spawn(exchange(ServerForm::Modern(Digest::Sha256), Answer::Wrong)).await;
    assert!(matches!(call(&server).await, Err(RpcError::Authentication)));
    let observed = server.finish().await;
    assert_eq!(observed.verdict.as_deref(), Some(b"#FAILURE#".as_slice()));
    assert!(observed.request.is_none());
}

#[tokio::test]
async fn legacy_server_with_bad_answer_is_authentication() {
    let server = MockServer::spawn(exchange(ServerForm::Legacy, Answer::Wrong)).await;
    assert!(matches!(call(&server).await, Err(RpcError::Authentication)));
    let observed = server.finish().await;
    assert_eq!(observed.verdict.as_deref(), Some(b"#FAILURE#".as_slice()));
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

#[tokio::test]
async fn short_challenge_body_is_protocol() {
    let mut frame = b"#CHALLENGE#".to_vec();
    frame.extend_from_slice(&[7u8; 10]);
    let server = MockServer::spawn(Script::Frame(frame)).await;
    assert!(matches!(call(&server).await, Err(RpcError::Protocol(_))));
    server.finish().await;
}

#[tokio::test]
async fn truncated_frame_is_protocol() {
    let mut bytes = 48i32.to_be_bytes().to_vec();
    bytes.extend_from_slice(b"#CHALLENGE");
    let server = MockServer::spawn(Script::RawBytes(bytes)).await;
    assert!(matches!(call(&server).await, Err(RpcError::Protocol(_))));
    server.finish().await;
}

#[tokio::test]
async fn oversized_auth_frame_is_protocol() {
    let server = MockServer::spawn(Script::RawBytes(300i32.to_be_bytes().to_vec())).await;
    assert!(matches!(call(&server).await, Err(RpcError::Protocol(_))));
    server.finish().await;
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
    for bad in ["", "abc", "zz", "0g"] {
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
    assert_eq!(
        RpcEndpoint::for_instance("default"),
        RpcEndpoint::Unix(LocalUnixEndpoint::abstract_name("rns/default/rpc"))
    );
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
#[test]
fn for_instance_is_the_default_tcp_port_elsewhere() {
    assert_eq!(RpcEndpoint::for_instance("default"), RpcEndpoint::default_tcp());
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
fn decode_rejects_a_top_level_that_is_not_a_map() {
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
