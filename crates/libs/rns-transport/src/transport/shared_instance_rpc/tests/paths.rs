use std::time::{Duration, Instant};

use rmpv::Value;

use super::*;
use crate::hash::AddressHash;

const DESTINATION: [u8; 16] = [
    0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e, 0x1f,
];
const VIA: [u8; 16] = [0x5a; 16];
const INTERFACE_NAME: &str = "TCPInterface[A/127.0.0.1:4242]";

fn destination() -> AddressHash {
    AddressHash::new(DESTINATION)
}

fn answering(reply: Value) -> Script {
    Script::Exchange {
        form: ServerForm::Modern(Digest::Sha256),
        key: key().as_bytes().to_vec(),
        answer: Answer::Correct,
        reply: Reply::Frame(pack(&reply)),
    }
}

async fn served(reply: Value) -> MockServer {
    MockServer::spawn(answering(reply)).await
}

fn endpoint(server: &MockServer) -> RpcEndpoint {
    RpcEndpoint::Tcp(server.addr())
}

fn request(observed: &mock_server::Observed) -> &[u8] {
    observed.request.as_deref().expect("request frame")
}

/// `head` is the fixmap up to and including the bin8 header `\xc4\x10`; the
/// destination's 16 bytes follow.
fn hash_request(head: &[u8]) -> Vec<u8> {
    let mut bytes = head.to_vec();
    bytes.extend_from_slice(&DESTINATION);
    bytes
}

const NEXT_HOP_REQUEST_HEAD: &[u8] = b"\x82\xa3get\xb0next_hop_if_name\xb0destination_hash\xc4\x10";
const DROP_PATH_REQUEST_HEAD: &[u8] = b"\x82\xa4drop\xa4path\xb0destination_hash\xc4\x10";
const DROP_ALL_VIA_REQUEST_HEAD: &[u8] = b"\x82\xa4drop\xa7all_via\xb0destination_hash\xc4\x10";
const FIRST_HOP_TIMEOUT_REQUEST_HEAD: &[u8] =
    b"\x82\xa3get\xb1first_hop_timeout\xb0destination_hash\xc4\x10";

fn assert_decode<T: std::fmt::Debug>(result: &Result<T, RpcError>, names: &str) {
    match result {
        Err(RpcError::Decode(text)) => assert!(text.contains(names), "{text:?} lacks {names:?}"),
        other => panic!("expected Decode naming {names:?}, got {other:?}"),
    }
}

/// A row the way `get_path_table` builds it (`RNS/Reticulum.py:1642-1645`),
/// with `overrides` replacing or (via `None`) removing keys.
fn row(overrides: Vec<(&str, Option<Value>)>) -> Value {
    let mut pairs = vec![
        ("hash", Value::Binary(DESTINATION.to_vec())),
        ("timestamp", Value::F64(1_700_000_000.25)),
        ("via", Value::Binary(VIA.to_vec())),
        ("hops", Value::from(3)),
        ("expires", Value::F64(1_700_000_600.5)),
        ("interface", Value::from(INTERFACE_NAME)),
    ];
    for (key, value) in overrides {
        pairs.retain(|(name, _)| *name != key);
        if let Some(value) = value {
            pairs.push((key, value));
        }
    }
    map(pairs)
}

#[tokio::test]
async fn path_table_request_without_max_hops_sends_nil() {
    let server = served(Value::Array(vec![])).await;
    let rows = path_table(&endpoint(&server), &key(), None, WAIT).await.expect("path_table");
    assert!(rows.is_empty());
    let observed = server.finish().await;
    assert_eq!(request(&observed), b"\x82\xa3get\xaapath_table\xa8max_hops\xc0");
}

#[tokio::test]
async fn path_table_request_with_max_hops_sends_the_count() {
    let server = served(Value::Array(vec![])).await;
    path_table(&endpoint(&server), &key(), Some(5), WAIT).await.expect("path_table");
    let observed = server.finish().await;
    assert_eq!(request(&observed), b"\x82\xa3get\xaapath_table\xa8max_hops\x05");
}

#[tokio::test]
async fn path_table_reply_decodes_rows_and_keeps_the_raw_entry() {
    let second = row(vec![
        ("hash", Some(Value::Binary(vec![0x02; 16]))),
        ("hops", Some(Value::from(1))),
        ("extra", Some(Value::from("unknown"))),
    ]);
    let dropped = row(vec![
        ("hash", Some(Value::Binary(vec![0x03; 16]))),
        ("timestamp", Some(Value::from(0))),
    ]);
    let reply = Value::Array(vec![row(vec![]), second.clone(), dropped]);
    let server = served(reply).await;
    let rows = path_table(&endpoint(&server), &key(), None, WAIT).await.expect("path_table");
    server.finish().await;

    assert_eq!(rows.len(), 3);
    let first = &rows[0];
    assert_eq!(first.hash, destination());
    assert_eq!(first.via, AddressHash::new(VIA));
    assert_eq!(first.timestamp, 1_700_000_000.25);
    assert_eq!(first.expires, 1_700_000_600.5);
    assert_eq!(first.hops, 3);
    assert_eq!(first.interface, INTERFACE_NAME);
    assert_eq!(first.raw, row(vec![]));

    assert_eq!(rows[1].hash, AddressHash::new([0x02; 16]));
    assert_eq!(rows[1].hops, 1);
    assert_eq!(rows[1].raw, second);
    assert!(rows[1]
        .raw
        .as_map()
        .expect("map")
        .iter()
        .any(|(key, value)| key.as_str() == Some("extra") && value.as_str() == Some("unknown")));

    assert_eq!(rows[2].hash, AddressHash::new([0x03; 16]));
    assert_eq!(rows[2].timestamp, 0.0);
}

