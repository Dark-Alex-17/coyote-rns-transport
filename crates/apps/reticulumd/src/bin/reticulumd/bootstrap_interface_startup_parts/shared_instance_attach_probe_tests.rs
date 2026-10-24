// Black-box pins for the shared-instance client knobs a daemon config carries after the
// migration onto `attach_shared_instance`: `prefer_ipv6` steers which loopback the client
// dials, observed as the accept on a `[::1]`-only listener; `connect_timeout` and
// `max_reconnect_tries` still parse (the config surface is unchanged) and each draws one
// startup warning naming the posture, the entry and the key. Helpers (`attach_test_args`,
// `test_transport`, `records_for`, `run_startup`, `assert_slot_holds_attached_record`) come
// from the parent test module.
// `force_shared_instance_bitrate` is pinned at the wire: the spawned client paces its first
// frame by `raw_len * 8 / bitrate`, observable as the time-to-first-byte at the listener.

use super::*;

/// Store for the daemon's log lines. `log::set_logger` takes one logger per process, so
/// the first test to need it installs the capture and every later caller reuses it; tests
/// filter by their entry's unique label, so parallel tests cannot see each other's lines.
static PROBE_LOG_RECORDS: std::sync::Mutex<Vec<(log::Level, String)>> =
    std::sync::Mutex::new(Vec::new());

struct ProbeLogCapture;

impl log::Log for ProbeLogCapture {
    fn enabled(&self, _metadata: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        let mut records = PROBE_LOG_RECORDS.lock().expect("probe log store");
        records.push((record.level(), format!("{}", record.args())));
    }

    fn flush(&self) {}
}

fn install_probe_log_capture() {
    static CAPTURE: ProbeLogCapture = ProbeLogCapture;
    // A second install reports `SetLoggerError`; the first install's store is the shared one.
    let _ = log::set_logger(&CAPTURE);
    log::set_max_level(log::LevelFilter::Debug);
}

fn probe_warnings_containing(needle: &str) -> Vec<String> {
    PROBE_LOG_RECORDS
        .lock()
        .expect("probe log store")
        .iter()
        .filter(|(level, message)| *level == log::Level::Warn && message.contains(needle))
        .map(|(_, message)| message.clone())
        .collect()
}

/// Whether `localhost` has an IPv6 answer on this host; the tests that steer the dial onto
/// `[::1]` skip (with a note on stderr) where it has none, as the pick then falls back to
/// the IPv4 loopback and the oracle has nothing to discriminate.
async fn localhost_resolves_to_ipv6() -> bool {
    let mut addrs = match tokio::net::lookup_host("localhost:1").await {
        Ok(addrs) => addrs,
        Err(err) => {
            eprintln!("skipping: localhost does not resolve on this host: {err}");
            return false;
        }
    };
    if addrs.any(|addr| addr.is_ipv6()) {
        return true;
    }
    eprintln!("skipping: localhost has no IPv6 address on this host");
    false
}

/// `prefer_ipv6` is the one knob a shared instance listening on `[::1]` alone needs: with
/// it set, `host = "localhost"` dials the IPv6 loopback, observed as the accept arriving on
/// a `[::1]`-only listener. Without it the sibling pin
/// `local_client_localhost_host_connects_to_the_ipv4_loopback_listener` sees the same name
/// dial `127.0.0.1`, so the two together discriminate the pick order from a flag that is
/// read but not honoured.
#[tokio::test]
async fn local_client_prefer_ipv6_dials_the_ipv6_loopback_listener_for_localhost() {
    if !localhost_resolves_to_ipv6().await {
        return;
    }
    let shared_listener = match tokio::net::TcpListener::bind("[::1]:0").await {
        Ok(listener) => listener,
        Err(err) => {
            eprintln!("skipping: no IPv6 loopback on this host: {err}");
            return;
        }
    };
    let port = shared_listener.local_addr().expect("shared local addr").port();
    let config = DaemonConfig::from_toml(&format!(
        r#"
interfaces = [
  {{ type = "LocalClientInterface", enabled = true, name = "localhost-v6-dial", shared_instance_type = "tcp", host = "localhost", shared_instance_port = {port}, prefer_ipv6 = true }}
]
"#
    ))
    .expect("prefer_ipv6 parses on a local_client entry");
    assert_eq!(config.interfaces[0].prefer_ipv6, Some(true));
    let args = attach_test_args();
    let transport = test_transport();
    let mut records = records_for(&config);

    let batch = run_startup(&args, &config, &transport, &mut records).await;
    assert_eq!(batch.startup_successes, 1);
    assert!(batch.startup_failures.is_empty(), "failures={:?}", batch.startup_failures);

    let (_stream, peer) =
        tokio::time::timeout(std::time::Duration::from_secs(5), shared_listener.accept())
            .await
            .expect("the client dials the IPv6 loopback listener within 5 s")
            .expect("accept the client's connection");
    assert!(
        peer.ip().is_loopback() && peer.is_ipv6(),
        "the connection comes from the IPv6 loopback, not from {peer}"
    );
    assert_slot_holds_attached_record(&transport, &records[0]).await;
}

