//! A real `reticulumd` process booted with one `type = "auto"` interface answers
//! `daemon_status_ex` with the AutoInterface status handle's `"peers"` key copied into
//! `interfaces[].settings._runtime.auto.carrier_runtime`: present and `0` while nobody has
//! been admitted (the group id is unique to this run, so no LAN peer can pair), and the
//! same on the next refresh tick. The in-crate pin in `bootstrap_parts/module_core.rs`
//! drives the refresher by hand; this probe drives the daemon the way a consumer does,
//! over HTTP RPC.

#![cfg(unix)]

use rns_rpc::rpc::{codec, RpcRequest, RpcResponse};
use serde_json::Value;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const INTERFACE_NAME: &str = "auto-peers-probe";

/// A daemon booted for one test: its storage, config and the RPC port it listens on.
struct ProbeDaemon {
    // Fields drop in declaration order: the daemon must die before its storage directory goes.
    child: Daemon,
    _temp: tempfile::TempDir,
    rpc_port: u16,
}

fn boot_lonely_auto_daemon() -> ProbeDaemon {
    let temp = tempfile::tempdir().expect("temporary daemon storage");
    let storage = temp.path().join("storage");
    std::fs::create_dir_all(&storage).expect("create storage directory");
    let config = temp.path().join("reticulum.toml");
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).expect("clock").as_nanos();
    let group_id = format!("auto-peers-probe-{}-{nanos}", std::process::id());
    let (discovery_port, data_port) = reserve_two_udp_ports();
    std::fs::write(
        &config,
        format!(
            "[reticulum]\nenable_transport = true\n\n[[interfaces]]\ntype = \"auto\"\n\
             enabled = true\nname = \"{INTERFACE_NAME}\"\ngroup_id = \"{group_id}\"\n\
             discovery_port = {discovery_port}\ndata_port = {data_port}\n"
        ),
    )
    .expect("write the daemon config");
    let (transport_port, rpc_port) = reserve_two_tcp_ports();
    let child = start_daemon(temp.path(), &storage, &config, transport_port, rpc_port);
    ProbeDaemon { child, _temp: temp, rpc_port }
}

/// Polls `daemon_status_ex` until the auto interface's `carrier_runtime` object is
/// present, failing fast if the interface reports `startup_status = "failed"`.
fn wait_for_carrier_runtime(daemon: &mut ProbeDaemon) -> Value {
    let rpc_port = daemon.rpc_port;
    wait_until(&mut daemon.child, "auto carrier_runtime in daemon_status_ex", || {
        let status = rpc_status(rpc_port).ok()?;
        let record = auto_record(&status)?;
        let startup = record["settings"]["_runtime"]["startup_status"].as_str();
        assert_ne!(
            startup,
            Some("failed"),
            "the auto interface failed to start on this host (environment, not the peers key): {}",
            record["settings"]["_runtime"]
        );
        record["settings"]["_runtime"]["auto"]["carrier_runtime"].as_object()?;
        Some(status)
    })
}

#[test]
fn daemon_status_ex_carries_the_auto_peers_key_at_zero_for_a_lonely_auto_interface() {
    let mut daemon = boot_lonely_auto_daemon();
    let status = wait_for_carrier_runtime(&mut daemon);

    let record = auto_record(&status).expect("auto interface record");
    let carrier_runtime = &record["settings"]["_runtime"]["auto"]["carrier_runtime"];
    assert!(
        carrier_runtime.get("peers").is_some(),
        "daemon_status_ex carrier_runtime lacks the peers key: {carrier_runtime}"
    );
    assert!(
        !carrier_runtime["peers"].is_null(),
        "the peers key is always present and never null: {carrier_runtime}"
    );
    assert_eq!(
        carrier_runtime["peers"].as_u64(),
        Some(0),
        "a lonely AutoInterface reports zero peers: {carrier_runtime}"
    );
    assert!(
        carrier_runtime.get("last_peer_job").is_some(),
        "last_peer_job sibling missing: {carrier_runtime}"
    );

    // The refresher ticks every 2 s; the key is a steady reading of the counter, not a
    // one-shot.
    thread::sleep(Duration::from_millis(2300));
    let again = rpc_status(daemon.rpc_port).expect("second daemon_status_ex");
    let again = auto_record(&again).expect("auto interface record on the second read");
    let again = &again["settings"]["_runtime"]["auto"]["carrier_runtime"];
    assert_eq!(
        again["peers"].as_u64(),
        Some(0),
        "a lonely AutoInterface still reports zero peers a tick later: {again}"
    );
}