#[test]
fn path_table_decode_rejects_bad_hashes_and_shapes() {
    let short_hash = Value::Array(vec![row(vec![("hash", Some(Value::Binary(vec![0x01; 15])))])]);
    assert_decode(&path_row::path_table(short_hash), "hash");
    let str_via = Value::Array(vec![row(vec![("via", Some(Value::from("5a5a")))])]);
    assert_decode(&path_row::path_table(str_via), "via");
    let wide_hops = Value::Array(vec![row(vec![("hops", Some(Value::from(256)))])]);
    assert_decode(&path_row::path_table(wide_hops), "hops");
    let missing_interface = Value::Array(vec![row(vec![("interface", None)])]);
    assert_decode(&path_row::path_table(missing_interface), "interface");
    assert_decode(&path_row::path_table(map(vec![("interfaces", Value::Array(vec![]))])), "array");
    assert_decode(&path_row::path_table(Value::Array(vec![Value::from(1)])), "map");
}

#[tokio::test]
async fn next_hop_interface_name_reads_the_python_str() {
    for (reply, expected) in [
        (Value::from("None"), None),
        (Value::from(INTERFACE_NAME), Some(INTERFACE_NAME.to_string())),
        (Value::Nil, None),
    ] {
        let server = served(reply.clone()).await;
        let name = next_hop_interface_name(&endpoint(&server), &key(), &destination(), WAIT)
            .await
            .expect("next_hop_interface_name");
        assert_eq!(name, expected, "reply {reply:?}");
        let observed = server.finish().await;
        assert_eq!(request(&observed), hash_request(NEXT_HOP_REQUEST_HEAD));
    }
}

#[tokio::test]
async fn next_hop_interface_name_rejects_a_non_str_reply() {
    let server = served(Value::from(7)).await;
    let result = next_hop_interface_name(&endpoint(&server), &key(), &destination(), WAIT).await;
    assert_decode(&result, "str");
    server.finish().await;
    // A one-byte fixstr holding 0xff: a str on the wire, but not UTF-8.
    let server = MockServer::spawn(Script::Exchange {
        form: ServerForm::Modern(Digest::Sha256),
        key: key().as_bytes().to_vec(),
        answer: Answer::Correct,
        reply: Reply::Frame(vec![0xa1, 0xff]),
    })
    .await;
    let result = next_hop_interface_name(&endpoint(&server), &key(), &destination(), WAIT).await;
    match &result {
        Err(RpcError::Decode(text)) => assert_eq!(text, "reply str is not utf-8"),
        other => panic!("expected Decode for a non-utf-8 str, got {other:?}"),
    }
    server.finish().await;
}

#[tokio::test]
async fn drop_path_reads_the_bool() {
    for expected in [true, false] {
        let server = served(Value::from(expected)).await;
        let dropped =
            drop_path(&endpoint(&server), &key(), &destination(), WAIT).await.expect("drop_path");
        assert_eq!(dropped, expected);
        let observed = server.finish().await;
        assert_eq!(request(&observed), hash_request(DROP_PATH_REQUEST_HEAD));
    }
    let server = served(Value::from("True")).await;
    let result = drop_path(&endpoint(&server), &key(), &destination(), WAIT).await;
    assert_decode(&result, "bool");
    server.finish().await;
}

#[tokio::test]
async fn drop_all_via_reads_the_count() {
    let server = served(Value::from(2)).await;
    let count =
        drop_all_via(&endpoint(&server), &key(), &destination(), WAIT).await.expect("drop_all_via");
    assert_eq!(count, 2);
    let observed = server.finish().await;
    assert_eq!(request(&observed), hash_request(DROP_ALL_VIA_REQUEST_HEAD));
    let server = served(Value::Nil).await;
    let result = drop_all_via(&endpoint(&server), &key(), &destination(), WAIT).await;
    assert_decode(&result, "integer");
    server.finish().await;
}

#[tokio::test]
async fn first_hop_timeout_reads_int_and_float() {
    for (reply, expected) in [(Value::from(6), 6.0), (Value::F64(6.0128), 6.0128)] {
        let server = served(reply).await;
        let seconds = first_hop_timeout(&endpoint(&server), &key(), &destination(), WAIT)
            .await
            .expect("first_hop_timeout");
        assert_eq!(seconds, expected);
        let observed = server.finish().await;
        assert_eq!(request(&observed), hash_request(FIRST_HOP_TIMEOUT_REQUEST_HEAD));
    }
    let server = served(Value::from("6")).await;
    let result = first_hop_timeout(&endpoint(&server), &key(), &destination(), WAIT).await;
    assert_decode(&result, "number");
    server.finish().await;
}

