// Verdict probes for the two evidence scripts: the cross-target clippy run
// reports zero diagnostics or SKIPs when zig or the target is absent, and the
// interop batch passes only when every failure is a named macOS environment
// red or the reference-pin test while the reference checkout is off its pin.
// The sibling probe3 pins the words and the early exits; these drive the
// verdict with a stub toolchain first on PATH (`cargo` answering the clippy
// or nextest run with a canned transcript, `zig`, `rustup` and `git`
// answering the presence and revision checks) so every arm is reached in
// milliseconds and the scripts never build anything.

/// Adds an executable `name` to the stub directory that prints `stdout` and
/// exits 0.
#[cfg(unix)]
fn add_stub_tool(stub: &CargoStub, name: &str, stdout: &str) {
    use std::os::unix::fs::PermissionsExt;
    let path = stub.dir.join(name);
    fs::write(&path, format!("#!/usr/bin/env bash\nprintf '%s\\n' {}\n", shell_quote(stdout)))
        .unwrap_or_else(|err| panic!("write the {name} stub: {err}"));
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755))
        .unwrap_or_else(|err| panic!("make the {name} stub executable: {err}"));
}

#[cfg(unix)]
fn shell_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

/// Runs `tools/scripts/<script>` from the repository root with the stub
/// directory first on PATH, the stub `cargo` answering with `out` and
/// `status`, and `extra_env` applied last. Returns `(exit code, stdout, stderr)`.
#[cfg(unix)]
fn run_script_with_stub_toolchain(
    script: &str,
    stub: &CargoStub,
    out: &str,
    status: i32,
    extra_env: &[(&str, &str)],
) -> (Option<i32>, String, String) {
    let path = format!("{}:{}", stub.dir.display(), std::env::var("PATH").unwrap_or_default());
    let mut command = Command::new("/bin/bash");
    command
        .arg(repo_root().join("tools/scripts").join(script))
        .env("PATH", path)
        .env("PROBE_CARGO_OUT", out)
        .env("PROBE_CARGO_STATUS", status.to_string())
        .current_dir(repo_root());
    for (key, value) in extra_env {
        command.env(key, value);
    }
    let output =
        command.output().unwrap_or_else(|err| panic!("run tools/scripts/{script}: {err}"));
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[cfg(unix)]
const CROSS_TARGET_SCRIPT: &str = "cross-target-clippy-classify.sh";
#[cfg(unix)]
const INTEROP_SCRIPT: &str = "python-interop-classify.sh";
#[cfg(unix)]
const LINUX_TARGET: &str = "x86_64-unknown-linux-gnu";
#[cfg(unix)]
const BUILD_FINISHED: &str = r#"{"reason":"build-finished","success":true}"#;

/// A stub toolchain where zig and the Linux rust-std are present.
#[cfg(unix)]
fn cross_target_stub(name: &str) -> CargoStub {
    let stub = CargoStub::new(name);
    add_stub_tool(&stub, "zig", "");
    add_stub_tool(&stub, "rustup", LINUX_TARGET);
    stub
}

#[cfg(unix)]
fn clippy_hit(file_name: &str) -> String {
    format!(
        r#"{{"reason":"compiler-message","message":{{"code":{{"code":"clippy::await_holding_invalid_type"}},"level":"warning","message":"guard held across await","spans":[{{"is_primary":true,"file_name":"{file_name}"}}]}}}}"#
    )
}

/// With zig and the target present and a clean clippy run the cross-target
/// script reaches cargo for the Linux target and reports zero diagnostics.
#[cfg(unix)]
#[test]
fn cross_target_script_passes_a_clean_linux_run() {
    let stub = cross_target_stub("cross-clean");
    let (code, stdout, stderr) =
        run_script_with_stub_toolchain(CROSS_TARGET_SCRIPT, &stub, BUILD_FINISHED, 0, &[]);
    assert_eq!(code, Some(0), "a clean run passes: stdout {stdout:?} stderr {stderr:?}");
    assert!(
        stdout.contains(&format!("cargo clippy (--target {LINUX_TARGET}) exit: 0")),
        "the clippy run is reached and named, got {stdout:?}"
    );
    assert!(
        stdout.contains("await_holding_invalid_type diagnostics (any level): 0"),
        "the zero count is reported, got {stdout:?}"
    );
    assert!(stdout.contains("OK: zero await_holding_invalid_type"), "the verdict is OK, got {stdout:?}");
    assert!(!stdout.contains("SKIP:") && !stdout.contains("FAIL:"), "no skip, no refusal: {stdout:?}");
}

/// One diagnostic of the lint on the Linux target fails the run by file even
/// though cargo itself exited 0 (the run never denies warnings).
#[cfg(unix)]
#[test]
fn cross_target_script_fails_on_a_linux_only_hit_and_names_the_file() {
    let stub = cross_target_stub("cross-hit");
    let file = "crates/libs/rns-transport/src/iface/pipe_tests.rs";
    let out = format!("{}\n{BUILD_FINISHED}", clippy_hit(file));
    let (code, stdout, stderr) =
        run_script_with_stub_toolchain(CROSS_TARGET_SCRIPT, &stub, &out, 0, &[]);
    assert_eq!(code, Some(1), "a hit fails: stdout {stdout:?} stderr {stderr:?}");
    assert!(
        stdout.contains("await_holding_invalid_type diagnostics (any level): 1"),
        "the count is reported, got {stdout:?}"
    );
    assert!(
        stdout.lines().any(|line| line.trim_start().starts_with("1  ") && line.ends_with(file)),
        "the hit is listed per file, got {stdout:?}"
    );
    assert!(stdout.contains("FAIL: await_holding_invalid_type fired on the Linux target"), "{stdout:?}");
    assert!(!stdout.contains("OK:"), "a hit is never OK, got {stdout:?}");
}

/// A cross-target build that fails with no diagnostic of the lint is not a
/// pass either: the verdict requires a clean cargo exit.
#[cfg(unix)]
#[test]
fn cross_target_script_fails_when_the_cross_build_itself_fails() {
    let stub = cross_target_stub("cross-build-failed");
    let (code, stdout, stderr) =
        run_script_with_stub_toolchain(CROSS_TARGET_SCRIPT, &stub, BUILD_FINISHED, 101, &[]);
    assert_eq!(code, Some(1), "a failed build fails: stdout {stdout:?} stderr {stderr:?}");
    assert!(stdout.contains(&format!("cargo clippy (--target {LINUX_TARGET}) exit: 101")), "{stdout:?}");
    assert!(stdout.contains("FAIL: cargo clippy exited 101"), "the failure names the exit, got {stdout:?}");
    assert!(!stdout.contains("OK:"), "a failed build is never OK, got {stdout:?}");
}

/// With zig present but the Linux rust-std missing the script skips before
/// any cargo run and names the target to add.
#[cfg(unix)]
#[test]
fn cross_target_script_skips_without_the_linux_target_and_names_it() {
    let stub = CargoStub::new("cross-no-target");
    add_stub_tool(&stub, "zig", "");
    add_stub_tool(&stub, "rustup", "aarch64-apple-darwin");
    let (code, stdout, stderr) =
        run_script_with_stub_toolchain(CROSS_TARGET_SCRIPT, &stub, BUILD_FINISHED, 0, &[]);
    assert_eq!(code, Some(0), "a missing target skips: stdout {stdout:?} stderr {stderr:?}");
    assert!(
        stdout.lines().any(|line| {
            line.starts_with("SKIP:")
                && line.contains(LINUX_TARGET)
                && line.contains("rustup target add")
        }),
        "the skip names the target and the command to add it, got {stdout:?}"
    );
    assert!(!stdout.contains("cargo clippy"), "no cargo run on a skip, got {stdout:?}");
    assert!(!stdout.contains("OK:"), "a skip is not a pass, got {stdout:?}");
}

/// The cross-target script takes no arguments.
#[cfg(unix)]
#[test]
fn cross_target_script_refuses_arguments_with_usage() {
    let output = Command::new("/bin/bash")
        .arg(repo_root().join("tools/scripts").join(CROSS_TARGET_SCRIPT))
        .arg("--all-features")
        .current_dir(repo_root())
        .output()
        .expect("run the cross-target script with an argument");
    assert_eq!(output.status.code(), Some(2), "an argument is a usage error");
    assert!(
        String::from_utf8_lossy(&output.stderr).starts_with("usage:"),
        "the refusal is the usage line"
    );
}

/// Runs the interop script with the stub `cargo` answering the nextest
/// batch. `RETICULUM_PY_REPO` points at this repository: the script only asks
/// it for `rev-parse HEAD`, and a fork commit is never the Python pin, so the
/// pin-drift test is tolerated on every host (unless a stub `git` answers
/// with the pin itself) while the Darwin list applies on macOS only.
#[cfg(unix)]
fn run_interop_with_transcript(stub: &CargoStub, transcript: &str, status: i32) -> (Option<i32>, String, String) {
    let repo = repo_root();
    let repo = repo.to_str().expect("utf-8 repo root");
    run_script_with_stub_toolchain(INTEROP_SCRIPT, stub, transcript, status, &[("RETICULUM_PY_REPO", repo)])
}

#[cfg(unix)]
const PIN_DRIFT_TEST: &str = "pinned_python_resource_cancels_after_missing_part_retry_budget";
#[cfg(unix)]
const PIN_SOURCE: &str = "crates/apps/reticulumd/tests/python_channel_interop_parts/resource_timeout_reference.rs";

#[cfg(unix)]
fn nextest_line(status: &str, index: usize, total: usize, test: &str) -> String {
    format!("{status:>12} [   0.100s] ({index}/{total}) reticulumd::python_channel_interop {test}")
}

#[cfg(unix)]
fn summary_line(run: usize, passed: usize, failed: usize) -> String {
    format!("     Summary [   0.300s] {run} tests run: {passed} passed, {failed} failed, 0 skipped")
}

#[cfg(unix)]
fn pinned_python_revision() -> String {
    let source = read_repo_file(PIN_SOURCE);
    let (_, after) = source.split_once("PINNED_RETICULUM: &str = \"").expect("the pin constant");
    after.split('"').next().expect("the pin value").to_string()
}

/// Off its pin, the pin-drift test is the only failure tolerated on every
/// host; two passes plus that failure (nextest exit 100) is a pass that names
/// the tolerated test, and the drift is announced against the checkout.
#[cfg(unix)]
#[test]
fn interop_script_tolerates_only_the_pin_drift_test_when_the_checkout_is_off_its_pin() {
    let stub = CargoStub::new("interop-pin-drift");
    let transcript = [
        nextest_line("PASS", 1, 3, "python_to_rust_channel_roundtrip"),
        nextest_line("FAIL", 2, 3, PIN_DRIFT_TEST),
        nextest_line("PASS", 3, 3, "rust_to_python_link_roundtrip"),
        summary_line(3, 2, 1),
    ]
    .join("\n");
    let (code, stdout, stderr) = run_interop_with_transcript(&stub, &transcript, 100);
    assert_eq!(code, Some(0), "tolerated failures pass: stdout {stdout:?} stderr {stderr:?}");
    assert!(stdout.contains("checkout differs from the pin; tolerating"), "{stdout:?}");
    assert!(stdout.contains("cargo nextest (python_channel_interop batch) exit: 100"), "{stdout:?}");
    assert!(stdout.contains("passed: 2, failed: 1"), "the counts are reported, got {stdout:?}");
    assert!(
        stdout.contains(&format!("OK: 2 passed; every failure is tolerated: {PIN_DRIFT_TEST}")),
        "the verdict names the tolerated failure, got {stdout:?}"
    );
}

/// On its pin, the reference checkout gives the pin test no excuse: its
/// failure is untolerated and fails the run. The stub `git` answers the
/// script's `rev-parse HEAD` with the pinned revision.
#[cfg(unix)]
#[test]
fn interop_script_fails_when_the_pin_test_fails_on_the_pinned_checkout() {
    let stub = CargoStub::new("interop-on-pin");
    let pinned = pinned_python_revision();
    add_stub_tool(&stub, "git", &pinned);
    let transcript = [
        nextest_line("PASS", 1, 2, "python_to_rust_channel_roundtrip"),
        nextest_line("FAIL", 2, 2, PIN_DRIFT_TEST),
        summary_line(2, 1, 1),
    ]
    .join("\n");
    let (code, stdout, stderr) = run_interop_with_transcript(&stub, &transcript, 100);
    assert_eq!(code, Some(1), "the pin test fails on its pin: stdout {stdout:?} stderr {stderr:?}");
    assert!(stdout.contains(&format!("python reticulum checkout: {pinned} (pinned: {pinned})")), "{stdout:?}");
    assert!(!stdout.contains("checkout differs from the pin"), "no drift to announce, got {stdout:?}");
    assert!(stdout.contains("FAIL: 1 failure(s) outside the tolerated set:"), "{stdout:?}");
    assert!(stdout.lines().any(|line| line.trim() == PIN_DRIFT_TEST), "the pin test is named, got {stdout:?}");
    assert!(!stdout.contains("OK:"), "never OK when the pin test fails on its pin, got {stdout:?}");
}

/// A failure outside the tolerated set fails the run and is named, even when
/// a tolerated failure sits next to it.
#[cfg(unix)]
#[test]
fn interop_script_fails_naming_an_untolerated_failure() {
    let stub = CargoStub::new("interop-untolerated");
    let transcript = [
        nextest_line("PASS", 1, 3, "python_to_rust_channel_roundtrip"),
        nextest_line("FAIL", 2, 3, PIN_DRIFT_TEST),
        nextest_line("FAIL", 3, 3, "rust_to_python_link_roundtrip"),
    ]
    .join("\n");
    let (code, stdout, stderr) = run_interop_with_transcript(&stub, &transcript, 100);
    assert_eq!(code, Some(1), "an untolerated failure fails: stdout {stdout:?} stderr {stderr:?}");
    assert!(stdout.contains("FAIL: 1 failure(s) outside the tolerated set:"), "{stdout:?}");
    assert!(
        stdout.lines().any(|line| line.trim() == "rust_to_python_link_roundtrip"),
        "the untolerated test is named on a line of its own, got {stdout:?}"
    );
    assert!(!stdout.contains("OK:"), "never OK with an untolerated failure, got {stdout:?}");
}

/// A nextest exit without a single status line means the batch did not run
/// (a build error, a missing Python); that is a failure, not an empty pass.
#[cfg(unix)]
#[test]
fn interop_script_fails_when_the_batch_did_not_run() {
    let stub = CargoStub::new("interop-no-batch");
    let transcript = "error: could not compile `reticulumd` (test \"python_channel_interop\")";
    let (code, stdout, stderr) = run_interop_with_transcript(&stub, transcript, 101);
    assert_eq!(code, Some(1), "a batch that did not run fails: stdout {stdout:?} stderr {stderr:?}");
    assert!(stdout.contains("passed: 0, failed: 0"), "{stdout:?}");
    assert!(
        stdout.contains("FAIL: cargo nextest exited 101 without a status line; the batch itself did not run"),
        "{stdout:?}"
    );
}

/// All green with a clean nextest exit is OK and says no failure was tolerated.
#[cfg(unix)]
#[test]
fn interop_script_passes_an_all_green_batch() {
    let stub = CargoStub::new("interop-green");
    let transcript = [
        nextest_line("PASS", 1, 2, "python_to_rust_channel_roundtrip"),
        nextest_line("PASS", 2, 2, "rust_to_python_link_roundtrip"),
        summary_line(2, 2, 0),
    ]
    .join("\n");
    let (code, stdout, stderr) = run_interop_with_transcript(&stub, &transcript, 0);
    assert_eq!(code, Some(0), "all green passes: stdout {stdout:?} stderr {stderr:?}");
    assert!(stdout.contains("OK: 2 passed; every failure is tolerated: none"), "{stdout:?}");
}

/// nextest marks a passing test that leaked a child as `LEAK`, one over the
/// slow threshold as `SLOW` and one that passed on a retry as `FLAKY n/m`;
/// all three are passes in its Summary and must count as passes here, or the
/// cross-check fails a green batch.
#[cfg(unix)]
#[test]
fn interop_script_counts_leak_slow_and_flaky_as_passes() {
    let stub = CargoStub::new("interop-pass-vocabulary");
    let transcript = [
        nextest_line("PASS", 1, 4, "python_to_rust_channel_roundtrip"),
        nextest_line("LEAK", 2, 4, "rust_to_python_link_roundtrip"),
        nextest_line("SLOW", 3, 4, "rust_to_python_buffer_roundtrip"),
        nextest_line("TRY 1 FAIL", 4, 4, "python_to_rust_request_roundtrip"),
        nextest_line("TRY 2 PASS", 4, 4, "python_to_rust_request_roundtrip"),
        "     Summary [   0.400s] 4 tests run: 4 passed (1 slow, 1 flaky, 1 leaky), 0 skipped".to_string(),
        nextest_line("FLAKY 2/2", 4, 4, "python_to_rust_request_roundtrip"),
    ]
    .join("\n");
    let (code, stdout, stderr) = run_interop_with_transcript(&stub, &transcript, 0);
    assert_eq!(code, Some(0), "leak, slow and flaky are passes: stdout {stdout:?} stderr {stderr:?}");
    assert!(stdout.contains("passed: 4, failed: 0"), "{stdout:?}");
    assert!(stdout.contains("OK: 4 passed; every failure is tolerated: none"), "{stdout:?}");
}

/// Status lines and the Summary must agree: a batch whose parsed counts fall
/// short of what nextest tallied (a status line the parser did not
/// recognise, output lost between nextest and the log) is a failure, not a
/// smaller pass.
#[cfg(unix)]
#[test]
fn interop_script_fails_when_the_parsed_counts_disagree_with_the_summary() {
    let stub = CargoStub::new("interop-summary-mismatch");
    let transcript = [
        nextest_line("PASS", 1, 3, "python_to_rust_channel_roundtrip"),
        nextest_line("PASS", 2, 3, "rust_to_python_link_roundtrip"),
        summary_line(3, 3, 0),
    ]
    .join("\n");
    let (code, stdout, stderr) = run_interop_with_transcript(&stub, &transcript, 0);
    assert_eq!(code, Some(1), "a count mismatch fails: stdout {stdout:?} stderr {stderr:?}");
    assert!(stdout.contains("passed: 2, failed: 0"), "{stdout:?}");
    assert!(
        stdout.contains("FAIL: parsed 2 passed, 0 failed but the nextest Summary says 3 passed, 0 failed"),
        "{stdout:?}"
    );
    assert!(!stdout.contains("OK:"), "never OK on a mismatch, got {stdout:?}");
}

/// Status lines without a Summary mean nextest did not finish the batch;
/// that is a failure even when nextest's exit was clean.
#[cfg(unix)]
#[test]
fn interop_script_fails_without_a_summary_line() {
    let stub = CargoStub::new("interop-no-summary");
    let transcript = [
        nextest_line("PASS", 1, 2, "python_to_rust_channel_roundtrip"),
        nextest_line("PASS", 2, 2, "rust_to_python_link_roundtrip"),
    ]
    .join("\n");
    let (code, stdout, stderr) = run_interop_with_transcript(&stub, &transcript, 0);
    assert_eq!(code, Some(1), "a missing Summary fails: stdout {stdout:?} stderr {stderr:?}");
    assert!(
        stdout.contains("FAIL: no Summary line in the nextest output; the batch did not finish"),
        "{stdout:?}"
    );
    assert!(!stdout.contains("OK:"), "never OK without a Summary, got {stdout:?}");
}

/// nextest reports a test whose process died on a signal as `SIGABRT [` (or
/// `SIGSEGV [`, `TIMEOUT [`) and counts it as failed in its Summary; such a
/// test is a failure outside the tolerated set and must fail the run, with
/// or without a tolerated `FAIL` beside it.
#[cfg(unix)]
#[test]
fn interop_script_fails_when_a_test_dies_on_a_signal_beside_a_tolerated_failure() {
    let stub = CargoStub::new("interop-sigabrt");
    let transcript = [
        nextest_line("PASS", 1, 3, "python_to_rust_channel_roundtrip"),
        nextest_line("SIGABRT", 2, 3, "rust_to_python_link_roundtrip"),
        "    (test aborted with signal 6: SIGABRT)".to_string(),
        nextest_line("FAIL", 3, 3, PIN_DRIFT_TEST),
        summary_line(3, 1, 2),
    ]
    .join("\n");
    let (code, stdout, stderr) = run_interop_with_transcript(&stub, &transcript, 100);
    assert!(
        matches!(code, Some(c) if c != 0),
        "a test that died on a signal is an untolerated failure: stdout {stdout:?} stderr {stderr:?}"
    );
    assert!(
        stdout.contains("rust_to_python_link_roundtrip"),
        "the signalled test is named, got {stdout:?}"
    );
    assert!(!stdout.contains("OK:"), "never OK with a signalled test, got {stdout:?}");
}

/// The interop script takes no positional arguments even with the checkout set.
#[cfg(unix)]
#[test]
fn interop_script_refuses_arguments_with_usage() {
    let repo = repo_root();
    let output = Command::new("/bin/bash")
        .arg(repo.join("tools/scripts").join(INTEROP_SCRIPT))
        .arg("--release")
        .env("RETICULUM_PY_REPO", &repo)
        .current_dir(&repo)
        .output()
        .expect("run the interop script with an argument");
    assert_eq!(output.status.code(), Some(2), "an argument is a usage error");
    assert!(
        String::from_utf8_lossy(&output.stderr).starts_with("usage:"),
        "the refusal is the usage line"
    );
}
