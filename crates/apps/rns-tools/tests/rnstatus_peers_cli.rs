//! rnstatus-rs renders the AutoInterface `peers` key as `peers=N` on the
//! auto line when `_runtime.auto.carrier_runtime` carries it, including `peers=0`, and
//! leaves the token off the auto line when a daemon without the key answers.
//! Driven black-box: a mock `daemon_status_ex` TCP RPC server and the built binary.
//! Lives beside `rnstatus_cli.rs`, which sits at the module-size limit.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::Command;
use std::thread;

use rns_rpc::rpc::codec;
use rns_rpc::RpcResponse;
use serde_json::{json, Value};

/// One auto interface whose carrier runtime carries the given extra keys on top of the
/// counters the summary line always renders.
fn daemon_status_with_auto_runtime(extra: Value) -> Value {
    let mut carrier_runtime = json!({
        "online": true,
        "final_init_done": true,
        "carrier_changed": false,
        "carrier_event_count": 0,
        "adopted_device_count": 1,
        "adopted_add_count": 1,
        "adopted_remove_count": 0,
        "link_local_replacement_count": 0
    });
    if let (Some(target), Some(source)) = (carrier_runtime.as_object_mut(), extra.as_object()) {
        for (key, value) in source {
            target.insert(key.clone(), value.clone());
        }
    }
    json!({
        "identity_hash": "0123456789abcdef0123456789abcdef",
        "running": true,
        "interface_count": 1,
        "interfaces": [
            {
                "name": "auto-probe",
                "type": "auto",
                "enabled": true,
                "settings": {
                    "_runtime": {
                        "startup_status": "spawned",
                        "auto": { "carrier_runtime": carrier_runtime }
                    }
                }
            }
        ]
    })
}

/// Serves one `daemon_status_ex` answer and runs `rnstatus-rs --rpc` against it,
/// returning stdout.
fn rnstatus_human_output(result: Value) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock rpc");
    let rpc = listener.local_addr().expect("mock rpc addr").to_string();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept rpc request");
        let mut request = Vec::new();
        stream.read_to_end(&mut request).expect("read rpc request");
        let body = http_body(&request);
        let rpc_request = codec::decode_frame::<rns_rpc::RpcRequest>(body).expect("decode request");
        assert_eq!(rpc_request.method, "daemon_status_ex");
        let response = RpcResponse { id: rpc_request.id, result: Some(result), error: None };
        let body = codec::encode_frame(&response).expect("encode response");
        let headers = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/msgpack\r\nContent-Length: {}\r\n\r\n",
            body.len()
        );
        stream.write_all(headers.as_bytes()).expect("write response headers");
        stream.write_all(&body).expect("write response body");
    });

    let output = Command::new(env!("CARGO_BIN_EXE_rnstatus-rs"))
        .arg("--rpc")
        .arg(rpc)
        .output()
        .expect("run rnstatus-rs");
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    server.join().expect("mock rpc server");
    String::from_utf8(output.stdout).expect("utf8 stdout")
}

/// The one line of the human rendering that summarises the auto runtime.
fn auto_line(stdout: &str) -> &str {
    stdout
        .lines()
        .find(|line| line.contains("auto online="))
        .unwrap_or_else(|| panic!("no auto summary line in stdout: {stdout}"))
}

/// `peers=N` is printed whenever the key is present, so a runtime that has admitted
/// nobody yet shows `peers=0` rather than dropping the token, in the slot between
/// `replaced=` and the peer-data counters.
#[test]
fn rnstatus_prints_peers_zero_when_the_key_is_present_with_no_peers() {
    let stdout = rnstatus_human_output(daemon_status_with_auto_runtime(json!({
        "peers": 0,
        "peer_data_admitted_count": 0
    })));
    let line = auto_line(&stdout);
    assert!(line.contains("replaced=0 peers=0 peer_data_admitted=0"), "auto line: {line}");
}

/// A daemon that predates the key (no `peers` in `carrier_runtime`) renders the auto
/// line without any `peers=` token, so the rest of the summary is unchanged for an
/// older reticulumd.
#[test]
fn rnstatus_omits_the_peers_token_when_the_daemon_has_no_peers_key() {
    let stdout = rnstatus_human_output(daemon_status_with_auto_runtime(json!({})));
    let line = auto_line(&stdout);
    assert!(line.contains("added=1 removed=0 replaced=0"), "auto line: {line}");
    assert!(!line.contains("peers="), "no peers token expected on the auto line: {line}");
}

/// `peers=N` follows the key's value as a plain integer.
#[test]
fn rnstatus_prints_the_peer_count_the_daemon_reports() {
    let stdout = rnstatus_human_output(daemon_status_with_auto_runtime(json!({ "peers": 7 })));
    let line = auto_line(&stdout);
    assert!(line.contains("replaced=0 peers=7"), "auto line: {line}");
}

fn http_body(request: &[u8]) -> &[u8] {
    let marker = b"\r\n\r\n";
    let start = request
        .windows(marker.len())
        .position(|window| window == marker)
        .map(|index| index + marker.len())
        .expect("request headers");
    &request[start..]
}