/// The two knobs the shared-instance client has no setting for still parse on a
/// client-posture entry, and the attach warns once per key, in the documented shape
/// `[daemon] <posture> name=<label> ignores <key>`; `prefer_ipv6` is honoured, so it draws
/// no such line, and an entry without the knobs draws none at all.
#[tokio::test]
async fn local_client_timeout_and_retry_keys_parse_and_each_draws_one_startup_warning() {
    install_probe_log_capture();
    let shared_listener =
        std::net::TcpListener::bind("127.0.0.1:0").expect("bind shared local instance");
    let port = shared_listener.local_addr().expect("shared local addr").port();
    let config = DaemonConfig::from_toml(&format!(
        r#"
interfaces = [
  {{ type = "LocalClientInterface", enabled = true, name = "knob-warn-client", shared_instance_type = "tcp", host = "127.0.0.1", shared_instance_port = {port}, connect_timeout = 5, max_reconnect_tries = 0, prefer_ipv6 = false }}
]
"#
    ))
    .expect("connect_timeout and max_reconnect_tries still parse on a local_client entry");
    assert_eq!(config.interfaces[0].connect_timeout, Some(5));
    assert_eq!(config.interfaces[0].max_reconnect_tries, Some(0));
    let args = attach_test_args();
    let transport = test_transport();
    let mut records = records_for(&config);

    let batch = run_startup(&args, &config, &transport, &mut records).await;
    assert_eq!(batch.startup_successes, 1);
    assert!(batch.startup_failures.is_empty(), "failures={:?}", batch.startup_failures);
    assert_slot_holds_attached_record(&transport, &records[0]).await;

    let lines = probe_warnings_containing("name=knob-warn-client ignores");
    assert_eq!(lines.len(), 2, "one warning per ignored key: {lines:?}");
    for key in ["connect_timeout", "max_reconnect_tries"] {
        let matching: Vec<&String> = lines
            .iter()
            .filter(|line| {
                line.starts_with(&format!("[daemon] local name=knob-warn-client ignores {key}"))
            })
            .collect();
        assert_eq!(matching.len(), 1, "exactly one line names {key}: {lines:?}");
    }
    assert!(
        lines.iter().all(|line| !line.contains("prefer_ipv6")),
        "prefer_ipv6 is honoured, not ignored: {lines:?}"
    );
}

/// An entry without the two knobs draws no `ignores` warning, and the same holds for the
/// busy-port `local` arm, whose attach runs the same code: the warning is keyed to the
/// knobs present in the entry, not to the posture.
#[tokio::test]
async fn client_posture_entries_without_the_knobs_draw_no_ignores_warning() {
    install_probe_log_capture();
    let shared_listener =
        std::net::TcpListener::bind("127.0.0.1:0").expect("bind shared local instance");
    let port = shared_listener.local_addr().expect("shared local addr").port();
    let config = DaemonConfig::from_toml(&format!(
        r#"
interfaces = [
  {{ type = "LocalInterface", enabled = true, name = "quiet-busy-port-local", shared_instance_type = "tcp", shared_instance_port = {port} }}
]
"#
    ))
    .expect("local busy-port config");
    let args = attach_test_args();
    let transport = test_transport();
    let mut records = records_for(&config);

    let batch = run_startup(&args, &config, &transport, &mut records).await;
    assert_eq!(batch.startup_successes, 1);
    assert!(batch.startup_failures.is_empty(), "failures={:?}", batch.startup_failures);
    assert_slot_holds_attached_record(&transport, &records[0]).await;

    let lines = probe_warnings_containing("name=quiet-busy-port-local ignores");
    assert!(lines.is_empty(), "no knob, no warning: {lines:?}");
}

