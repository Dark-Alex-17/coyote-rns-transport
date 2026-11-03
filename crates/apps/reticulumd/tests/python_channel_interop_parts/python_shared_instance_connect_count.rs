use rns_transport::transport::SharedInstanceEvent;

// The dial log is the only evidence of the warn-once-then-debug cadence, so
// this binary installs its own `log::Log`; nothing else in it sets one, and
// `log::set_logger` accepts a single logger per process, hence the `Once`.
// The logger stays installed for the rest of the process, so it stores a
// record only while `armed`, and the run below arms it after emptying the
// store, disarms it before its teardown and turns the level filter off, so
// the binary's other tests, which log at debug per packet, keep nothing.
struct CaptureLogger {
    armed: std::sync::atomic::AtomicBool,
    records: StdMutex<Vec<(log::Level, String)>>,
}

static CAPTURED: CaptureLogger = CaptureLogger {
    armed: std::sync::atomic::AtomicBool::new(false),
    records: StdMutex::new(Vec::new()),
};

impl log::Log for CaptureLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        self.armed.load(std::sync::atomic::Ordering::Acquire)
            && metadata.target().starts_with("rns_transport::iface")
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        self.records
            .lock()
            .expect("captured record store")
            .push((record.level(), record.args().to_string()));
    }

    fn flush(&self) {}
}

fn install_capture_logger() {
    static INSTALL: std::sync::Once = std::sync::Once::new();
    INSTALL.call_once(|| {
        log::set_logger(&CAPTURED).expect("no other logger in this test binary");
    });
    log::set_max_level(log::LevelFilter::Debug);
}

fn arm_capture() {
    CAPTURED.records.lock().expect("captured record store").clear();
    CAPTURED.armed.store(true, std::sync::atomic::Ordering::Release);
}

fn disarm_capture() {
    CAPTURED.armed.store(false, std::sync::atomic::Ordering::Release);
    log::set_max_level(log::LevelFilter::Off);
    CAPTURED.records.lock().expect("captured record store").clear();
}

fn captured_record_count() -> usize {
    CAPTURED.records.lock().expect("captured record store").len()
}

/// The captured lines from the `from` record on that contain `needle`.
fn captured_lines_containing(from: usize, needle: &str) -> Vec<(log::Level, String)> {
    CAPTURED
        .records
        .lock()
        .expect("captured record store")
        .iter()
        .skip(from)
        .filter(|(_, message)| message.contains(needle))
        .cloned()
        .collect()
}

#[deny(clippy::await_holding_invalid_type)]
async fn next_state(
    what: &str,
    events: &mut tokio::sync::mpsc::Receiver<SharedInstanceEvent>,
) -> SharedInstanceEvent {
    tokio::time::timeout(Duration::from_secs(20), events.recv())
        .await
        .unwrap_or_else(|_| panic!("timed out waiting for {what}"))
        .unwrap_or_else(|| panic!("state stream closed waiting for {what}"))
}

