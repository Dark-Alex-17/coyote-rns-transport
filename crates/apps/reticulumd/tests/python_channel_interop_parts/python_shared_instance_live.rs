use std::net::SocketAddr;

use rns_transport::destination::DestinationDesc;
#[cfg(target_os = "linux")]
use rns_transport::iface::local::LocalUnixEndpoint;
use rns_transport::transport::{AnnounceEvent, SharedInstanceEndpoint, SharedInstanceOptions};

// Passed to the Python client on its command line so both sides name the
// same destination and the same request.
const SHARED_INSTANCE_LIVE_APP_NAME: &str = "test";
const SHARED_INSTANCE_LIVE_ASPECT: &str = "shared";
const SHARED_INSTANCE_LIVE_REQUEST_PATH: &str = "/test/request";
const SHARED_INSTANCE_LIVE_REQUEST_DATA: &str = "hello-rust";

// Drains a child's stdout and stderr into one buffer from the moment it is
// spawned, so RNS log output cannot fill a pipe and stall the child, and
// lets the test gate on a log line instead of sleeping. The whole buffer is
// printed when the test panics, after both drain threads have joined, so
// it holds everything the child wrote before its guard killed it.
struct ProcessLogTail {
    label: &'static str,
    lines: Arc<StdMutex<String>>,
    changed: Arc<tokio::sync::Notify>,
    drains: Vec<std::thread::JoinHandle<()>>,
}

impl ProcessLogTail {
    fn follow(label: &'static str, child: &mut std::process::Child) -> Self {
        let mut tail =
            Self { label, lines: Arc::default(), changed: Arc::default(), drains: Vec::new() };
        let stdout = tail.drain(child.stdout.take().expect("child stdout"));
        let stderr = tail.drain(child.stderr.take().expect("child stderr"));
        tail.drains = vec![stdout, stderr];
        tail
    }

    fn drain(&self, pipe: impl std::io::Read + Send + 'static) -> std::thread::JoinHandle<()> {
        let lines = self.lines.clone();
        let changed = self.changed.clone();
        std::thread::spawn(move || {
            use std::io::BufRead as _;
            for line in std::io::BufReader::new(pipe).lines() {
                let line = match line {
                    Ok(line) => line,
                    Err(err) if err.kind() == std::io::ErrorKind::InvalidData => continue,
                    Err(_) => break,
                };
                {
                    let mut lines = lines.lock().expect("log tail lock");
                    lines.push_str(&line);
                    lines.push('\n');
                }
                changed.notify_waiters();
            }
        })
    }

    fn snapshot(&self) -> String {
        self.lines.lock().expect("log tail lock").clone()
    }

    // The `Notified` future is created before the scan, so a line appended
    // between the scan and the await still wakes the loop.
    #[deny(clippy::await_holding_invalid_type)]
    async fn wait_for<T>(
        &self,
        what: &str,
        deadline: Duration,
        find: impl Fn(&str) -> Option<T>,
    ) -> T {
        tokio::time::timeout(deadline, async {
            loop {
                let changed = self.changed.notified();
                let found = self.snapshot().lines().find_map(&find);
                if let Some(found) = found {
                    return found;
                }
                changed.await;
            }
        })
        .await
        .unwrap_or_else(|_| panic!("timed out waiting for {what} in the {} output", self.label))
    }
}

// The child's guard is declared after the tail, so it has already killed
// and reaped the child by the time this runs and the drains see EOF.
impl Drop for ProcessLogTail {
    fn drop(&mut self) {
        for drain in self.drains.drain(..) {
            let _ = drain.join();
        }
        if std::thread::panicking() {
            eprintln!("{} output:\n{}", self.label, self.snapshot());
        }
    }
}

