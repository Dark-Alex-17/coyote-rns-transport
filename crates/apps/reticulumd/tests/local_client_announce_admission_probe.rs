//! Announce ingress admission on a REAL `reticulumd` shared instance, exercised the way a
//! colocated program sees it: the daemon binary boots from an empty storage directory with
//! one `LocalInterface` TCP listener on a free loopback port and `[reticulum]
//! enable_transport = true` (rnsd's topology), and Rust transports attach to it with
//! `SharedInstanceOptions::default()`.
//!
//! - Two local clients announce two different destinations 100 ms apart, each with no packet
//!   context; a third local client, attached before either, learns both well inside 5 s. The
//!   announces cross two per-client children of the daemon and the learner's own interface,
//!   all of which are shared-instance interfaces that never ingress-limit
//!   (`RNS/Interfaces/LocalInterface.py:130`); a hold would last 30 s under the refreshed
//!   defaults, six minutes under the old ones.
//! - One local client then announces three more destinations back to back on ONE child and
//!   the third client learns all three inside 5 s.
//! - The daemon's own log (`RUST_LOG=debug`) carries no `holding announce` record over the
//!   whole run, so neither of its per-client children held anything.
//!
//! `tests/announce_admission_usage_probe.rs` in the transport crate proves the same against a
//! Rust `TcpServer` flagged in-process; this one proves the daemon flags the listener it
//! builds from its own config, so the exemption reaches reticulumd's users.

#![cfg(unix)]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use rns_rpc::rpc::{codec, RpcRequest, RpcResponse};
use rns_transport::destination::{DestinationName, SingleInputDestination};
use rns_transport::hash::AddressHash;
use rns_transport::identity::PrivateIdentity;
use rns_transport::rand_core::OsRng;
use rns_transport::transport::{
    AnnounceEvent, SharedInstanceEndpoint, SharedInstanceOptions, Transport, TransportConfig,
};
use serde_json::Value;
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;
use tokio::time::timeout;

const APP_NAME: &str = "reticulumd-announce-admission-probe";
const DAEMON_START: Duration = Duration::from_secs(30);
const WAIT: Duration = Duration::from_secs(15);
/// All announces must be learned inside this; a held one surfaces no sooner than 30 s.
const PROMPT: Duration = Duration::from_secs(5);
const BACK_TO_BACK: Duration = Duration::from_millis(100);

struct Daemon {
    child: Child,
    log: Arc<Mutex<String>>,
}

