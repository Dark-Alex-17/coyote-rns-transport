//! The rnsd RPC client, through `rns_transport::transport::shared_instance_rpc` only,
//! against a stdlib-faithful mock listener on a real loopback socket speaking the
//! Python 3.12+ `{sha256}` form (`connection.py:917-1006`): a server that passes our
//! answer, then answers our own challenge wrong and closes with a FIN at once (as a
//! Python `Connection.close()` does) reads as `Err(RpcError::Authentication)` every time
//! and well inside the budget; the same mock answering honestly serves the reply, so the
//! refusal is the server's doing and not the mock's framing. On loopback a FIN never
//! makes the client's `#FAILURE#` write fail, so this is a departed-server exercise and
//! not the pin of the write-error arm: that arm is pinned by the in-crate duplex test
//! `failure_notice_on_a_broken_pipe_is_still_authentication` and exercised against
//! CPython's reset in `shared_instance_rpc_departed_server_python_probe.rs`. A one-byte
//! key and a 200-byte key (longer than the SHA-256 block) pass the same handshake: the
//! HMAC takes a key of any length, as the README says.

use std::time::{Duration, Instant};

use hmac::{Hmac, Mac};
use rmpv::Value;
use rns_transport::transport::shared_instance_rpc::{
    interface_stats, RpcEndpoint, RpcError, RpcKey,
};
use sha2::Sha256;
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};

#[path = "support/stdlib_rpc_frame.rs"]
mod stdlib_rpc_frame;
use stdlib_rpc_frame::{frame, recv_frame, CHALLENGE, DIGEST_PREFIX, FAILURE, WELCOME};

const WAIT: Duration = Duration::from_secs(5);
const KEY_HEX: &str = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";
const ONE_BYTE_KEY_HEX: &str = "ab";
const LONG_KEY_LEN: usize = 200;
const DEPARTED_ROUNDS: usize = 16;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Server {
    /// Answers the client's challenge correctly and serves `interface_stats`.
    Honest,
    /// Answers the client's challenge with every MAC byte inverted, then closes the
    /// socket before the client's verdict can arrive.
    WrongThenDepart,
}

/// The bare MAC with no `{sha256}` prefix: `WrongThenDepart` inverts its bytes before the
/// prefix goes on.
fn hmac_sha256(key: &[u8], message: &[u8]) -> Vec<u8> {
    let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("hmac takes any key length");
    mac.update(message);
    mac.finalize().into_bytes().to_vec()
}

/// `Connection._send_bytes`, flushed; returns the io result so the `#FAILURE#` write can
/// discard it.
async fn send_frame(stream: &mut TcpStream, payload: &[u8]) -> std::io::Result<()> {
    stream.write_all(&frame(payload)).await?;
    stream.flush().await
}

fn stats_reply() -> Vec<u8> {
    let iface = Value::Map(vec![
        (Value::from("name"), Value::from("AutoInterface[probe]")),
        (Value::from("short_name"), Value::from("probe")),
        (Value::from("type"), Value::from("AutoInterface")),
        (Value::from("status"), Value::from(true)),
        (Value::from("peers"), Value::from(0u64)),
        (Value::from("clients"), Value::Nil),
        (Value::from("rxb"), Value::from(0u64)),
        (Value::from("txb"), Value::from(0u64)),
        (Value::from("hash"), Value::Binary(vec![1u8; 32])),
    ]);
    let top = Value::Map(vec![(Value::from("interfaces"), Value::Array(vec![iface]))]);
    let mut buf = Vec::new();
    rmpv::encode::write_value(&mut buf, &top).expect("encode reply");
    buf
}

/// What the mock saw of one connection, for the test's diagnostics.
#[derive(Debug, Default, PartialEq, Eq)]
struct Seen {
    client_answer_ok: bool,
    client_challenge_prefixed: bool,
    verdict: Option<Vec<u8>>,
    request: Option<Vec<u8>>,
}