fn auto_record(status: &Value) -> Option<&Value> {
    status["interfaces"]
        .as_array()?
        .iter()
        .find(|record| record["name"].as_str() == Some(INTERFACE_NAME))
}

fn reserve_two_tcp_ports() -> (u16, u16) {
    let transport_listener = TcpListener::bind(("127.0.0.1", 0)).expect("reserve transport port");
    let rpc_listener = TcpListener::bind(("127.0.0.1", 0)).expect("reserve RPC port");
    let transport_port = transport_listener.local_addr().expect("read transport port").port();
    let rpc_port = rpc_listener.local_addr().expect("read RPC port").port();
    drop((transport_listener, rpc_listener));
    (transport_port, rpc_port)
}

fn reserve_two_udp_ports() -> (u16, u16) {
    let data = UdpSocket::bind(("127.0.0.1", 0)).expect("reserve data port");
    let data_port = data.local_addr().expect("read data port").port();
    // The daemon derives the unicast discovery port as `discovery_port + 1`, so that
    // port must exist and must not be the data port, which the daemon binds plainly.
    let (discovery, discovery_port) = loop {
        let discovery = UdpSocket::bind(("127.0.0.1", 0)).expect("reserve discovery port");
        let port = discovery.local_addr().expect("read discovery port").port();
        if port != u16::MAX && port + 1 != data_port {
            break (discovery, port);
        }
    };
    drop((discovery, data));
    (discovery_port, data_port)
}

struct Daemon(Child);

impl Drop for Daemon {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

fn start_daemon(
    root: &Path,
    storage: &Path,
    config: &Path,
    transport_port: u16,
    rpc_port: u16,
) -> Daemon {
    let child = Command::new(env!("CARGO_BIN_EXE_reticulumd"))
        .arg("--db")
        .arg(storage.join("reticulum.db"))
        .arg("--identity")
        .arg(storage.join("identity"))
        .arg("--config")
        .arg(config)
        .arg("--transport")
        .arg(format!("127.0.0.1:{transport_port}"))
        .arg("--rpc")
        .arg(format!("127.0.0.1:{rpc_port}"))
        .arg("--rpc-unix")
        .arg(root.join(format!("rpc-{rpc_port}.sock")))
        .env("RUST_LOG", "info")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn standalone reticulumd process");
    Daemon(child)
}

fn wait_until<F>(child: &mut Daemon, description: &str, mut probe: F) -> Value
where
    F: FnMut() -> Option<Value>,
{
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if let Some(value) = probe() {
            return value;
        }
        if let Ok(Some(status)) = child.0.try_wait() {
            panic!("daemon exited before {description}: {status}");
        }
        assert!(Instant::now() < deadline, "timed out waiting for {description}");
        thread::sleep(Duration::from_millis(100));
    }
}

fn rpc_status(port: u16) -> std::io::Result<Value> {
    let address = SocketAddr::from(([127, 0, 0, 1], port));
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_millis(250))?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    let body = codec::encode_frame(&RpcRequest {
        id: 1,
        method: "daemon_status_ex".to_owned(),
        params: None,
    })?;
    write!(
        stream,
        "POST /rpc HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/msgpack\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    stream.write_all(&body)?;
    let mut response = Vec::new();
    stream.read_to_end(&mut response)?;
    let body_start = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|offset| offset + 4)
        .ok_or_else(|| std::io::Error::other("RPC response omitted HTTP header boundary"))?;
    let decoded: RpcResponse = codec::decode_frame(&response[body_start..])?;
    decoded.result.ok_or_else(|| {
        std::io::Error::other(format!("daemon status RPC returned error: {:?}", decoded.error))
    })
}
