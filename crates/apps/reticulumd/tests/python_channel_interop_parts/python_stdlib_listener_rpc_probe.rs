// The shared-instance RPC client held to CPython's own
// `multiprocessing.connection.Listener`, the very class rnsd opens at
// `RNS/Reticulum.py:363-367`, run here on its own with a chosen `authkey` and
// a canned `get_interface_stats`-shaped reply, so the key length and the
// refusal path are exercised against the stdlib's `deliver_challenge` /
// `answer_challenge` (`connection.py:917-1006`) without an rnsd in between.
//
// Covered here and not by the rnsd-backed parts:
//  * the README's "the HMAC accepts a key of any length": a one-byte
//    `rpc_key` and a sixteen-byte one both pass the stdlib's mutual
//    handshake through `RpcKey::from_hex`, and the typed reply is read;
//  * a wrong key is `RpcError::Authentication` from the stdlib's own
//    `#FAILURE#`, and the listener goes on serving the next correct dial;
//  * the typed fields are projections of a reply whose shape is rnsd's:
//    `peers` present is `Some`, a nil `clients` is `None`, an absent `peers`
//    is `None`.
//  * the path calls reach CPython as `rpc_loop` reads them
//    (`RNS/Reticulum.py:1283-1306`): `max_hops` is present and `None` when
//    the caller passes none, the hash arrives as `bytes` (the key type of
//    `Transport.path_table`, `RNS/Transport.py:2459`, which umsgpack packs as
//    bin, `RNS/vendor/umsgpack.py:505-506`), and the canned replies read as
//    rnsd's shapes: `get_path_table` rows (`RNS/Reticulum.py:1638-1648`) with
//    the int `0` timestamp of a dropped path (`RNS/Transport.py:3219-3226`),
//    the str `"None"` for no next hop (`RNS/Reticulum.py:1715`), the int `6`
//    first-hop timeout, the bool of `drop path` and the count of `drop all_via`.

use rns_transport::transport::shared_instance_rpc::{
    drop_all_via, drop_path, drop_path_settled, first_hop_timeout, next_hop_interface_name,
    path_table,
};

const STDLIB_LISTENER_PYTHON_SERVER: &str = r#"
import json, sys
import multiprocessing.connection as mpc
from RNS.vendor import umsgpack as mp
key = bytes.fromhex(sys.argv[1])
listener = mpc.Listener(("127.0.0.1", 0), family="AF_INET", authkey=key)
print(json.dumps({"port": listener.address[1]}), flush=True)
reply = {"interfaces": [
    {"name": "AutoInterface[probe]", "short_name": "probe", "type": "AutoInterface",
     "status": True, "peers": 3, "clients": None, "rxb": 7, "txb": 11,
     "hash": b"\x01" * 32, "mode": 1, "bitrate": None},
    {"name": "Shared Instance[probe]", "short_name": "shared",
     "type": "LocalServerInterface", "status": True, "clients": 2,
     "rxb": 0, "txb": 0, "hash": b"\x02" * 32, "mode": 1, "bitrate": None},
]}
path_rows = [
    {"hash": b"\x11" * 16, "timestamp": 1700000000.25, "via": b"\x22" * 16, "hops": 1,
     "expires": 1700000600.5, "interface": "AutoInterface[probe]"},
    {"hash": b"\x33" * 16, "timestamp": 0, "via": b"\x22" * 16, "hops": 0,
     "expires": 1700000600.5, "interface": "AutoInterface[probe]"},
]
served = 0
refused = 0
while True:
    try:
        conn = listener.accept()
    except mpc.AuthenticationError as err:
        refused += 1
        print(json.dumps({"refused": refused, "error": str(err)}), flush=True)
        continue
    request = mp.unpackb(conn.recv_bytes())
    if request.get("get") == "interface_stats":
        conn.send_bytes(mp.packb(reply))
    elif request.get("get") == "path_table":
        print(json.dumps({"max_hops": request["max_hops"]}), flush=True)
        conn.send_bytes(mp.packb(path_rows))
    elif request.get("get") == "next_hop_if_name":
        conn.send_bytes(mp.packb("None"))
    elif request.get("get") == "first_hop_timeout":
        conn.send_bytes(mp.packb(6))
    elif request.get("drop") == "path":
        conn.send_bytes(mp.packb(isinstance(request["destination_hash"], bytes)))
    elif request.get("drop") == "all_via":
        conn.send_bytes(mp.packb(2))
    else:
        print(json.dumps({"unexpected": repr(request)}), flush=True)
    conn.close()
    served += 1
    print(json.dumps({"served": served}), flush=True)
"#;

