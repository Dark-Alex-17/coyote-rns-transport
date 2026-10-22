// The Python shared-instance client's `emit` writes a report under
// `RNS.logging_lock`, the lock `RNS.log` holds around its own print
// (`RNS/__init__.py:89`, `:129-133`), so a report never shares a stdout
// line with an RNS log line the transport thread prints. The Rust reader
// skips lines that are not JSON, so a report glued to a log line is a
// report lost and a 60 s wait. Pinned here without an rnsd: the helper's
// `emit` is imported as a module function and raced against four threads
// calling `RNS.log` to stdout for one second; every stdout line must then
// be either a whole log line or a whole report. The unlocked form of `emit`
// (one `write` + `flush`, no lock) loses under one per cent of its reports
// (about 0.7-0.9 % measured) to glued lines in that window on this host, so
// the oracle discriminates.

const SHARED_INSTANCE_CLIENT_EMIT_RACE_SECS: &str = "1.0";

// argv[1] is the helper path, argv[2] the wall time of the race in seconds.
const SHARED_INSTANCE_CLIENT_EMIT_RACE_SCRIPT: &str = r#"
import importlib.util
import sys
import threading
import time

import RNS

RNS.loglevel = RNS.LOG_DEBUG
RNS.logdest = RNS.LOG_STDOUT

spec = importlib.util.spec_from_file_location("python_shared_instance_client", sys.argv[1])
helper = importlib.util.module_from_spec(spec)
spec.loader.exec_module(helper)

stop = threading.Event()

def hammer():
    i = 0
    while not stop.is_set():
        RNS.log(f"emit-probe log line {i}", RNS.LOG_DEBUG)
        i += 1

threads = [threading.Thread(target=hammer, daemon=True) for _ in range(4)]
for thread in threads:
    thread.start()
deadline = time.time() + float(sys.argv[2])
reports = 0
while time.time() < deadline:
    helper.emit({"probe": reports})
    reports += 1
stop.set()
for thread in threads:
    thread.join(5)
sys.stdout.flush()
print(f"emit-probe reports {reports}", file=sys.stderr)
"#;

/// Every stdout line the race produced is a whole RNS log line or a whole
/// report: a line holding both, or a report missing its closing brace, is
/// a report `emit` let the transport thread's print interleave with.
#[tokio::test]
#[ignore = "requires local Python Reticulum checkout"]
async fn python_shared_instance_client_emit_never_shares_a_line_with_an_rns_log_line() {
    let _interop_guard = python_interop_guard().await;
    let paths = python_channel_interop_paths();
    let output = paths.run_shared_instance_client_helper_script(
        SHARED_INSTANCE_CLIENT_EMIT_RACE_SCRIPT,
        &[SHARED_INSTANCE_CLIENT_EMIT_RACE_SECS],
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "the emit race script failed: {stderr}");
    let stdout = String::from_utf8_lossy(&output.stdout);

    let reports_emitted: usize = stderr
        .lines()
        .find_map(|line| line.strip_prefix("emit-probe reports "))
        .and_then(|count| count.trim().parse().ok())
        .expect("the script reports how many times it called emit");

    let mut log_lines = 0usize;
    let mut reports_seen = 0usize;
    let mut malformed = Vec::new();
    for line in stdout.lines() {
        if line.is_empty() {
            continue;
        }
        let is_log_line = line.starts_with('[')
            && line.contains("] [Debug]")
            && line.rsplit_once("emit-probe log line ").is_some_and(|(_, index)| {
                !index.is_empty() && index.bytes().all(|b| b.is_ascii_digit())
            });
        if is_log_line {
            log_lines += 1;
            continue;
        }
        match serde_json::from_str::<serde_json::Value>(line) {
            Ok(value) if value.get("probe").is_some_and(serde_json::Value::is_u64) => {
                reports_seen += 1;
            }
            _ => malformed.push(line.to_string()),
        }
    }

    // Positive controls: the race ran on both sides.
    assert!(log_lines >= 100, "the RNS.log hammer printed only {log_lines} lines");
    assert!(reports_emitted >= 100, "emit was called only {reports_emitted} times");
    assert!(
        malformed.is_empty(),
        "{} of {reports_emitted} reports shared a stdout line with an RNS log line; first five: {:#?}",
        malformed.len(),
        &malformed[..malformed.len().min(5)]
    );
    assert_eq!(
        reports_seen, reports_emitted,
        "every report emitted arrives as its own JSON line ({log_lines} log lines around them)"
    );
}
