//! The rnsd path RPC calls against loopback servers that complete the stdlib handshake
//! honestly: `drop_path_settled` keeps ONE budget across a SLOW drop and the polls that
//! follow (an instant drop cannot tell one shared budget from one per phase); the path
//! calls behave the same over a unix endpoint as over TCP (same bytes, same replies, the
//! Linux abstract-socket shape rnsd binds); a wrong key on a path call is
//! `Authentication`, not transient, and `drop_path_settled` dials once for it; a
//! two-thousand-row `path_table` reply (a frame well past 64 KiB, rows with their keys in
//! reverse order and an extra key) is read by name; `next_hop_interface_name` turns only
//! the exact 4-char str `"None"` into `None` (`RNS/Reticulum.py:1715` is `str(None)`);
//! `drop_all_via` reads an int only (`RNS/Reticulum.py:1689-1695`), so a float or a
//! negative is `Decode`; and a zero budget is `Timeout`, never a settled `Ok`.

use std::time::{Duration, Instant};

use hmac::{Hmac, Mac};
use rmpv::Value;
use sha2::Sha256;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpListener;

use rns_transport::hash::AddressHash;
#[cfg(unix)]
use rns_transport::iface::local::LocalUnixEndpoint;
use rns_transport::transport::shared_instance_rpc::{
    drop_all_via, drop_path, drop_path_settled, next_hop_interface_name, path_table, RpcEndpoint,
    RpcError, RpcKey,
};

const WAIT: Duration = Duration::from_secs(5);
const KEY_HEX: &str = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";
const OTHER_KEY: [u8; 32] = [0x77; 32];

const CHALLENGE: &[u8] = b"#CHALLENGE#";
const WELCOME: &[u8] = b"#WELCOME#";
const FAILURE: &[u8] = b"#FAILURE#";

const DESTINATION: [u8; 16] = [
    0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e, 0x1f,
];
const VIA: [u8; 16] = [0x5a; 16];
const INTERFACE_NAME: &str = "TCPInterface[A/127.0.0.1:4242]";

const PATH_TABLE_REQUEST_NO_MAX_HOPS: &[u8] = b"\x82\xa3get\xaapath_table\xa8max_hops\xc0";
const NEXT_HOP_REQUEST_HEAD: &[u8] = b"\x82\xa3get\xb0next_hop_if_name\xb0destination_hash\xc4\x10";
const DROP_PATH_REQUEST_HEAD: &[u8] = b"\x82\xa4drop\xa4path\xb0destination_hash\xc4\x10";
const DROP_ALL_VIA_REQUEST_HEAD: &[u8] = b"\x82\xa4drop\xa7all_via\xb0destination_hash\xc4\x10";

fn key() -> RpcKey {
    RpcKey::from_hex(KEY_HEX).expect("hex key")
}

fn destination() -> AddressHash {
    AddressHash::new(DESTINATION)
}

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

async fn recv_frame<S: AsyncRead + Unpin>(stream: &mut S) -> std::io::Result<Vec<u8>> {
    let mut header = [0u8; 4];
    stream.read_exact(&mut header).await?;
    let len = usize::try_from(i32::from_be_bytes(header)).expect("non-negative length");
    let mut payload = vec![0u8; len];
    stream.read_exact(&mut payload).await?;
    Ok(payload)
}

fn server_challenge() -> Vec<u8> {
    let mut message = b"{sha256}".to_vec();
    message.extend_from_slice(&[0x42u8; 40]);
    message
}

fn answer(authkey: &[u8], message: &[u8]) -> Vec<u8> {
    assert!(message.starts_with(b"{sha256}"), "the client challenges with {{sha256}}: {message:?}");
    let mut mac = Hmac::<Sha256>::new_from_slice(authkey).expect("hmac key");
    mac.update(message);
    let mut out = b"{sha256}".to_vec();
    out.extend_from_slice(&mac.finalize().into_bytes());
    out
}

