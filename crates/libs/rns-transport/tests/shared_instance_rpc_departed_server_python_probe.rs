//! The rnsd RPC client, through `rns_transport::transport::shared_instance_rpc`
//! only, against CPython's own `multiprocessing.connection` as the server side of the
//! handshake (`deliver_challenge` / `answer_challenge`, `connection.py:917-1006`), run on a
//! raw loopback socket so the server can misbehave after the stdlib has done the
//! challenging. The README promises that a server answering our challenge wrong and
//! hanging up at once reads as `Authentication` every time, and `RpcError::Protocol`'s
//! rustdoc carves out the failed `#FAILURE#` write, which stays `Authentication`. The
//! stdlib pins the frame shapes; this file adds the hang-up the in-tree Rust mock does
//! not model: a reset (`SO_LINGER 0`), where the client's `#FAILURE#` write meets
//! ECONNRESET/EPIPE rather than a kernel buffer that still accepts it. The honest mode of
//! the same script serves a reply, so a refusal is the server's doing and not this
//! file's framing. A third mode is an honest server that resets after reading the
//! request, so the client's reply read meets ECONNRESET and the call is
//! `RpcError::Io`.
//!
//! Needs `python3` (or `LXMF_PYTHON_BIN`) on PATH; the script imports the stdlib only.

use std::io::{BufRead, BufReader};
use std::net::SocketAddr;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use rns_transport::transport::shared_instance_rpc::{
    interface_stats, RpcEndpoint, RpcError, RpcKey,
};

const CALL: Duration = Duration::from_secs(5);
const ROUNDS: usize = 12;
const KEY_HEX: &str = "00112233445566778899aabbccddeeff";

const PYTHON_SERVER: &str = r#"
import json, os, socket, struct, sys
from multiprocessing.connection import Connection, deliver_challenge, answer_challenge
key = bytes.fromhex(sys.argv[1])
mode = sys.argv[2]
srv = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
srv.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
srv.bind(("127.0.0.1", 0))
srv.listen(16)
print(json.dumps({"port": srv.getsockname()[1]}), flush=True)
EMPTY_STATS = b"\x81\xaainterfaces\x90"  # msgpack {"interfaces": []}
while True:
    s, _ = srv.accept()
    conn = Connection(s.detach())
    try:
        deliver_challenge(conn, key)
    except Exception as err:
        print(json.dumps({"refused": str(err)}), flush=True)
        conn.close()
        continue
    if mode == "honest":
        answer_challenge(conn, key)
        conn.recv_bytes(1 << 16)
        conn.send_bytes(EMPTY_STATS)
        conn.close()
        print(json.dumps({"served": True}), flush=True)
        continue
    if mode == "honest_rst":
        answer_challenge(conn, key)
        conn.recv_bytes(1 << 16)
        raw = socket.socket(fileno=conn.fileno())
        raw.setsockopt(socket.SOL_SOCKET, socket.SO_LINGER, struct.pack("ii", 1, 0))
        raw.detach()
        conn.close()
        print(json.dumps({"departed": mode}), flush=True)
        continue
    # The client's challenge: answer it wrong (right shape, wrong bytes) and hang up.
    challenge = conn.recv_bytes(256)
    conn.send_bytes(b"{sha256}" + bytes(32))
    if mode == "rst":
        raw = socket.socket(fileno=conn.fileno())
        raw.setsockopt(socket.SOL_SOCKET, socket.SO_LINGER, struct.pack("ii", 1, 0))
        raw.detach()
    conn.close()
    print(json.dumps({"departed": mode}), flush=True)
"#;

struct PythonServer {
    child: Child,
    endpoint: RpcEndpoint,
}