impl Daemon {
    fn log_snapshot(&self) -> String {
        self.log.lock().expect("daemon log store").clone()
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

fn reserve_ports(count: usize) -> Vec<u16> {
    let listeners: Vec<TcpListener> = (0..count)
        .map(|_| TcpListener::bind("127.0.0.1:0").expect("reserve a loopback port"))
        .collect();
    listeners
        .iter()
        .map(|listener| listener.local_addr().expect("reserved port address").port())
        .collect()
}

/// Follows `reader` line by line into `log`.
fn follow(reader: impl Read + Send + 'static, log: Arc<Mutex<String>>) {
    thread::spawn(move || {
        for line in BufReader::new(reader).lines().map_while(Result::ok) {
            let mut log = log.lock().expect("daemon log store");
            log.push_str(&line);
            log.push('\n');
        }
    });
}

/// Boots `reticulumd` from `root` with one `LocalInterface` TCP listener on `local_port`,
/// its config written the way `tools/scripts/local-interface-smoke.sh` writes it.
fn start_daemon(root: &Path, local_port: u16, rpc_port: u16) -> Daemon {
    let config = root.join("reticulum.toml");
    std::fs::write(
        &config,
        format!(
            "[reticulum]\nenable_transport = true\n\n[[interfaces]]\ntype = \"LocalInterface\"\n\
             enabled = true\nname = \"probe-local-listener\"\nshared_instance_type = \"tcp\"\n\
             host = \"127.0.0.1\"\nport = {local_port}\nfixed_mtu = 262144\n\
             force_shared_instance_bitrate = 1000000\n"
        ),
    )
    .expect("write the daemon config");
    let mut child = Command::new(env!("CARGO_BIN_EXE_reticulumd"))
        .arg("--rpc")
        .arg(format!("127.0.0.1:{rpc_port}"))
        .arg("--rpc-unix")
        .arg(root.join("rpc.sock"))
        .arg("--db")
        .arg(root.join("reticulum.db"))
        .arg("--identity")
        .arg(root.join("identity"))
        .arg("--config")
        .arg(&config)
        .arg("--strict-interface-startup")
        .env("RUST_LOG", "debug")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn reticulumd");
    let log = Arc::new(Mutex::new(String::new()));
    follow(child.stdout.take().expect("daemon stdout"), Arc::clone(&log));
    follow(child.stderr.take().expect("daemon stderr"), Arc::clone(&log));
    Daemon { child, log }
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

/// Waits until the daemon answers its status RPC (startup finished, the listener flagged) and
/// the local listener accepts a TCP connect.
async fn wait_for_daemon(daemon: &mut Daemon, local_port: u16, rpc_port: u16) {
    let deadline = Instant::now() + DAEMON_START;
    loop {
        if let Ok(Some(status)) = daemon.child.try_wait() {
            panic!(
                "reticulumd exited before it was ready: {status}\n--- daemon log ---\n{}",
                daemon.log_snapshot()
            );
        }
        let rpc_ready = rpc_status(rpc_port).is_ok();
        let listening = TcpStream::connect_timeout(
            &SocketAddr::from(([127, 0, 0, 1], local_port)),
            Duration::from_millis(250),
        )
        .is_ok();
        if rpc_ready && listening {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "reticulumd did not come up within {DAEMON_START:?}\n--- daemon log ---\n{}",
            daemon.log_snapshot()
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

fn hold_records(log: &str) -> Vec<String> {
    log.lines().filter(|line| line.contains("holding announce")).map(str::to_owned).collect()
}

/// A local client of the daemon attached with `SharedInstanceOptions::default()`, its announce
/// receiver subscribed before the attach.
#[deny(clippy::await_holding_invalid_type)]
async fn local_client(
    name: &str,
    local_port: u16,
) -> (Transport, broadcast::Receiver<AnnounceEvent>) {
    let transport =
        Transport::new(TransportConfig::new(name, &PrivateIdentity::new_from_rand(OsRng), false));
    let announces = transport.recv_announces().await;
    transport
        .attach_shared_instance(
            SharedInstanceEndpoint::Tcp(SocketAddr::from(([127, 0, 0, 1], local_port))),
            SharedInstanceOptions::default(),
        )
        .await
        .expect("the attach to reticulumd is accepted");
    (transport, announces)
}

#[deny(clippy::await_holding_invalid_type)]
async fn register_destinations(
    transport: &Transport,
    tag: &str,
    count: usize,
) -> Vec<(Arc<tokio::sync::Mutex<SingleInputDestination>>, AddressHash)> {
    let mut out = Vec::with_capacity(count);
    for idx in 0..count {
        let destination = transport
            .add_destination(
                PrivateIdentity::new_from_rand(OsRng),
                DestinationName::new(APP_NAME, &format!("{tag}{idx}")),
            )
            .await;
        let hash = destination.lock().await.desc.address_hash;
        out.push((destination, hash));
    }
    out
}

/// Announces with no packet context, so every limiter on the path weighs it.
#[deny(clippy::await_holding_invalid_type)]
async fn announce(
    transport: &Transport,
    destination: &Arc<tokio::sync::Mutex<SingleInputDestination>>,
) {
    let packet = destination.lock().await.announce(OsRng, None).expect("announce packet");
    transport.send_packet(packet).await;
}

/// Waits up to `within` for every hash in `expected`, in any order; returns the ones missing.
#[deny(clippy::await_holding_invalid_type)]
async fn learn_all(
    announces: &mut broadcast::Receiver<AnnounceEvent>,
    expected: &[AddressHash],
    within: Duration,
) -> Vec<AddressHash> {
    let mut missing: Vec<AddressHash> = expected.to_vec();
    let deadline = Instant::now() + within;
    while !missing.is_empty() {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        match timeout(remaining, announces.recv()).await {
            Ok(Ok(event)) => {
                let address_hash = event.destination.lock().await.desc.address_hash;
                if let Some(idx) = missing.iter().position(|hash| *hash == address_hash) {
                    eprintln!("learned {address_hash}");
                    missing.swap_remove(idx);
                }
            }
            Ok(Err(RecvError::Lagged(_))) => continue,
            Ok(Err(RecvError::Closed)) => panic!("announce channel closed"),
            Err(_elapsed) => break,
        }
    }
    missing
}

#[deny(clippy::await_holding_invalid_type)]
async fn stop_all(transports: Vec<Transport>) {
    for transport in transports {
        timeout(WAIT, transport.detach_shared_instance()).await.expect("detach completes");
        timeout(WAIT, transport.stop()).await.expect("stop completes");
    }
}

fn stop_daemon(mut daemon: Daemon) -> String {
    let status = Command::new("kill")
        .args(["-INT", &daemon.child.id().to_string()])
        .status()
        .expect("send SIGINT to the daemon");
    assert!(status.success(), "failed to signal the daemon with SIGINT");
    let deadline = Instant::now() + WAIT;
    loop {
        if let Some(status) = daemon.child.try_wait().expect("wait for the daemon's exit") {
            assert!(status.success(), "the daemon failed during shutdown: {status}");
            break;
        }
        assert!(Instant::now() < deadline, "the daemon did not exit within {WAIT:?} of SIGINT");
        thread::sleep(Duration::from_millis(50));
    }
    daemon.log_snapshot()
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn reticulumd_local_clients_announcing_back_to_back_are_learned_in_full() {
    let root = tempfile::tempdir().expect("disposable daemon storage");
    let ports = reserve_ports(2);
    let (local_port, rpc_port) = (ports[0], ports[1]);
    let mut daemon = start_daemon(root.path(), local_port, rpc_port);
    wait_for_daemon(&mut daemon, local_port, rpc_port).await;

    // The learner attaches first: a shared instance relays a local client's announce only
    // to the clients attached when it arrives (`RNS/Transport.py:2400-2429`).
    let (learner, mut learner_announces) = local_client("probe-rd-learner", local_port).await;
    let (a, _a_announces) = local_client("probe-rd-a", local_port).await;
    let (b, _b_announces) = local_client("probe-rd-b", local_port).await;

    // (a) two clients, two children: one destination each, announced 100 ms apart.
    let a_destinations = register_destinations(&a, "a", 1).await;
    let b_destinations = register_destinations(&b, "b", 1).await;
    announce(&a, &a_destinations[0].0).await;
    tokio::time::sleep(BACK_TO_BACK).await;
    announce(&b, &b_destinations[0].0).await;
    let started = Instant::now();
    let missing =
        learn_all(&mut learner_announces, &[a_destinations[0].1, b_destinations[0].1], PROMPT)
            .await;
    assert!(
        missing.is_empty(),
        "the third local client of reticulumd did not learn {missing:?} within {PROMPT:?} after \
         two local clients announced 100 ms apart; daemon hold records: {:?}",
        hold_records(&daemon.log_snapshot())
    );
    eprintln!("both destinations learned {:?} after the second announce", started.elapsed());

    // (e) one client, one child: three destinations back to back.
    let burst = register_destinations(&a, "burst", 3).await;
    for (destination, _hash) in &burst {
        announce(&a, destination).await;
    }
    let hashes: Vec<AddressHash> = burst.iter().map(|(_, hash)| *hash).collect();
    let started = Instant::now();
    let missing = learn_all(&mut learner_announces, &hashes, PROMPT).await;
    assert!(
        missing.is_empty(),
        "the learner did not learn {} of three back-to-back destinations within {PROMPT:?} \
         ({missing:?}); daemon hold records: {:?}",
        missing.len(),
        hold_records(&daemon.log_snapshot())
    );
    eprintln!("all three burst destinations learned {:?} after the burst", started.elapsed());
    for (idx, hash) in hashes.iter().enumerate() {
        assert!(
            learner.destination_desc(hash).await.is_some(),
            "the learner resolves burst{idx} after the burst"
        );
    }

    let accepted_record = format!("[announce-debug] accepted dst={}", a_destinations[0].1);
    drop(a_destinations);
    drop(b_destinations);
    drop(burst);
    stop_all(vec![a, b, learner]).await;
    let log = stop_daemon(daemon);
    // A debug record from the announce path proves the debug level reached the log, so the
    // absence of a hold record below means something.
    assert!(
        log.contains(&accepted_record),
        "the daemon log carries no {accepted_record:?} record\n--- daemon log ---\n{log}"
    );
    let holds = hold_records(&log);
    assert!(holds.is_empty(), "reticulumd held an announce during the run: {holds:?}");
}