/// The busy-port `local` arm attaches through the same helper, so an entry of that kind
/// carrying `connect_timeout` is warned about under the `local` posture label too.
#[tokio::test]
async fn busy_port_local_entry_with_connect_timeout_is_warned_about_under_the_local_posture() {
    install_probe_log_capture();
    let shared_listener =
        std::net::TcpListener::bind("127.0.0.1:0").expect("bind shared local instance");
    let port = shared_listener.local_addr().expect("shared local addr").port();
    let config = DaemonConfig::from_toml(&format!(
        r#"
interfaces = [
  {{ type = "LocalInterface", enabled = true, name = "busy-port-local-timeout", shared_instance_type = "tcp", shared_instance_port = {port}, connect_timeout = 7 }}
]
"#
    ))
    .expect("connect_timeout still parses on a local entry");
    let args = attach_test_args();
    let transport = test_transport();
    let mut records = records_for(&config);

    let batch = run_startup(&args, &config, &transport, &mut records).await;
    assert_eq!(batch.startup_successes, 1);
    assert!(batch.startup_failures.is_empty(), "failures={:?}", batch.startup_failures);
    assert_slot_holds_attached_record(&transport, &records[0]).await;

    let lines = probe_warnings_containing("name=busy-port-local-timeout ignores");
    assert_eq!(lines, vec![
        "[daemon] local name=busy-port-local-timeout ignores connect_timeout: the shared-instance client has no such setting, as Python's LocalClientInterface has none (RNS/Interfaces/LocalInterface.py)".to_string()
    ]);
}

/// The deleted builder test pinned `force_shared_instance_bitrate` on the adapter the
/// daemon no longer builds; its replacement reads the mode and the MTU back from the iface
/// manager, which cannot read the bitrate. The bitrate is observable at the shared
/// instance's side instead: the spawned client sleeps `raw_len * 8 / bitrate` before each
/// write, so with 400 bps the tunnel-synthesis packet the attach sends on the new stream
/// (about 200 raw bytes) reaches a listener no sooner than 2 s after the accept, where an
/// entry without the knob delivers it within milliseconds.
#[tokio::test]
async fn local_client_force_shared_instance_bitrate_paces_the_first_frame_on_the_new_stream() {
    let shared_listener =
        tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind shared local instance");
    let port = shared_listener.local_addr().expect("shared local addr").port();
    let config = DaemonConfig::from_toml(&format!(
        r#"
interfaces = [
  {{ type = "LocalClientInterface", enabled = true, name = "paced-client", shared_instance_type = "tcp", shared_instance_port = {port}, force_shared_instance_bitrate = 400 }}
]
"#
    ))
    .expect("local_client with a forced bitrate");
    assert_eq!(config.interfaces[0].force_shared_instance_bitrate, Some(400));
    let args = attach_test_args();
    let transport = test_transport();
    let mut records = records_for(&config);

    let batch = run_startup(&args, &config, &transport, &mut records).await;
    assert_eq!(batch.startup_successes, 1);
    assert!(batch.startup_failures.is_empty(), "failures={:?}", batch.startup_failures);
    assert_slot_holds_attached_record(&transport, &records[0]).await;

    let (mut stream, _) =
        tokio::time::timeout(std::time::Duration::from_secs(5), shared_listener.accept())
            .await
            .expect("the client dials the listener within 5 s")
            .expect("accept the client's connection");
    let accepted = std::time::Instant::now();
    let mut byte = [0u8; 1];
    let read = tokio::time::timeout(
        std::time::Duration::from_secs(20),
        tokio::io::AsyncReadExt::read(&mut stream, &mut byte),
    )
    .await
    .expect("the first frame reaches the listener within 20 s")
    .expect("read the first byte");
    assert_eq!(read, 1, "the stream delivered a byte");
    let arrival = accepted.elapsed();
    assert!(
        arrival >= std::time::Duration::from_secs(2),
        "with force_shared_instance_bitrate = 400 the first frame arrived {arrival:?} after the \
         accept: the entry's bitrate did not reach the spawned client"
    );
}

