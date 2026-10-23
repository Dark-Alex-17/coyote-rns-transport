// Verdict probes for the arms the sibling probe4 leaves unpinned: every
// nextest status token classified (`TIMEOUT` and `TRY n FAIL` count as
// failures, once per test), the counts cross-checked against the `Summary`
// line including its `timed out` bucket and its cancelled `N/M` form, a
// listed environment red tolerated on `FAIL` or `TIMEOUT` but never on a
// crash token, and for the clippy classify script the manifest grep's own
// exit code and the Darwin-only dead_code tolerance. Same stub toolchain as
// probe4, with a stub `uname` where the host decides: nothing here runs
// cargo or nextest.

/// A test nextest reports as `TIMEOUT` lands in the Summary's `timed out`
/// bucket, not `failed`; the cross-check must read that bucket and the test
/// is an untolerated failure that fails the run by name and token.
#[cfg(unix)]
#[test]
fn interop_script_fails_a_timed_out_test_counted_in_the_summary_timed_out_bucket() {
    let stub = CargoStub::new("interop-timeout");
    let transcript = [
        nextest_line("PASS", 1, 3, "python_to_rust_channel_roundtrip"),
        nextest_line("TIMEOUT", 2, 3, "rust_to_python_link_roundtrip"),
        nextest_line("FAIL", 3, 3, PIN_DRIFT_TEST),
        "     Summary [  61.000s] 3 tests run: 1 passed, 1 failed, 1 timed out, 0 skipped"
            .to_string(),
    ]
    .join("\n");
    let (code, stdout, stderr) = run_interop_with_transcript(&stub, &transcript, 100);
    assert_eq!(
        code,
        Some(1),
        "a timed-out test fails the run: stdout {stdout:?} stderr {stderr:?}"
    );
    assert!(
        stdout.contains("passed: 1, failed: 2"),
        "the timeout is counted as a failure, got {stdout:?}"
    );
    assert!(stdout.contains("FAIL: 1 failure(s) outside the tolerated set:"), "{stdout:?}");
    assert!(
        stdout.lines().any(|line| line.trim() == "rust_to_python_link_roundtrip (TIMEOUT)"),
        "the timed-out test is named with its token, got {stdout:?}"
    );
    assert!(!stdout.contains("OK:"), "never OK with a timed-out test, got {stdout:?}");
}

/// A retried test whose retries ran out: every attempt line is `TRY n FAIL`
/// and nextest reprints it as `FAIL` after the Summary. That is one failure,
/// counted once (by name, not per attempt), and it fails the run.
#[cfg(unix)]
#[test]
fn interop_script_fails_a_test_whose_retries_ran_out_counting_it_once() {
    let stub = CargoStub::new("interop-retries-exhausted");
    let transcript = [
        nextest_line("PASS", 1, 2, "python_to_rust_channel_roundtrip"),
        nextest_line("TRY 1 FAIL", 2, 2, "rust_to_python_link_roundtrip"),
        nextest_line("TRY 2 FAIL", 2, 2, "rust_to_python_link_roundtrip"),
        summary_line(2, 1, 1),
        nextest_line("FAIL", 2, 2, "rust_to_python_link_roundtrip"),
    ]
    .join("\n");
    let (code, stdout, stderr) = run_interop_with_transcript(&stub, &transcript, 100);
    assert_eq!(
        code,
        Some(1),
        "exhausted retries fail the run: stdout {stdout:?} stderr {stderr:?}"
    );
    assert!(stdout.contains("passed: 1, failed: 1"), "three lines, one failure, got {stdout:?}");
    assert!(
        stdout.lines().any(|line| line.trim() == "rust_to_python_link_roundtrip (FAIL)"),
        "the exhausted test is named with its final token, got {stdout:?}"
    );
    assert!(!stdout.contains("OK:"), "{stdout:?}");
}

#[cfg(unix)]
const DARWIN_RED: &str = "python_to_rust_backbone_channel_roundtrip";

/// Runs the interop script with `uname -s` answering `Darwin`, so the Darwin
/// list is populated whatever the host, and `DARWIN_RED` finishing with
/// `token` beside two passes.
#[cfg(unix)]
fn run_interop_with_darwin_red(label: &str, token: &str) -> (Option<i32>, String, String) {
    let stub = CargoStub::new(label);
    add_stub_tool(&stub, "uname", "Darwin");
    let transcript = [
        nextest_line("PASS", 1, 3, "python_to_rust_channel_roundtrip"),
        nextest_line(token, 2, 3, DARWIN_RED),
        nextest_line("PASS", 3, 3, "rust_to_python_link_roundtrip"),
        summary_line(3, 2, 1),
    ]
    .join("\n");
    run_interop_with_transcript(&stub, &transcript, 100)
}