/// `Listener.accept` as CPython 3.12+ runs it: `deliver_challenge` then
/// `answer_challenge`, then one request and one reply (`RNS/Reticulum.py:1274-1334`).
async fn serve_one(mut stream: TcpStream, key: Vec<u8>, server: Server) -> Seen {
    let mut seen = Seen::default();
    let body = [DIGEST_PREFIX, &[0x5a; 32]].concat();
    send_frame(&mut stream, &[CHALLENGE, body.as_slice()].concat()).await.expect("send challenge");
    let answer = recv_frame(&mut stream).await.expect("recv answer");
    let expected = [DIGEST_PREFIX, hmac_sha256(&key, &body).as_slice()].concat();
    seen.client_answer_ok = answer == expected;
    if !seen.client_answer_ok {
        let _ = send_frame(&mut stream, FAILURE).await;
        return seen;
    }
    send_frame(&mut stream, WELCOME).await.expect("send welcome");

    let challenge = recv_frame(&mut stream).await.expect("recv client challenge");
    let Some(client_body) = challenge.strip_prefix(CHALLENGE) else {
        return seen;
    };
    seen.client_challenge_prefixed = client_body.starts_with(DIGEST_PREFIX);
    let mut tag = hmac_sha256(&key, client_body);
    if server == Server::WrongThenDepart {
        for byte in &mut tag {
            *byte = !*byte;
        }
    }
    send_frame(&mut stream, &[DIGEST_PREFIX, tag.as_slice()].concat()).await.expect("send answer");
    if server == Server::WrongThenDepart {
        // A plain close, as `Connection.close()` is. The reset variant lives in
        // `shared_instance_rpc_departed_server_python_probe.rs`, where the client's
        // `#FAILURE#` write, not its read, is what fails.
        stream.shutdown().await.expect("shutdown before the verdict");
        drop(stream);
        return seen;
    }
    let verdict = recv_frame(&mut stream).await.expect("recv verdict");
    seen.verdict = Some(verdict.clone());
    if verdict != WELCOME {
        return seen;
    }
    let request = recv_frame(&mut stream).await.expect("recv request");
    seen.request = Some(request);
    send_frame(&mut stream, &stats_reply()).await.expect("send reply");
    seen
}

async fn spawn_mock(key: Vec<u8>, server: Server) -> (RpcEndpoint, tokio::task::JoinHandle<Seen>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind mock");
    let addr = listener.local_addr().expect("mock addr");
    let task = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("accept");
        serve_one(stream, key, server).await
    });
    (RpcEndpoint::Tcp(addr), task)
}

fn request_bytes() -> Vec<u8> {
    let mut buf = Vec::new();
    let value = Value::Map(vec![(Value::from("get"), Value::from("interface_stats"))]);
    rmpv::encode::write_value(&mut buf, &value).expect("encode request");
    buf
}

#[tokio::test]
async fn honest_mock_serves_the_reply_so_the_mock_is_faithful() {
    let key = RpcKey::from_hex(KEY_HEX).expect("hex key");
    let (endpoint, task) = spawn_mock(hex::decode(KEY_HEX).expect("hex"), Server::Honest).await;
    let stats = interface_stats(&endpoint, &key, WAIT).await.expect("honest reply");
    let seen = task.await.expect("mock task");
    assert!(seen.client_answer_ok, "{seen:?}");
    assert!(seen.client_challenge_prefixed, "the client challenges in the modern form: {seen:?}");
    assert_eq!(seen.verdict.as_deref(), Some(WELCOME), "{seen:?}");
    assert_eq!(seen.request, Some(request_bytes()), "{seen:?}");
    assert_eq!(stats.interfaces.len(), 1);
    let iface = &stats.interfaces[0];
    assert_eq!(iface.kind, "AutoInterface");
    assert_eq!(iface.peers, Some(0));
    assert_eq!(iface.clients, None);
    assert!(iface.online);
}

#[tokio::test]
async fn one_byte_key_passes_the_handshake() {
    let key = RpcKey::from_hex(ONE_BYTE_KEY_HEX).expect("one-byte key");
    let (endpoint, task) = spawn_mock(vec![0xab], Server::Honest).await;
    let stats = interface_stats(&endpoint, &key, WAIT).await.expect("one-byte key reply");
    let seen = task.await.expect("mock task");
    assert!(seen.client_answer_ok, "{seen:?}");
    assert_eq!(stats.interfaces.len(), 1);
}

#[tokio::test]
async fn long_key_passes_the_handshake() {
    let key_bytes: Vec<u8> = (0..LONG_KEY_LEN).map(|i| (i * 7 % 251) as u8).collect();
    let key = RpcKey::from_hex(&hex::encode(&key_bytes)).expect("long key");
    let (endpoint, task) = spawn_mock(key_bytes, Server::Honest).await;
    let stats = interface_stats(&endpoint, &key, WAIT).await.expect("long key reply");
    let seen = task.await.expect("mock task");
    assert!(seen.client_answer_ok, "{seen:?}");
    assert_eq!(stats.interfaces.len(), 1);
}

#[tokio::test]
async fn wrong_answer_from_a_departed_server_is_authentication_every_time() {
    let key = RpcKey::from_hex(KEY_HEX).expect("hex key");
    for round in 0..DEPARTED_ROUNDS {
        let (endpoint, task) =
            spawn_mock(hex::decode(KEY_HEX).expect("hex"), Server::WrongThenDepart).await;
        let started = Instant::now();
        let result = interface_stats(&endpoint, &key, WAIT).await;
        let elapsed = started.elapsed();
        let seen = task.await.expect("mock task");
        assert!(seen.client_answer_ok, "round {round}: {seen:?}");
        assert_eq!(
            seen.verdict, None,
            "round {round}: WrongThenDepart reads no verdict by construction: {seen:?}"
        );
        assert!(
            matches!(result, Err(RpcError::Authentication)),
            "round {round}: expected Authentication, got {result:?}"
        );
        assert!(elapsed < Duration::from_secs(2), "round {round}: {elapsed:?}");
    }
}
