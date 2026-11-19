// The shared-instance RPC client held to Python's own view of a live rnsd
// (`RNS/Reticulum.py:1274-1334`, `:1397-1402`, `:1501-1503`, `:1527-1566`),
// on the rnsd fixture of `python_shared_instance_rpc.rs`.
//
// Covered here and not by the tests of that part:
//  * an independent reference: Python's own `multiprocessing.connection.Client`
//    (the client rnstatus uses, `RNS/Reticulum.py:1397-1402`) reads the same
//    listener with the same identity-derived key and must see the same
//    interface set, kinds, online flags, peer counts and client counts;
//  * typed-vs-raw consistency: `kind`/`online`/`peers` are projections of the
//    per-interface map's `type`/`status`/`peers`, the `LocalServerInterface`
//    map carries no `peers` key (`:1501-1503`), `hash` is bytes, and the
//    top-level `interfaces` array has one map per typed entry;
//  * the read is idempotent: three back-to-back calls agree on the interface
//    set (each call is a fresh connection, the server closes after one reply);
//  * a zero budget returns at once, never a hang: the whole call runs under
//    the one `Duration`, so the answer is `RpcError::Timeout`, or the reply
//    itself when the loopback exchange completes before the timer's first
//    tick (`tokio::time::timeout` polls the call before the deadline on
//    every wake).

const SHARED_INSTANCE_RPC_PROBE_PYTHON_CLIENT: &str = r#"
import json, sys
import multiprocessing.connection as mpc
import RNS
from RNS.vendor import umsgpack as mp
identity_file, port = sys.argv[1], int(sys.argv[2])
with open(identity_file, "rb") as fh:
    key = RNS.Identity.full_hash(fh.read())
conn = mpc.Client(("127.0.0.1", port), family="AF_INET", authkey=key)
conn.send_bytes(mp.packb({"get": "interface_stats"}))
reply = mp.unpackb(conn.recv_bytes())
conn.close()
out = []
for i in reply["interfaces"]:
    out.append({
        "type": i["type"],
        "short_name": i["short_name"],
        "name": i["name"],
        "status": bool(i["status"]),
        "peers": (i["peers"] if "peers" in i else "absent"),
        "clients": i.get("clients"),
        "hash_is_bytes": isinstance(i["hash"], (bytes, bytearray)),
    })
out.sort(key=lambda e: e["short_name"])
print(json.dumps(out))
"#;