/// Serve the stdlib's mutual handshake honestly with `authkey` and return the request
/// frame the client sends next; with a key the client does not hold, do what
/// `deliver_challenge` does: send `#FAILURE#`, close, return `None`.
async fn serve_handshake<S: AsyncRead + AsyncWrite + Unpin>(
    stream: &mut S,
    authkey: &[u8],
) -> Option<Vec<u8>> {
    let message = server_challenge();
    let mut challenge = CHALLENGE.to_vec();
    challenge.extend_from_slice(&message);
    stream.write_all(&frame(&challenge)).await.expect("write challenge");
    let response = recv_frame(stream).await.expect("the client's answer");
    if response != answer(authkey, &message) {
        stream.write_all(&frame(FAILURE)).await.expect("write failure");
        stream.flush().await.expect("flush");
        let _ = stream.shutdown().await;
        return None;
    }
    stream.write_all(&frame(WELCOME)).await.expect("write welcome");
    let theirs = recv_frame(stream).await.expect("the client's challenge");
    assert!(theirs.starts_with(CHALLENGE), "{theirs:?}");
    let ours = answer(authkey, &theirs[CHALLENGE.len()..]);
    stream.write_all(&frame(&ours)).await.expect("write our answer");
    let welcome = recv_frame(stream).await.expect("the client's verdict");
    assert_eq!(welcome, WELCOME, "the client rejected a correct answer");
    Some(recv_frame(stream).await.expect("the request"))
}

async fn reply_and_close<S: AsyncRead + AsyncWrite + Unpin>(stream: &mut S, reply: &Value) {
    stream.write_all(&frame(&pack(reply))).await.expect("write reply");
    stream.flush().await.expect("flush");
    stream.shutdown().await.expect("fin");
}

fn pack(value: &Value) -> Vec<u8> {
    let mut bytes = Vec::new();
    rmpv::encode::write_value(&mut bytes, value).expect("pack into a vec");
    bytes
}

fn map(pairs: Vec<(&str, Value)>) -> Value {
    Value::Map(pairs.into_iter().map(|(key, value)| (Value::from(key), value)).collect())
}

/// A TCP loopback listener answering one canned reply per connection, in order; the task
/// yields the requests read.
async fn canned_tcp(replies: Vec<Value>) -> (RpcEndpoint, tokio::task::JoinHandle<Vec<Vec<u8>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind loopback");
    let endpoint = RpcEndpoint::Tcp(listener.local_addr().expect("listener address"));
    let server = tokio::spawn(async move {
        let mut requests = Vec::with_capacity(replies.len());
        for reply in replies {
            let (mut stream, _) = listener.accept().await.expect("accept");
            let request =
                serve_handshake(&mut stream, &authkey()).await.expect("an honest handshake");
            requests.push(request);
            reply_and_close(&mut stream, &reply).await;
        }
        requests
    });
    (endpoint, server)
}

async fn one_request(server: tokio::task::JoinHandle<Vec<Vec<u8>>>) -> Vec<u8> {
    let mut requests = server.await.expect("server task");
    assert_eq!(requests.len(), 1, "{requests:?}");
    requests.remove(0)
}