#[tokio::test]
async fn drop_path_settled_polls_until_the_path_is_gone() {
    let server = MockServer::spawn_sequence(vec![
        answering(Value::from(true)),
        answering(Value::from("TCPInterface[x]")),
        answering(Value::from("TCPInterface[x]")),
        answering(Value::from("None")),
    ])
    .await;
    let started = Instant::now();
    let settled = drop_path_settled(&endpoint(&server), &key(), &destination(), WAIT)
        .await
        .expect("drop_path_settled");
    assert!(settled);
    // Two polls read a name before the third reads "None": two sleeps of
    // the 50 ms cadence, which tokio's sleep never cuts short.
    assert!(
        started.elapsed() >= 2 * Duration::from_millis(50),
        "{:?} is shorter than two poll intervals",
        started.elapsed()
    );
    assert!(started.elapsed() < WAIT, "{:?}", started.elapsed());
    let observed = server.finish_all().await;
    assert_eq!(observed.len(), 4);
    assert_eq!(request(&observed[0]), hash_request(DROP_PATH_REQUEST_HEAD));
    for poll in &observed[1..] {
        assert_eq!(request(poll), hash_request(NEXT_HOP_REQUEST_HEAD));
    }
}

#[tokio::test]
async fn drop_path_settled_without_a_path_does_not_poll() {
    let server = MockServer::spawn_sequence(vec![answering(Value::from(false))]).await;
    let settled = drop_path_settled(&endpoint(&server), &key(), &destination(), WAIT)
        .await
        .expect("drop_path_settled");
    assert!(!settled);
    let observed = server.finish_all().await;
    assert_eq!(observed.len(), 1);
    assert_eq!(request(&observed[0]), hash_request(DROP_PATH_REQUEST_HEAD));
}

/// A poll that meets a transient failure is returned as it is: the helper
/// never retries, budget left or not.
#[tokio::test]
async fn drop_path_settled_returns_a_transient_poll_error_without_retrying() {
    let server = MockServer::spawn_sequence(vec![
        answering(Value::from(true)),
        answering(Value::from("TCPInterface[x]")),
        Script::Close,
    ])
    .await;
    let started = Instant::now();
    let result = drop_path_settled(&endpoint(&server), &key(), &destination(), WAIT).await;
    let err = result.expect_err("a closed poll connection is an error");
    assert_eq!(err.io_kind(), Some(std::io::ErrorKind::UnexpectedEof), "{err:?}");
    assert!(err.is_transient(), "{err:?}");
    assert!(started.elapsed() < Duration::from_secs(2), "{:?}", started.elapsed());
    assert_eq!(server.finish_all().await.len(), 3);
}

#[tokio::test]
async fn drop_path_settled_outrunning_its_budget_is_timeout() {
    let budget = Duration::from_millis(400);
    let mut scripts = vec![answering(Value::from(true))];
    scripts.extend((0..64).map(|_| answering(Value::from("TCPInterface[x]"))));
    let server = MockServer::spawn_sequence(scripts).await;
    let started = Instant::now();
    let result = drop_path_settled(&endpoint(&server), &key(), &destination(), budget).await;
    assert!(matches!(result, Err(RpcError::Timeout)), "{result:?}");
    assert!(started.elapsed() >= budget, "{:?}", started.elapsed());
    assert!(started.elapsed() < Duration::from_secs(2), "{:?}", started.elapsed());
}

#[test]
fn is_transient_follows_the_io_kind_retry_rule() {
    use std::io::{Error, ErrorKind};
    for kind in [
        ErrorKind::ConnectionReset,
        ErrorKind::ConnectionAborted,
        ErrorKind::BrokenPipe,
        ErrorKind::UnexpectedEof,
    ] {
        assert!(RpcError::Io(Error::from(kind)).is_transient(), "Io({kind:?})");
        assert!(RpcError::Connect(Error::from(kind)).is_transient(), "Connect({kind:?})");
    }
    assert!(RpcError::Timeout.is_transient());
    for err in [
        RpcError::Connect(Error::from(ErrorKind::ConnectionRefused)),
        RpcError::Io(Error::from(ErrorKind::Other)),
        RpcError::Authentication,
        RpcError::Protocol("x".into()),
        RpcError::Decode("x".into()),
        RpcError::UnsupportedDigest("x".into()),
        RpcError::InvalidKeyMaterial,
    ] {
        assert!(!err.is_transient(), "{err:?}");
    }
}

// A new variant stops this match compiling; add it to the hand-maintained list in
// tests/support/every_rpc_error.rs as well.
#[test]
fn every_rpc_error_variant_is_in_the_census() {
    match RpcError::Timeout {
        RpcError::Connect(_)
        | RpcError::Authentication
        | RpcError::Protocol(_)
        | RpcError::Io(_)
        | RpcError::Decode(_)
        | RpcError::UnsupportedDigest(_)
        | RpcError::InvalidKeyMaterial
        | RpcError::Timeout => {}
    }
}