// Python's stdlib client against the same listener: the independent view
// the Rust decode is held to.
fn shared_instance_rpc_probe_python_view(
    identity_file: &std::path::Path,
    control_port: u16,
) -> Vec<serde_json::Value> {
    let paths = python_channel_interop_paths();
    let output = std::process::Command::new(paths.python_bin())
        .arg("-c")
        .arg(SHARED_INSTANCE_RPC_PROBE_PYTHON_CLIENT)
        .arg(identity_file)
        .arg(control_port.to_string())
        .env("PYTHONPATH", paths.reticulum_py_repo())
        .stdin(std::process::Stdio::null())
        .output()
        .expect("run the python rpc client");
    assert!(
        output.status.success(),
        "python client failed: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout.lines().last().expect("python client json line");
    serde_json::from_str::<Vec<serde_json::Value>>(line).expect("python client json")
}

fn shared_instance_rpc_probe_sorted(stats: &InterfaceStats) -> Vec<&InterfaceStat> {
    let mut sorted: Vec<&InterfaceStat> = stats.interfaces.iter().collect();
    sorted.sort_by(|a, b| a.short_name.cmp(&b.short_name));
    sorted
}

fn shared_instance_rpc_probe_raw_has_key(raw: &rmpv::Value, key: &str) -> bool {
    raw.as_map().expect("per-interface map").iter().any(|(k, _)| k.as_str() == Some(key))
}

/// The Rust typed view equals Python's own stdlib-client view of the same
/// live listener, entry for entry, and the typed fields are projections of
/// the raw map: `kind` is `type`, `online` is `status`, `peers` is the
/// `peers` key where present (`RNS/Reticulum.py:1501-1503`, `:1527-1566`)
/// and the `LocalServerInterface` map carries no `peers` key at all.
#[tokio::test]
#[ignore = "requires local Python Reticulum checkout"]
async fn probe_python_rnsd_shared_instance_rpc_matches_pythons_own_client_view() {
    let _interop_guard = python_interop_guard().await;
    let temp = tempfile::tempdir().expect("tempdir");
    let instance = live_tcp_shared_instance();
    let control_port = free_tcp_port();
    let mut rnsd = SharedInstanceRpcRnsd::spawn(
        &instance,
        control_port,
        &temp.path().join("probe-rnsd-shared-instance-rpc-python-view-rnsd"),
    )
    .await;
    let endpoint = RpcEndpoint::Tcp(SocketAddr::from(([127, 0, 0, 1], control_port)));
    let key = rnsd.identity_key().await;

    let stats = rnsd.poll_interface_stats(&endpoint, &key).await.expect("first reply");
    let python_view = shared_instance_rpc_probe_python_view(&rnsd.identity_file, control_port);
    eprintln!("python client view: {}", serde_json::Value::Array(python_view.clone()));
    let rust = shared_instance_rpc_probe_sorted(&stats);
    assert_eq!(rust.len(), python_view.len(), "rust={rust:?}\npython={python_view:?}");
    assert_eq!(rust.len(), 2, "exactly the shared instance server and the AutoInterface: {rust:?}");

    for (stat, py) in rust.iter().zip(python_view.iter()) {
        assert_eq!(stat.kind, py["type"].as_str().expect("type"), "{stat:?} vs {py}");
        assert_eq!(
            stat.short_name,
            py["short_name"].as_str().expect("short_name"),
            "{stat:?} vs {py}"
        );
        assert_eq!(stat.name, py["name"].as_str().expect("name"), "{stat:?} vs {py}");
        assert_eq!(stat.online, py["status"].as_bool().expect("status"), "{stat:?} vs {py}");
        let py_peers = match &py["peers"] {
            serde_json::Value::String(s) if s == "absent" => None,
            serde_json::Value::Null => None,
            other => Some(other.as_u64().expect("peers int")),
        };
        assert_eq!(stat.peers, py_peers, "{stat:?} vs {py}");
        assert_eq!(stat.clients, py["clients"].as_u64(), "{stat:?} vs {py}");
        assert!(py["hash_is_bytes"].as_bool().expect("hash_is_bytes"), "{py}");

        // the typed fields are projections of the raw map
        assert_eq!(stat.raw["type"].as_str(), Some(stat.kind.as_str()), "{stat:?}");
        assert_eq!(stat.raw["status"].as_bool(), Some(stat.online), "{stat:?}");
        assert!(stat.raw["hash"].as_slice().is_some(), "hash should be msgpack bin: {stat:?}");
        assert_eq!(stat.raw["rxb"].as_u64(), Some(stat.rxb), "{stat:?}");
        assert_eq!(stat.raw["txb"].as_u64(), Some(stat.txb), "{stat:?}");
        match stat.kind.as_str() {
            "AutoInterface" => {
                assert_eq!(stat.raw["peers"].as_u64(), Some(0), "{stat:?}");
                assert_eq!(py["peers"], serde_json::json!(0), "{py}");
            }
            "LocalServerInterface" => {
                assert!(
                    !shared_instance_rpc_probe_raw_has_key(&stat.raw, "peers"),
                    "LocalServerInterface has no `peers` attribute, so no key: {stat:?}"
                );
                assert_eq!(py["peers"], serde_json::json!("absent"), "{py}");
            }
            other => panic!("unexpected interface kind {other}: {stat:?}"),
        }
    }

    let top = stats.raw["interfaces"].as_array().expect("top-level interfaces array");
    assert_eq!(top.len(), stats.interfaces.len(), "{:?}", stats.raw);
}

/// Reading is idempotent and bounded: three back-to-back calls (each a
/// fresh connection, the server closing after one reply,
/// `RNS/Reticulum.py:1274-1334`) agree on the interface set, and a zero
/// budget returns at once, as `RpcError::Timeout` or as the reply when the
/// loopback exchange beats the timer's first tick, rather than hanging.
#[tokio::test]
#[ignore = "requires local Python Reticulum checkout"]
async fn probe_python_rnsd_shared_instance_rpc_repeated_reads_agree_and_zero_budget_returns() {
    let _interop_guard = python_interop_guard().await;
    let temp = tempfile::tempdir().expect("tempdir");
    let instance = live_tcp_shared_instance();
    let control_port = free_tcp_port();
    let mut rnsd = SharedInstanceRpcRnsd::spawn(
        &instance,
        control_port,
        &temp.path().join("probe-rnsd-shared-instance-rpc-repeat-rnsd"),
    )
    .await;
    let endpoint = RpcEndpoint::Tcp(SocketAddr::from(([127, 0, 0, 1], control_port)));
    let key = rnsd.identity_key().await;

    let first = rnsd.poll_interface_stats(&endpoint, &key).await.expect("first reply");
    let view = |stats: &InterfaceStats| -> Vec<(String, String, bool, Option<u64>)> {
        shared_instance_rpc_probe_sorted(stats)
            .into_iter()
            .map(|s| (s.kind.clone(), s.short_name.clone(), s.online, s.peers))
            .collect()
    };
    let first_view = view(&first);
    for n in 2..=3 {
        let again = interface_stats(&endpoint, &key, SHARED_INSTANCE_RPC_CALL)
            .await
            .unwrap_or_else(|err| panic!("call {n} failed: {err:?}"));
        assert_eq!(view(&again), first_view, "call {n} disagrees with the first");
    }

    let started = tokio::time::Instant::now();
    let zero = interface_stats(&endpoint, &key, Duration::ZERO).await;
    match &zero {
        Err(RpcError::Timeout) => {}
        Ok(stats) => assert_eq!(view(stats), first_view, "a reply under a zero budget"),
        Err(other) => panic!("zero budget: expected Timeout or the reply, got {other:?}"),
    }
    assert!(started.elapsed() < Duration::from_secs(2), "zero budget took {:?}", started.elapsed());

    // The listener keeps serving after the abandoned dial.
    let after =
        rnsd.poll_interface_stats(&endpoint, &key).await.expect("serves after the timed-out dial");
    assert_eq!(view(&after), first_view);
}