const STDLIB_LISTENER_START: Duration = Duration::from_secs(20);
const STDLIB_LISTENER_CALL: Duration = Duration::from_secs(5);
const STDLIB_LISTENER_ONE_BYTE_KEY: &str = "ab";
const STDLIB_LISTENER_SIXTEEN_BYTE_KEY: &str = "0123456789abcdef0123456789abcdef";
const STDLIB_LISTENER_WRONG_KEY: &str = "ffeeddccbbaa99887766554433221100";

struct StdlibListener {
    _guard: ChildGuard,
    log: ProcessLogTail,
    endpoint: RpcEndpoint,
}

impl StdlibListener {
    #[deny(clippy::await_holding_invalid_type)]
    async fn spawn(key_hex: &str) -> Self {
        let paths = python_channel_interop_paths();
        let mut child = std::process::Command::new(paths.python_bin())
            .arg("-c")
            .arg(STDLIB_LISTENER_PYTHON_SERVER)
            .arg(key_hex)
            .env("PYTHONPATH", paths.reticulum_py_repo())
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("spawn the python listener");
        let log = ProcessLogTail::follow("python listener", &mut child);
        let guard = ChildGuard { child: Some(child) };
        let port = log
            .wait_for("the listener port", STDLIB_LISTENER_START, |line| {
                python_json_line(&["port"])(line).and_then(|v| v["port"].as_u64())
            })
            .await;
        let port = u16::try_from(port).expect("port fits");
        Self {
            _guard: guard,
            log,
            endpoint: RpcEndpoint::Tcp(SocketAddr::from(([127, 0, 0, 1], port))),
        }
    }

    #[deny(clippy::await_holding_invalid_type)]
    async fn wait_served(&self, n: u64) {
        self.log
            .wait_for(&format!("served={n}"), STDLIB_LISTENER_START, |line| {
                python_json_line(&["served"])(line).filter(|v| v["served"].as_u64() == Some(n))
            })
            .await;
    }

    #[deny(clippy::await_holding_invalid_type)]
    async fn wait_refused(&self, n: u64) -> String {
        self.log
            .wait_for(&format!("refused={n}"), STDLIB_LISTENER_START, |line| {
                python_json_line(&["refused"])(line)
                    .filter(|v| v["refused"].as_u64() == Some(n))
                    .map(|v| v["error"].as_str().unwrap_or_default().to_string())
            })
            .await
    }

    /// The `max_hops` the listener read from the last `path_table` request: `None` is
    /// Python's `None`, logged as JSON null.
    #[deny(clippy::await_holding_invalid_type)]
    async fn wait_max_hops(&self, expected: Option<u64>) {
        self.log
            .wait_for(&format!("max_hops={expected:?}"), STDLIB_LISTENER_START, |line| {
                python_json_line(&["max_hops"])(line).filter(|v| match expected {
                    Some(n) => v["max_hops"].as_u64() == Some(n),
                    None => v["max_hops"].is_null(),
                })
            })
            .await;
    }
}

fn stdlib_listener_assert_reply(stats: &InterfaceStats) {
    let mut sorted: Vec<&InterfaceStat> = stats.interfaces.iter().collect();
    sorted.sort_by(|a, b| a.short_name.cmp(&b.short_name));
    assert_eq!(sorted.len(), 2, "{stats:?}");
    let auto = sorted[0];
    assert_eq!(auto.kind, "AutoInterface");
    assert_eq!(auto.short_name, "probe");
    assert_eq!(auto.name, "AutoInterface[probe]");
    assert!(auto.online);
    assert_eq!(auto.peers, Some(3));
    assert_eq!(auto.clients, None, "a nil clients is None");
    assert_eq!(auto.rxb, 7);
    assert_eq!(auto.txb, 11);
    let shared = sorted[1];
    assert_eq!(shared.kind, "LocalServerInterface");
    assert_eq!(shared.peers, None, "an absent peers key is None");
    assert_eq!(shared.clients, Some(2));
    assert_eq!(stats.raw["interfaces"].as_array().map(Vec::len), Some(2), "{:?}", stats.raw);
}