fn assert_decode_not_transient<T: std::fmt::Debug>(result: &Result<T, RpcError>, what: &str) {
    match result {
        Err(err @ RpcError::Decode(_)) => {
            assert_eq!(err.io_kind(), None, "{what}: {err:?}");
            assert!(!err.is_transient(), "{what}: {err:?}");
        }
        other => panic!("{what}: expected Decode, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------------------
// ONE budget: a drop that takes 400 ms of a 700 ms budget leaves the polls 300 ms.
// ---------------------------------------------------------------------------------------

/// `drop_path`, then poll under the SAME `timeout` budget shared across all dials. A
/// server that answers the drop at once cannot tell that apart from a helper giving the
/// polls a fresh budget after the drop; here the drop reply is held back 400 ms and the
/// next hop never reads `None`, so one shared 700 ms budget ends in `Timeout` near 700 ms
/// while a per-phase budget would run past 1100 ms. The polls still run at their 50 ms
/// cadence inside the 300 ms left: the drop plus at least two polls.
#[tokio::test]
async fn drop_path_settled_shares_one_budget_between_a_slow_drop_and_its_polls() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind loopback");
    let endpoint = RpcEndpoint::Tcp(listener.local_addr().expect("listener address"));
    let served = std::sync::Arc::new(AtomicUsize::new(0));
    let counter = served.clone();
    let server = tokio::spawn(async move {
        loop {
            let (mut stream, _) = listener.accept().await.expect("accept");
            let request = serve_handshake(&mut stream, &authkey()).await.expect("honest");
            let n = counter.fetch_add(1, Ordering::SeqCst);
            if n == 0 {
                assert_eq!(request, hash_request(DROP_PATH_REQUEST_HEAD), "first dial is the drop");
                tokio::time::sleep(Duration::from_millis(400)).await;
                reply_and_close(&mut stream, &Value::from(true)).await;
            } else {
                assert_eq!(request, hash_request(NEXT_HOP_REQUEST_HEAD), "dial {n} is a poll");
                reply_and_close(&mut stream, &Value::from(INTERFACE_NAME)).await;
            }
        }
    });
    let budget = Duration::from_millis(700);
    let started = Instant::now();
    let result = drop_path_settled(&endpoint, &key(), &destination(), budget).await;
    let elapsed = started.elapsed();
    server.abort();
    let dials = served.load(Ordering::SeqCst);
    eprintln!("slow drop + endless polls: {result:?} after {dials} dials in {elapsed:?}");
    match &result {
        Err(err @ RpcError::Timeout) => assert!(err.is_transient(), "{err:?}"),
        other => {
            panic!("a slow drop then a path that never settles: expected Timeout, got {other:?}")
        }
    }
    assert!(elapsed >= Duration::from_millis(600), "{elapsed:?} returned before the budget");
    assert!(
        elapsed < Duration::from_millis(1000),
        "{elapsed:?}: the polls were given their own budget after a 400 ms drop"
    );
    assert!(dials >= 3, "only {dials} dials: no polls in the 300 ms left after the drop");
}

/// A zero budget is `Timeout` before anything settles: the drop never completes, so the
/// helper can neither report `Ok(true)` nor `Ok(false)`.
#[tokio::test]
async fn drop_path_settled_with_a_zero_budget_is_timeout() {
    let (endpoint, server) = canned_tcp(vec![Value::from(true), Value::from("None")]).await;
    let started = Instant::now();
    let result = drop_path_settled(&endpoint, &key(), &destination(), Duration::ZERO).await;
    server.abort();
    assert!(matches!(result, Err(RpcError::Timeout)), "{result:?}");
    assert!(started.elapsed() < Duration::from_secs(1), "{:?}", started.elapsed());
}

// ---------------------------------------------------------------------------------------
// The seam over a unix endpoint: same bytes, same replies.
// ---------------------------------------------------------------------------------------

/// Every public fn takes `&RpcEndpoint`; rnsd on Linux binds the abstract unix name
/// `rns/<instance>/rpc` (`RNS/Reticulum.py:354`). A filesystem unix socket here stands in
/// for it: `drop_path` and `next_hop_interface_name` send the TCP-pinned bytes and read the
/// same replies over it.
#[cfg(unix)]
#[tokio::test]
async fn path_calls_over_a_unix_endpoint_send_the_pinned_bytes_and_read_the_replies() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("rpc.sock");
    let listener = tokio::net::UnixListener::bind(&path).expect("bind unix");
    let server = tokio::spawn(async move {
        let mut requests = Vec::new();
        for reply in [Value::from(true), Value::from("None"), Value::Array(vec![])] {
            let (mut stream, _) = listener.accept().await.expect("accept");
            requests.push(serve_handshake(&mut stream, &authkey()).await.expect("honest"));
            reply_and_close(&mut stream, &reply).await;
        }
        requests
    });
    let endpoint = RpcEndpoint::Unix(LocalUnixEndpoint::filesystem(path));
    let dropped = drop_path(&endpoint, &key(), &destination(), WAIT).await.expect("drop_path");
    assert!(dropped);
    let next = next_hop_interface_name(&endpoint, &key(), &destination(), WAIT)
        .await
        .expect("next_hop_interface_name");
    assert_eq!(next, None);
    let rows = path_table(&endpoint, &key(), None, WAIT).await.expect("path_table");
    assert!(rows.is_empty(), "{rows:?}");
    let requests = server.await.expect("server task");
    assert_eq!(requests[0], hash_request(DROP_PATH_REQUEST_HEAD));
    assert_eq!(requests[1], hash_request(NEXT_HOP_REQUEST_HEAD));
    assert_eq!(requests[2], PATH_TABLE_REQUEST_NO_MAX_HOPS);
}

// ---------------------------------------------------------------------------------------
// A wrong key on a path call.
// ---------------------------------------------------------------------------------------

/// README: `Authentication` is a wrong key (the server's `#FAILURE#`), a report and not a
/// retry. The path calls share the seam, so each ends there with no io kind and
/// `is_transient()` false, and `drop_path_settled` stops after its first dial: nothing to
/// poll for when the drop itself was refused.
#[tokio::test]
async fn wrong_key_on_a_path_call_is_authentication_and_not_transient() {
    async fn refusing_server(dials: usize) -> (RpcEndpoint, tokio::task::JoinHandle<usize>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind loopback");
        let endpoint = RpcEndpoint::Tcp(listener.local_addr().expect("listener address"));
        let server = tokio::spawn(async move {
            let mut refused = 0usize;
            for _ in 0..dials {
                let (mut stream, _) = listener.accept().await.expect("accept");
                assert!(
                    serve_handshake(&mut stream, &OTHER_KEY).await.is_none(),
                    "the client answered a challenge keyed with a key it does not hold"
                );
                refused += 1;
            }
            refused
        });
        (endpoint, server)
    }

    let (endpoint, server) = refusing_server(1).await;
    let result = path_table(&endpoint, &key(), None, WAIT).await;
    match &result {
        Err(err @ RpcError::Authentication) => {
            assert_eq!(err.io_kind(), None, "{err:?}");
            assert!(!err.is_transient(), "{err:?}");
        }
        other => panic!("path_table with a wrong key: expected Authentication, got {other:?}"),
    }
    assert_eq!(server.await.expect("server task"), 1);

    // drop_path_settled: one refused dial, no poll, well inside the budget.
    let (endpoint, server) = refusing_server(1).await;
    let started = Instant::now();
    let result = drop_path_settled(&endpoint, &key(), &destination(), WAIT).await;
    assert!(matches!(result, Err(RpcError::Authentication)), "{result:?}");
    assert!(started.elapsed() < Duration::from_secs(1), "{:?}", started.elapsed());
    assert_eq!(server.await.expect("server task"), 1);
    // A second dial would now be refused by the OS, not by the server: the task is done.
    let again = drop_path(&endpoint, &key(), &destination(), WAIT).await;
    assert!(matches!(again, Err(RpcError::Connect(_))), "{again:?}");
}

// ---------------------------------------------------------------------------------------
// A large table, rows keyed in reverse order.
// ---------------------------------------------------------------------------------------

/// Rows are read BY NAME and `raw` keeps the whole map. Two thousand rows with their six
/// keys in the reverse of `get_path_table`'s order plus an extra one make a reply past
/// 64 KiB (a frame length that needs more than two bytes) and decode to two thousand
/// `PathRow`s in wire order, each `raw` the map as sent.
#[tokio::test]
async fn path_table_with_two_thousand_reversed_rows_is_read_by_name_in_order() {
    let rows: Vec<Value> = (0..2000u32)
        .map(|i| {
            let mut hash = [0u8; 16];
            hash[..4].copy_from_slice(&i.to_be_bytes());
            map(vec![
                ("future", Value::from(i)),
                ("interface", Value::from(format!("TCPInterface[peer{i}/10.0.0.1:{}]", 4000 + i))),
                ("expires", Value::F64(1_700_000_600.5 + f64::from(i))),
                ("hops", Value::from(u8::try_from(i % 128).expect("fits"))),
                ("via", Value::Binary(VIA.to_vec())),
                (
                    "timestamp",
                    if i % 2 == 0 { Value::F64(1_700_000_000.25) } else { Value::from(0) },
                ),
                ("hash", Value::Binary(hash.to_vec())),
            ])
        })
        .collect();
    let packed_len = pack(&Value::Array(rows.clone())).len();
    assert!(packed_len > 64 * 1024, "the reply is {packed_len} bytes; the test wants > 64 KiB");

    let (endpoint, server) = canned_tcp(vec![Value::Array(rows.clone())]).await;
    let decoded = path_table(&endpoint, &key(), None, WAIT).await.expect("path_table");
    assert_eq!(decoded.len(), 2000);
    for (i, (row, sent)) in decoded.iter().zip(rows.iter()).enumerate() {
        let mut hash = [0u8; 16];
        hash[..4].copy_from_slice(&u32::try_from(i).expect("fits").to_be_bytes());
        assert_eq!(row.hash, AddressHash::new(hash), "row {i}");
        assert_eq!(row.via, AddressHash::new(VIA), "row {i}");
        assert_eq!(row.hops, u8::try_from(i % 128).expect("fits"), "row {i}");
        assert_eq!(row.interface, format!("TCPInterface[peer{i}/10.0.0.1:{}]", 4000 + i));
        assert_eq!(row.expires, 1_700_000_600.5 + i as f64, "row {i}");
        assert_eq!(row.timestamp, if i % 2 == 0 { 1_700_000_000.25 } else { 0.0 }, "row {i}");
        assert_eq!(&row.raw, sent, "row {i}");
    }
    assert_eq!(one_request(server).await, PATH_TABLE_REQUEST_NO_MAX_HOPS);
}

// ---------------------------------------------------------------------------------------
// Scalar edges: the exact "None", and an int-only count.
// ---------------------------------------------------------------------------------------

/// The str `"None"` -> `None` is `str(None)` (`RNS/Reticulum.py:1715`), the exact 4-char
/// str; any other str is a name, so `"none"`, `"NONE"` and `"None "` are `Some`.
#[tokio::test]
async fn next_hop_interface_name_treats_only_the_exact_none_str_as_absent() {
    for text in ["none", "NONE", "None ", " None", "Nil"] {
        let (endpoint, server) = canned_tcp(vec![Value::from(text)]).await;
        let name = next_hop_interface_name(&endpoint, &key(), &destination(), WAIT)
            .await
            .expect("next_hop_interface_name");
        assert_eq!(name.as_deref(), Some(text), "{text:?} is a name, not the absent marker");
        assert_eq!(one_request(server).await, hash_request(NEXT_HOP_REQUEST_HEAD));
    }
}

/// `drop all_via` = int (`RNS/Reticulum.py:1689-1695`) -> `u64`. A float, a negative int,
/// a bool and a str are not that count: `Decode`, with no io kind and not transient.
#[tokio::test]
async fn drop_all_via_accepts_only_a_non_negative_integer() {
    for (label, reply) in [
        ("float", Value::F64(2.0)),
        ("negative", Value::from(-1)),
        ("bool", Value::from(true)),
        ("str", Value::from("2")),
    ] {
        let (endpoint, server) = canned_tcp(vec![reply]).await;
        let result = drop_all_via(&endpoint, &key(), &destination(), WAIT).await;
        assert_decode_not_transient(&result, label);
        assert_eq!(one_request(server).await, hash_request(DROP_ALL_VIA_REQUEST_HEAD));
    }
    let (endpoint, server) = canned_tcp(vec![Value::from(0)]).await;
    let count = drop_all_via(&endpoint, &key(), &destination(), WAIT).await.expect("drop_all_via");
    assert_eq!(count, 0, "no path via that transport is the int 0");
    assert_eq!(one_request(server).await, hash_request(DROP_ALL_VIA_REQUEST_HEAD));
}