#[deny(clippy::await_holding_invalid_type)]
async fn run_python_rnsd_shared_instance_tcp_connect_count_across_restart() {
    let paths = python_channel_interop_paths();
    let temp = tempfile::tempdir().expect("tempdir");
    let instance = live_tcp_shared_instance();
    let endpoint = instance.endpoint.clone();
    let config = instance.config(free_tcp_port());
    let dir = temp.path().join("python-rnsd-connect-count-rnsd");
    write_live_shared_instance_config(&dir, &config);
    let SharedInstanceEndpoint::Tcp(addr) = &endpoint else {
        unreachable!("live_tcp_shared_instance returns a TCP endpoint")
    };
    let dial_failed_needle = format!("couldn't connect to <{addr}>: ");
    let reconnected_needle = format!("connected to <{addr}> after ");

    install_capture_logger();
    arm_capture();
    let mut rnsd = paths.spawn_rnsd(&dir);
    let rnsd_log = ProcessLogTail::follow("rnsd", &mut rnsd);
    let rnsd_guard = ChildGuard { child: Some(rnsd) };
    rnsd_log
        .wait_for("the shared instance interface", Duration::from_secs(30), |line| {
            line.contains("Started shared instance interface").then_some(())
        })
        .await;

    let rust_identity = to_transport_private_identity(&PrivateIdentity::new_from_rand(OsRng));
    let mut config = TransportConfig::new("python-rnsd-connect-count-rust", &rust_identity, false);
    config.set_path_request_timeout_secs(2);
    let transport = Transport::new(config);

    // State events are `try_send` and a full channel drops them; at a 50 ms
    // redial the longest phase below publishes a few hundred.
    let (events_tx, mut events) = tokio::sync::mpsc::channel(1024);
    transport
        .attach_shared_instance(
            endpoint,
            SharedInstanceOptions::default()
                .with_reconnect_wait(Duration::from_millis(50))
                .with_disconnect_reasons(true)
                .with_state_events(events_tx),
        )
        .await
        .expect("attach to the python shared instance");
    assert_eq!(
        next_state("the attach's Connecting", &mut events).await,
        SharedInstanceEvent::Connecting
    );
    assert_eq!(
        next_state("the first Connected", &mut events).await,
        SharedInstanceEvent::Connected
    );
    let iface = transport.shared_instance().expect("the shared instance is attached");
    assert_eq!(transport.interface_connect_count(&iface).await, Some(1));
    assert_eq!(transport.interface_connect_count(&AddressHash::new_from_rand(OsRng)).await, None);
    // The cadence is read from here on, so a first dial that failed before
    // this Connected cannot stand in for the outage's first redial.
    let outage_mark = captured_record_count();

    // The guard kills and reaps rnsd; the tail's Drop then joins the drains,
    // which see EOF only once the child is gone.
    drop(rnsd_guard);
    drop(rnsd_log);
    assert!(
        matches!(
            next_state("the disconnect after the kill", &mut events).await,
            SharedInstanceEvent::DisconnectedWithReason { .. }
        ),
        "a killed rnsd reads as a disconnect with a reason"
    );
    assert_eq!(
        next_state("the redial's Connecting", &mut events).await,
        SharedInstanceEvent::Connecting
    );
    let mut dial_failures = 0usize;
    tokio::time::timeout(Duration::from_secs(20), async {
        while dial_failures < 3 {
            match next_state("a DialFailed while rnsd is down", &mut events).await {
                SharedInstanceEvent::DialFailed { .. } => dial_failures += 1,
                SharedInstanceEvent::Connected => {
                    panic!("connected while rnsd is down after {dial_failures} DialFailed")
                }
                other => eprintln!("state while rnsd is down: {other:?}"),
            }
        }
    })
    .await
    .expect("three DialFailed arrive while rnsd is down");
    // The dial log warns on the first failure and debugs the rest until the
    // kind changes or the thirtieth attempt; a killed rnsd refuses every
    // redial, so the snapshot is taken now and read over the first
    // `dial_failures` lines, each logged before its DialFailed is
    // published, which keeps a slow host's later redials out of the proof.
    let dial_lines = captured_lines_containing(outage_mark, &dial_failed_needle);
    assert_eq!(
        transport.interface_connect_count(&iface).await,
        Some(1),
        "failed redials do not move the count"
    );

    assert!(
        dial_lines.len() >= dial_failures,
        "the dial log has {} lines after {dial_failures} DialFailed: {dial_lines:#?}",
        dial_lines.len()
    );
    let first_dials = &dial_lines[..dial_failures];
    let warns: Vec<&String> = first_dials
        .iter()
        .filter(|(level, _)| *level == log::Level::Warn)
        .map(|(_, message)| message)
        .collect();
    assert_eq!(warns.len(), 1, "one warn for a steady refusal: {first_dials:#?}");
    assert!(warns[0].ends_with("(attempt 1)"), "the warn is the first attempt: {:?}", warns[0]);
    assert_eq!(first_dials[0].0, log::Level::Warn, "the warn is the first line: {first_dials:#?}");
    assert!(
        first_dials[1..].iter().all(|(level, _)| *level == log::Level::Debug),
        "every other dial line is debug: {first_dials:#?}"
    );

    let mut rnsd = paths.spawn_rnsd(&dir);
    let rnsd_log = ProcessLogTail::follow("rnsd", &mut rnsd);
    let _rnsd_guard = ChildGuard { child: Some(rnsd) };
    rnsd_log
        .wait_for("the respawned shared instance interface", Duration::from_secs(30), |line| {
            line.contains("Started shared instance interface").then_some(())
        })
        .await;
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            match next_state("the Connected after the respawn", &mut events).await {
                SharedInstanceEvent::Connected => break,
                SharedInstanceEvent::DialFailed { .. } => dial_failures += 1,
                other => eprintln!("state while rnsd restarts: {other:?}"),
            }
        }
    })
    .await
    .expect("the client reconnects to the respawned rnsd");
    assert_eq!(
        transport.interface_connect_count(&iface).await,
        Some(2),
        "the reconnect counts a second stream"
    );

    let reconnected = captured_lines_containing(outage_mark, &reconnected_needle);
    assert_eq!(reconnected.len(), 1, "one reconnect line: {reconnected:#?}");
    let (level, message) = &reconnected[0];
    assert_eq!(*level, log::Level::Info, "the reconnect line is info: {message:?}");
    let logged_failures: usize = message
        .rsplit_once(" after ")
        .and_then(|(_, rest)| rest.strip_suffix(" failed attempt(s)"))
        .and_then(|count| count.parse().ok())
        .unwrap_or_else(|| panic!("the reconnect line names its failed attempts: {message:?}"));
    eprintln!(
        "rnsd respawn: {dial_failures} DialFailed read, {logged_failures} failed dials logged"
    );
    assert!(
        logged_failures >= dial_failures && logged_failures >= 3,
        "the reconnect line counts every failed dial ({logged_failures} logged, {dial_failures} DialFailed read)"
    );

    disarm_capture();
    let detached = tokio::time::timeout(Duration::from_secs(5), transport.detach_shared_instance())
        .await
        .expect("detach concludes");
    assert!(detached, "the shared instance was attached");
    tokio::time::timeout(Duration::from_secs(5), transport.stop()).await.expect("transport stops");
}