/// A listed Darwin environment red is tolerated when it fails the way the
/// environment makes it fail, and the verdict names it with its token.
#[cfg(unix)]
#[test]
fn interop_script_tolerates_a_listed_darwin_red_on_fail_and_prints_its_token() {
    let (code, stdout, stderr) = run_interop_with_darwin_red("interop-darwin-fail", "FAIL");
    assert_eq!(
        code,
        Some(0),
        "a listed red on FAIL is tolerated: stdout {stdout:?} stderr {stderr:?}"
    );
    assert!(stdout.contains("tolerated failures (Darwin):"), "{stdout:?}");
    assert!(stdout.contains("passed: 2, failed: 1"), "{stdout:?}");
    assert!(
        stdout.contains(&format!("OK: 2 passed; every failure is tolerated: {DARWIN_RED} (FAIL)")),
        "the verdict names the tolerated red with its token, got {stdout:?}"
    );
}

/// The same listed name dying on a signal is a crash, not the known
/// environment red: the tolerance is by name and token, so it fails the run
/// and is named with the token that refused it.
#[cfg(unix)]
#[test]
fn interop_script_refuses_a_listed_darwin_red_that_died_on_a_signal() {
    let (code, stdout, stderr) = run_interop_with_darwin_red("interop-darwin-sigabrt", "SIGABRT");
    assert_eq!(
        code,
        Some(1),
        "a listed red on SIGABRT is a crash, never tolerated: stdout {stdout:?} stderr {stderr:?}"
    );
    assert!(stdout.contains("passed: 2, failed: 1"), "the signal is one failure, got {stdout:?}");
    assert!(stdout.contains("FAIL: 1 failure(s) outside the tolerated set:"), "{stdout:?}");
    assert!(
        stdout.lines().any(|line| line.trim() == format!("{DARWIN_RED} (SIGABRT)")),
        "the crashed red is named with its token, got {stdout:?}"
    );
    assert!(!stdout.contains("OK:"), "never OK on a crash, got {stdout:?}");
}

/// nextest writes `N/M tests run` when the batch was cancelled before every
/// test ran; a cancelled batch is not a smaller pass even when every test
/// that did run passed and the counts agree.
#[cfg(unix)]
#[test]
fn interop_script_fails_a_cancelled_batch_that_ran_only_some_of_its_tests() {
    let stub = CargoStub::new("interop-cancelled");
    let transcript = [
        nextest_line("PASS", 1, 5, "python_to_rust_channel_roundtrip"),
        nextest_line("PASS", 2, 5, "rust_to_python_link_roundtrip"),
        "Canceling due to interrupt: 1 test still running".to_string(),
        "     Summary [   0.900s] 2/5 tests run: 2 passed, 3 skipped".to_string(),
    ]
    .join("\n");
    let (code, stdout, stderr) = run_interop_with_transcript(&stub, &transcript, 130);
    assert_eq!(code, Some(1), "a cancelled batch fails: stdout {stdout:?} stderr {stderr:?}");
    assert!(
        stdout.contains("FAIL: nextest ran 2 of 5 tests; the batch was cancelled"),
        "{stdout:?}"
    );
    assert!(!stdout.contains("OK:"), "never OK on a cancelled batch, got {stdout:?}");
}

/// A `RETICULUM_PY_REPO` that is not a git checkout is refused with a verdict
/// line before the batch, not with git's bare error. The stub directory
/// carries a dangling `gitdir` pointer so git fails there at once instead of
/// walking up to whatever repository the temp dir may live under.
#[cfg(unix)]
#[test]
fn interop_script_fails_naming_a_checkout_that_is_not_a_git_repository() {
    let stub = CargoStub::new("interop-not-a-repo");
    let not_a_repo = stub.dir.to_str().expect("utf-8 stub dir");
    fs::write(
        stub.dir.join(".git"),
        format!("gitdir: {}\n", stub.dir.join(".git-missing").display()),
    )
    .expect("write a dangling gitdir pointer");
    let (code, stdout, stderr) = run_script_with_stub_toolchain(
        INTEROP_SCRIPT,
        &stub,
        "",
        0,
        &[("RETICULUM_PY_REPO", not_a_repo)],
    );
    assert_eq!(code, Some(1), "a non-git checkout fails: stdout {stdout:?} stderr {stderr:?}");
    assert!(
        stdout.contains(&format!("FAIL: RETICULUM_PY_REPO={not_a_repo} is not a git checkout")),
        "the refusal names the path, got {stdout:?}"
    );
    assert!(!stdout.contains("cargo nextest"), "the batch never starts, got {stdout:?}");
}