/// A configured key of one byte and of sixteen bytes each pass CPython's own
/// listener handshake; a wrong key is the stdlib's `#FAILURE#` read as
/// `Authentication`, and the listener serves the next correct dial.
#[tokio::test]
#[ignore = "requires local Python Reticulum checkout"]
async fn probe_python_stdlib_listener_takes_any_key_length_and_refuses_a_wrong_key() {
    let _interop_guard = python_interop_guard().await;
    for key_hex in [STDLIB_LISTENER_ONE_BYTE_KEY, STDLIB_LISTENER_SIXTEEN_BYTE_KEY] {
        let listener = StdlibListener::spawn(key_hex).await;
        let key = RpcKey::from_hex(key_hex).expect("configured key");

        let stats = interface_stats(&listener.endpoint, &key, STDLIB_LISTENER_CALL)
            .await
            .unwrap_or_else(|err| panic!("key {key_hex}: first reply: {err:?}"));
        stdlib_listener_assert_reply(&stats);
        listener.wait_served(1).await;

        let wrong = RpcKey::from_hex(STDLIB_LISTENER_WRONG_KEY).expect("wrong key");
        let refused = interface_stats(&listener.endpoint, &wrong, STDLIB_LISTENER_CALL).await;
        assert!(
            matches!(refused, Err(RpcError::Authentication)),
            "key {key_hex}: wrong key: {refused:?}"
        );
        let error = listener.wait_refused(1).await;
        assert!(error.contains("digest received was wrong"), "key {key_hex}: {error}");

        let again = interface_stats(&listener.endpoint, &key, STDLIB_LISTENER_CALL)
            .await
            .unwrap_or_else(|err| panic!("key {key_hex}: serves after the refusal: {err:?}"));
        stdlib_listener_assert_reply(&again);
        listener.wait_served(2).await;
    }
}

/// The six path calls against CPython's own listener with the sixteen-byte
/// key. `path_table` carries `max_hops` as Python `None` for `None` and as
/// the int otherwise (the listener logs what it read), and its two canned
/// rows decode with the second row's int `0` timestamp read as `0.0`;
/// `next_hop_interface_name` reads the str `"None"` as `None`;
/// `first_hop_timeout` reads the int `6` as `6.0`; `drop_path` is the bin
/// pin, since the listener answers whether `destination_hash` arrived as
/// `bytes`; `drop_all_via`, handed the rows' next-hop transport hash, reads
/// the canned count; and `drop_path_settled` is
/// `Ok(true)` after the drop and one next-hop poll, two connections.
#[tokio::test]
#[ignore = "requires local Python Reticulum checkout"]
async fn probe_python_stdlib_listener_answers_shared_instance_rpc_path_calls_with_bytes_hashes() {
    let _interop_guard = python_interop_guard().await;
    let listener = StdlibListener::spawn(STDLIB_LISTENER_SIXTEEN_BYTE_KEY).await;
    let key = RpcKey::from_hex(STDLIB_LISTENER_SIXTEEN_BYTE_KEY).expect("configured key");
    let destination = AddressHash::new([0x11; 16]);
    let via = AddressHash::new([0x22; 16]);

    let rows = path_table(&listener.endpoint, &key, None, STDLIB_LISTENER_CALL)
        .await
        .expect("path_table without max_hops");
    listener.wait_max_hops(None).await;
    listener.wait_served(1).await;
    assert_eq!(rows.len(), 2, "{rows:?}");
    assert_eq!(rows[0].hash, destination);
    assert_eq!(rows[0].via, via);
    assert_eq!(rows[0].timestamp, 1_700_000_000.25);
    assert_eq!(rows[0].hops, 1);
    assert_eq!(rows[0].expires, 1_700_000_600.5);
    assert_eq!(rows[0].interface, "AutoInterface[probe]");
    assert_eq!(rows[1].hash, AddressHash::new([0x33; 16]));
    assert_eq!(rows[1].timestamp, 0.0);
    assert_eq!(rows[1].hops, 0);
    assert_eq!(rows[1].interface, "AutoInterface[probe]");

    let again = path_table(&listener.endpoint, &key, Some(1), STDLIB_LISTENER_CALL)
        .await
        .expect("path_table with max_hops");
    listener.wait_max_hops(Some(1)).await;
    listener.wait_served(2).await;
    assert_eq!(again, rows);

    let name =
        next_hop_interface_name(&listener.endpoint, &key, &destination, STDLIB_LISTENER_CALL)
            .await
            .expect("next_hop_interface_name");
    assert_eq!(name, None);
    listener.wait_served(3).await;

    let seconds = first_hop_timeout(&listener.endpoint, &key, &destination, STDLIB_LISTENER_CALL)
        .await
        .expect("first_hop_timeout");
    assert_eq!(seconds, 6.0);
    listener.wait_served(4).await;

    let dropped = drop_path(&listener.endpoint, &key, &destination, STDLIB_LISTENER_CALL)
        .await
        .expect("drop_path");
    assert!(dropped, "destination_hash did not arrive as bytes");
    listener.wait_served(5).await;

    let count = drop_all_via(&listener.endpoint, &key, &via, STDLIB_LISTENER_CALL)
        .await
        .expect("drop_all_via");
    assert_eq!(count, 2);
    listener.wait_served(6).await;

    let settled = drop_path_settled(&listener.endpoint, &key, &destination, STDLIB_LISTENER_CALL)
        .await
        .expect("drop_path_settled");
    assert!(settled);
    listener.wait_served(8).await;
}