fn python_json_line<'a>(keys: &'a [&'a str]) -> impl Fn(&str) -> Option<serde_json::Value> + 'a {
    move |line| {
        serde_json::from_str::<serde_json::Value>(line)
            .ok()
            .filter(|value| keys.iter().any(|key| value.get(key).is_some()))
    }
}

// The rnsd config and the Python client's config carry the same
// `[reticulum]` keys: the client finds the endpoint busy and falls back to a
// local client of it (RNS/Reticulum.py:420-441). The loglevel is
// LOG_PATHING (RNS/__init__.py:66), the floor at which rnsd logs the path
// line the test gates on (RNS/Transport.py:2461).
struct LiveSharedInstance {
    instance_lines: String,
    endpoint: SharedInstanceEndpoint,
}

impl LiveSharedInstance {
    fn config(&self, control_port: u16) -> String {
        format!(
            "[reticulum]\n\
             enable_transport = no\n\
             share_instance = yes\n\
             {}\
             instance_control_port = {control_port}\n\
             discover_interfaces = no\n\
             \n\
             [logging]\n\
             loglevel = 7\n",
            self.instance_lines
        )
    }
}

fn live_tcp_shared_instance() -> LiveSharedInstance {
    let port = free_tcp_port();
    LiveSharedInstance {
        instance_lines: format!("shared_instance_type = tcp\nshared_instance_port = {port}\n"),
        endpoint: SharedInstanceEndpoint::Tcp(SocketAddr::from(([127, 0, 0, 1], port))),
    }
}

// rnsd binds the abstract socket `\0rns/<instance_name>`
// (RNS/Interfaces/LocalInterface.py:366); the name is unique per run so a
// leftover rnsd cannot be mistaken for this one.
#[cfg(target_os = "linux")]
fn live_unix_shared_instance() -> LiveSharedInstance {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock after the epoch")
        .as_nanos();
    let instance_name = format!("coyote-live-{}-{nanos}", std::process::id());
    LiveSharedInstance {
        instance_lines: format!("shared_instance_type = unix\ninstance_name = {instance_name}\n"),
        endpoint: SharedInstanceEndpoint::Unix(LocalUnixEndpoint::abstract_name(format!(
            "rns/{instance_name}"
        ))),
    }
}

fn write_live_shared_instance_config(dir: &std::path::Path, config: &str) {
    fs::create_dir_all(dir).expect("config dir");
    fs::write(dir.join("config"), config).expect("write shared instance config");
}

#[deny(clippy::await_holding_invalid_type)]
async fn live_wait_for_python_announce(
    announces: &mut tokio::sync::broadcast::Receiver<AnnounceEvent>,
    python_hash: AddressHash,
) -> DestinationDesc {
    tokio::time::timeout(Duration::from_secs(60), async {
        loop {
            match announces.recv().await {
                Ok(event) => {
                    let desc = event.destination.lock().await.desc;
                    if desc.address_hash == python_hash {
                        return desc;
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    panic!("announce channel closed")
                }
            }
        }
    })
    .await
    .expect("timed out waiting for the Python client's announce")
}

#[deny(clippy::await_holding_invalid_type)]
async fn run_python_rnsd_shared_instance_requests_both_ways(
    instance: LiveSharedInstance,
    config_dir_name: &str,
) {
    let paths = python_channel_interop_paths();
    let temp = tempfile::tempdir().expect("tempdir");
    let rns_config = instance.config(free_tcp_port());
    let rnsd_config_dir = temp.path().join(format!("{config_dir_name}-rnsd"));
    let client_config_dir = temp.path().join(format!("{config_dir_name}-client"));
    write_live_shared_instance_config(&rnsd_config_dir, &rns_config);
    write_live_shared_instance_config(&client_config_dir, &rns_config);

    let mut rnsd = paths.spawn_rnsd(&rnsd_config_dir);
    let rnsd_log = ProcessLogTail::follow("rnsd", &mut rnsd);
    let _rnsd_guard = ChildGuard { child: Some(rnsd) };
    rnsd_log
        .wait_for("the shared instance interface", Duration::from_secs(30), |line| {
            line.contains("Started shared instance interface").then_some(())
        })
        .await;

    let rust_identity = to_transport_private_identity(&PrivateIdentity::new_from_rand(OsRng));
    let mut config =
        TransportConfig::new("python-rnsd-shared-instance-rust", &rust_identity, false);
    config.set_path_request_timeout_secs(2);
    let transport = Transport::new(config);
    let mut announces = transport.recv_announces().await;

    // Registered before the attach: a destination added while the posture is
    // set is announced again 250 ms later by the registration task, and the
    // announce guard below needs the explicit announce to be the only early
    // one.
    let destination = transport
        .add_destination(
            rust_identity.clone(),
            DestinationName::new(SHARED_INSTANCE_LIVE_APP_NAME, SHARED_INSTANCE_LIVE_ASPECT),
        )
        .await;
    let rust_hash = { destination.lock().await.desc.address_hash };
    let rust_hex = hex::encode(rust_hash.as_slice());
    {
        destination
            .lock()
            .await
            .register_request_handler(
                SHARED_INSTANCE_LIVE_REQUEST_PATH,
                Arc::new(|request: InboundRequest| {
                    std::future::ready(Some(RequestResponse::Value(request.into_data())))
                }),
                RequestAllow::All,
                true,
            )
            .expect("handler registers");
    }
    transport
        .attach_shared_instance(instance.endpoint, SharedInstanceOptions::default())
        .await
        .expect("attach to the python shared instance");
    assert_eq!(transport.announce_destinations_to_shared_instance().await, 1);
    let first_announce_done = tokio::time::Instant::now();

    // RNS.prettyhexrep wraps the hash in angle brackets; a hop count of zero
    // is what a local client's own announce gets (RNS/Transport.py:1938),
    // and the receiving interface renders as LocalInterface[<peer port>]
    // (or the abstract socket name) for a
    // spawned local client (RNS/Interfaces/LocalInterface.py:351-353).
    let path_learned = format!("Destination <{rust_hex}> is now 0 hops away");
    let path_line = rnsd_log
        .wait_for("the Rust destination path", Duration::from_secs(20), |line| {
            (line.contains(&path_learned) && line.contains("on LocalInterface["))
                .then(|| line.to_string())
        })
        .await;
    eprintln!("rnsd learned the Rust path: {path_line}");

    let mut client = paths.spawn_shared_instance_client(
        &client_config_dir,
        &rust_hex,
        (SHARED_INSTANCE_LIVE_APP_NAME, SHARED_INSTANCE_LIVE_ASPECT),
        SHARED_INSTANCE_LIVE_REQUEST_PATH,
        SHARED_INSTANCE_LIVE_REQUEST_DATA,
        30.0,
    );
    let client_log = ProcessLogTail::follow("python client", &mut client);
    let _client_guard = ChildGuard { child: Some(client) };
    let ready = client_log
        .wait_for("the ready line", Duration::from_secs(30), python_json_line(&["ready"]))
        .await;
    assert_eq!(ready["ready"], true, "python client is a local client of rnsd: {ready}");
    let python_hash =
        AddressHash::new_from_hex_string(ready["destination_hash"].as_str().expect("python hash"))
            .expect("python destination hash");

    // The first announce went out before the client connected, so rnsd had
    // nobody to rebroadcast it to; this one it re-sends to the client as a
    // plain announce (RNS/Transport.py:2398-2430), which is the only way
    // the client's handler can fire: its path request is answered with a
    // path response the handler does not accept (RNS/Transport.py:2501-2504).
    // rnsd files a re-announce at the same hop count only when its emitted
    // second is later than the one on file (RNS/Transport.py:2238) and
    // otherwise ignores it silently, so the second announce waits a full
    // second past the first one's return.
    tokio::time::sleep_until(first_announce_done + Duration::from_millis(1100)).await;
    assert_eq!(transport.announce_destinations_to_shared_instance().await, 1);

    let announce = client_log
        .wait_for(
            "the Rust announce",
            Duration::from_secs(60),
            python_json_line(&["announce_received"]),
        )
        .await;
    assert_eq!(announce["announce_received"], rust_hex);
    assert_eq!(
        announce["hops"], 0,
        "rnsd files a local client's announce at zero hops: {announce}"
    );
    let outcome = client_log
        .wait_for(
            "the python request outcome",
            Duration::from_secs(60),
            python_json_line(&["response", "request_failed"]),
        )
        .await;
    assert_eq!(
        outcome["response"], SHARED_INSTANCE_LIVE_REQUEST_DATA,
        "python request outcome: {outcome}"
    );

    let python_destination = live_wait_for_python_announce(&mut announces, python_hash).await;
    let mut link_events = transport.out_link_events();
    let link = transport.link(python_destination).await.expect("link request sent");
    let link_id = wait_for_out_link_active(&mut link_events, &link, Duration::from_secs(20)).await;
    let receipt = transport
        .request(
            &link_id,
            SHARED_INSTANCE_LIVE_REQUEST_PATH,
            rmpv::Value::from("hello-python"),
            RequestOptions::default(),
        )
        .await
        .expect("request is sent");
    let response = tokio::time::timeout(Duration::from_secs(30), receipt.response())
        .await
        .expect("receipt concludes")
        .expect("response");
    assert_eq!(response.value().and_then(|v| v.as_str()), Some("hello-python"));
    assert_eq!(receipt.status(), RequestStatus::Ready);

    let detached = tokio::time::timeout(Duration::from_secs(5), transport.detach_shared_instance())
        .await
        .expect("detach concludes");
    assert!(detached, "the shared instance was attached");
    tokio::time::timeout(Duration::from_secs(5), transport.stop()).await.expect("transport stops");
}

/// A Python rnsd (RNS 1.5.2) is the shared instance on a free TCP port
/// (`RNS/Reticulum.py:417` logs the interface start), this crate attaches
/// as one local client and a second Python program attaches as another
/// (`RNS/Reticulum.py:420-441`). Proven: rnsd files the Rust announce under
/// a `LocalInterface[<peer port>]` at zero hops (`RNS/Transport.py:1938`,
/// `:2461`, `RNS/Interfaces/LocalInterface.py:351-353`), a Rust announce
/// sent while the Python client is connected reaches that client's announce
/// handler through rnsd's live rebroadcast to its other local clients
/// (`RNS/Transport.py:2398-2430`; the handler takes no path responses, so
/// rnsd's replay of the cached announce cannot stand in for it), and a
/// `Link.request` is answered in both directions through rnsd.
#[tokio::test]
#[ignore = "requires local Python Reticulum checkout"]
async fn python_rnsd_shared_instance_tcp_requests_both_ways() {
    let _interop_guard = python_interop_guard().await;
    run_python_rnsd_shared_instance_requests_both_ways(
        live_tcp_shared_instance(),
        "python-rnsd-shared-instance-tcp",
    )
    .await;
}

/// The abstract `AF_UNIX` form of
/// [`python_rnsd_shared_instance_tcp_requests_both_ways`]: rnsd binds
/// `\0rns/<instance_name>` and both clients connect to it, so the Rust
/// announce is filed under `LocalInterface[rns/<instance_name>]`. Linux
/// only because Python's `use_af_unix()` is true on Linux and Android alone
/// (`RNS/vendor/platformutils.py:59-61`); elsewhere rnsd falls back to TCP
/// and an abstract-socket shared instance cannot be started.
#[cfg(target_os = "linux")]
#[tokio::test]
#[ignore = "requires local Python Reticulum checkout"]
async fn python_rnsd_shared_instance_unix_requests_both_ways() {
    let _interop_guard = python_interop_guard().await;
    run_python_rnsd_shared_instance_requests_both_ways(
        live_unix_shared_instance(),
        "python-rnsd-shared-instance-unix",
    )
    .await;
}