/// The manifest grep's own exit code is honoured: a tracked `Cargo.toml`
/// missing from the working tree makes `grep` exit 2 (unreadable file), and
/// the script stops with that code before the clippy run instead of folding
/// it into "no mention" and passing an unchecked tree.
#[cfg(unix)]
#[test]
fn classify_script_stops_when_a_tracked_manifest_cannot_be_read_instead_of_running_clippy() {
    let clone = ProbeClone::new("classify-unreadable-manifest");
    let stub = CargoStub::new("classify-unreadable");
    let manifest = "crates/apps/rns-tools/Cargo.toml";
    assert!(
        clone.git(&["ls-files", "--", manifest]).trim() == manifest,
        "the probe's manifest is tracked"
    );
    fs::remove_file(clone.path(manifest))
        .expect("remove the tracked manifest from the working tree only");

    let (code, stdout, stderr) = run_classify_with_stub_cargo(&clone, &stub);
    assert_eq!(
        code,
        Some(2),
        "grep's unreadable-file exit is the script's exit: stdout {stdout:?} stderr {stderr:?}"
    );
    assert!(
        stderr.contains(manifest) && stderr.contains("No such file"),
        "grep's own error names the manifest, got stderr {stderr:?}"
    );
    assert!(
        !stdout.contains(CLIPPY_RAN),
        "the script never reaches the clippy run, got {stdout:?}"
    );
    assert!(!stdout.contains("OK:"), "no verdict is printed, got {stdout:?}");
}

#[cfg(unix)]
const WORKSPACE_CLIPPY_SCRIPT: &str = "workspace-clippy-classify.sh";

#[cfg(unix)]
fn dead_code_error(file_name: &str) -> String {
    format!(
        r#"{{"reason":"compiler-message","message":{{"code":{{"code":"dead_code"}},"level":"error","message":"function `spawn` is never used","spans":[{{"is_primary":true,"file_name":"{file_name}"}}]}}}}"#
    )
}

/// Runs the workspace classify script with `uname -s` answering `host` and
/// the stub `cargo` emitting the two dead_code errors macOS produces in the
/// linux-only interop test target, exiting 101 as `-D warnings` makes it.
#[cfg(unix)]
fn run_classify_with_darwin_dead_code(label: &str, host: &str) -> (Option<i32>, String, String) {
    let stub = CargoStub::new(label);
    add_stub_tool(&stub, "uname", host);
    let out = [
        dead_code_error(
            "crates/apps/reticulumd/tests/python_channel_interop_parts/resource_memory_profile.rs",
        ),
        dead_code_error("crates/apps/reticulumd/tests/support/python_channel_process.rs"),
        BUILD_FINISHED.to_string(),
    ]
    .join("\n");
    run_script_with_stub_toolchain(WORKSPACE_CLIPPY_SCRIPT, &stub, &out, 101, &[])
}

/// On Darwin the two dead_code errors are the known condition and the run
/// is OK, saying so.
#[cfg(unix)]
#[test]
fn classify_script_tolerates_the_interop_target_dead_code_on_darwin() {
    let (code, stdout, stderr) =
        run_classify_with_darwin_dead_code("classify-darwin-dead-code", "Darwin");
    assert_eq!(
        code,
        Some(0),
        "the Darwin dead_code is tolerated: stdout {stdout:?} stderr {stderr:?}"
    );
    assert!(stdout.contains("error-level diagnostics by code: {'dead_code': 2}"), "{stdout:?}");
    assert!(
        stdout.contains("OK: zero await_holding_invalid_type diagnostics with the lint live at its default level; every error is the macOS-only dead_code"),
        "{stdout:?}"
    );
}

/// On Linux, where CI runs and the interop target is built, the same two
/// diagnostics are real dead code: nothing is tolerated and the run fails.
#[cfg(unix)]
#[test]
fn classify_script_fails_the_same_dead_code_on_linux_where_nothing_is_tolerated() {
    let (code, stdout, stderr) =
        run_classify_with_darwin_dead_code("classify-linux-dead-code", "Linux");
    assert_eq!(code, Some(1), "nothing is tolerated on Linux: stdout {stdout:?} stderr {stderr:?}");
    assert!(stdout.contains("tolerated errors (Linux): none"), "{stdout:?}");
    assert!(
        stdout.contains("FAIL: 2 error diagnostic(s) outside the tolerated set (Linux: none)"),
        "{stdout:?}"
    );
    assert!(!stdout.contains("OK:"), "never OK with untolerated errors, got {stdout:?}");
}