impl PythonServer {
    fn spawn(mode: &str) -> Self {
        let python = std::env::var("LXMF_PYTHON_BIN").unwrap_or_else(|_| "python3".to_string());
        let mut child = Command::new(python)
            .arg("-c")
            .arg(PYTHON_SERVER)
            .arg(KEY_HEX)
            .arg(mode)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("spawn python3 with the stdlib server");
        let stdout = child.stdout.take().expect("piped stdout");
        let mut lines = BufReader::new(stdout).lines();
        let first = lines.next().expect("the port line").expect("readable port line");
        let port = serde_json::from_str::<serde_json::Value>(&first)
            .ok()
            .and_then(|line| line["port"].as_u64())
            .and_then(|port| u16::try_from(port).ok())
            .unwrap_or_else(|| panic!("port line {first:?}"));
        // Drain the rest in the background so the child never blocks on a full pipe.
        std::thread::spawn(move || for _line in lines.map_while(Result::ok) {});
        Self { child, endpoint: RpcEndpoint::Tcp(SocketAddr::from(([127, 0, 0, 1], port))) }
    }
}

impl Drop for PythonServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// The stdlib server passing both challenges serves the reply: the framing is
/// CPython's, so the refusals below are the server's doing.
#[tokio::test]
async fn stdlib_server_that_answers_honestly_serves_the_reply() {
    let server = PythonServer::spawn("honest");
    let key = RpcKey::from_hex(KEY_HEX).expect("key");
    let stats = interface_stats(&server.endpoint, &key, CALL)
        .await
        .unwrap_or_else(|err| panic!("honest stdlib server: {err:?}"));
    assert!(stats.interfaces.is_empty(), "{stats:?}");
}

async fn every_round_is_authentication(mode: &str) {
    let server = PythonServer::spawn(mode);
    let key = RpcKey::from_hex(KEY_HEX).expect("key");
    for round in 0..ROUNDS {
        let started = Instant::now();
        let result = interface_stats(&server.endpoint, &key, CALL).await;
        let elapsed = started.elapsed();
        assert!(
            matches!(result, Err(RpcError::Authentication)),
            "mode {mode} round {round}: expected Authentication, got {result:?}"
        );
        assert!(elapsed < CALL, "mode {mode} round {round}: {elapsed:?} ran to the budget");
    }
}

/// A `Connection.close()` (FIN) right after the wrong answer.
#[tokio::test]
async fn stdlib_server_answering_wrong_then_closing_is_authentication_every_time() {
    every_round_is_authentication("fin").await;
}

/// A reset (`SO_LINGER 0`) right after the wrong answer, so the client's `#FAILURE#`
/// write fails outright rather than landing in a buffer nobody reads. Not on Windows:
/// WinSock drops the receive queue on a reset, so there the client's read of the wrong
/// answer fails first and the call is `Io`, a platform rendering of the reset
/// rather than a client defect.
#[cfg(not(windows))]
#[tokio::test]
async fn stdlib_server_answering_wrong_then_resetting_is_authentication_every_time() {
    every_round_is_authentication("rst").await;
}

/// An honest server that resets (`SO_LINGER 0`) once it has read the request: the
/// client's reply read meets ECONNRESET, with no FIN ahead of it to race, so every
/// round is `Io` of kind `ConnectionReset`.
#[cfg(not(windows))]
#[tokio::test]
async fn stdlib_server_resetting_after_the_request_is_io_connection_reset_every_time() {
    let server = PythonServer::spawn("honest_rst");
    let key = RpcKey::from_hex(KEY_HEX).expect("key");
    for round in 0..ROUNDS {
        let started = Instant::now();
        let result = interface_stats(&server.endpoint, &key, CALL).await;
        let elapsed = started.elapsed();
        match &result {
            Err(RpcError::Io(err)) => assert_eq!(
                err.kind(),
                std::io::ErrorKind::ConnectionReset,
                "round {round}: {err:?}"
            ),
            other => panic!("round {round}: expected Io(ConnectionReset), got {other:?}"),
        }
        assert!(elapsed < CALL, "round {round}: {elapsed:?} ran to the budget");
    }
}