/// A Python rnsd (RNS 1.5.2) is the shared instance on a free TCP port
/// (`RNS/Reticulum.py:417` logs the interface start) and this crate attaches
/// as a local client the way a second Python program does
/// (`RNS/Reticulum.py:420-441`; `RNS/Interfaces/LocalInterface.py:58-141` is
/// the `LocalClientInterface` connect path, `:46-49` the listener's
/// `SO_REUSEADDR` on non-Windows hosts that lets the respawn rebind the same
/// port). Proven:
/// `interface_connect_count(shared_instance())` reads `Some(1)` after
/// `Connected`, stays `Some(1)` across a kill of rnsd while at least three
/// `DialFailed` arrive and, over its first three or more redials, the dial
/// log warns once (attempt 1) then debugs, and reads `Some(2)` once rnsd is
/// back on the same port, with one info line counting the failed dials.
/// Python has no counter and logs each failed
/// dial at `LOG_DEBUG` (`RNS/Interfaces/TCPInterface.py:288`,
/// `RNS/Interfaces/LocalInterface.py:167`), so the fork is louder on the
/// first attempt and quieter after.
#[tokio::test]
#[ignore = "requires local Python Reticulum checkout"]
async fn python_rnsd_shared_instance_tcp_connect_count_across_restart() {
    let _interop_guard = python_interop_guard().await;
    run_python_rnsd_shared_instance_tcp_connect_count_across_restart().await;
}