/// Under `--strict-interface-startup` the preflight dials the address the attach will dial,
/// not the host string: with `prefer_ipv6` the pick is `[::1]`, so a shared instance that
/// listens on `127.0.0.1` alone is a recorded startup failure naming the picked address. A
/// preflight on the string would connect to the IPv4 loopback and pass, leaving an attach
/// that never connects.
#[tokio::test]
async fn strict_startup_preflights_the_picked_family_and_rejects_a_listener_on_the_other_one() {
    if !localhost_resolves_to_ipv6().await {
        return;
    }
    let shared_listener =
        std::net::TcpListener::bind("127.0.0.1:0").expect("bind shared local instance");
    let port = shared_listener.local_addr().expect("shared local addr").port();
    let config = DaemonConfig::from_toml(&format!(
        r#"
interfaces = [
  {{ type = "LocalClientInterface", enabled = true, name = "strict-other-family", shared_instance_type = "tcp", host = "localhost", shared_instance_port = {port}, prefer_ipv6 = true }}
]
"#
    ))
    .expect("prefer_ipv6 parses on a local_client entry");
    let mut args = attach_test_args();
    args.strict_interface_startup = true;
    let transport = test_transport();
    let mut records = records_for(&config);

    let batch = run_startup(&args, &config, &transport, &mut records).await;

    assert_eq!(batch.startup_successes, 0, "the IPv4-only listener is not the picked family");
    assert_eq!(batch.startup_failures.len(), 1, "failures={:?}", batch.startup_failures);
    let failure = &batch.startup_failures[0];
    assert_eq!(failure.label, "strict-other-family");
    assert!(
        failure.error.contains("preflight connect failed")
            || failure.error.contains("preflight connect timed out"),
        "the preflight is what failed: {}",
        failure.error
    );
    assert!(
        failure.error.contains(&format!("[::1]:{port}")),
        "the failure names the picked address: {}",
        failure.error
    );
    assert!(transport.shared_instance().is_none(), "nothing was attached");
    assert_eq!(runtime_field(&records[0], "startup_status"), Some("failed"));
}

/// The converse: the picked `[::1]` listens, strict startup passes, and the accept arrives
/// from the IPv6 loopback peer the preflight dialled.
#[tokio::test]
async fn strict_startup_passes_when_the_picked_family_listens() {
    if !localhost_resolves_to_ipv6().await {
        return;
    }
    let shared_listener = match tokio::net::TcpListener::bind("[::1]:0").await {
        Ok(listener) => listener,
        Err(err) => {
            eprintln!("skipping: no IPv6 loopback on this host: {err}");
            return;
        }
    };
    let port = shared_listener.local_addr().expect("shared local addr").port();
    let config = DaemonConfig::from_toml(&format!(
        r#"
interfaces = [
  {{ type = "LocalClientInterface", enabled = true, name = "strict-picked-family", shared_instance_type = "tcp", host = "localhost", shared_instance_port = {port}, prefer_ipv6 = true }}
]
"#
    ))
    .expect("prefer_ipv6 parses on a local_client entry");
    let mut args = attach_test_args();
    args.strict_interface_startup = true;
    let transport = test_transport();
    let mut records = records_for(&config);

    let batch = run_startup(&args, &config, &transport, &mut records).await;
    assert_eq!(batch.startup_successes, 1);
    assert!(batch.startup_failures.is_empty(), "failures={:?}", batch.startup_failures);

    // The preflight's own connection is dropped on return; the attach's is the first that
    // stays up, and either way every accept here comes from the IPv6 loopback.
    let (_stream, peer) =
        tokio::time::timeout(std::time::Duration::from_secs(5), shared_listener.accept())
            .await
            .expect("the preflight or the client dials the IPv6 loopback listener within 5 s")
            .expect("accept a connection");
    assert!(
        peer.ip().is_loopback() && peer.is_ipv6(),
        "the connection comes from the IPv6 loopback, not from {peer}"
    );
    assert_slot_holds_attached_record(&transport, &records[0]).await;
}
