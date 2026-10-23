// Verdict probes for the arms probe4 and probe5 leave unpinned, written from
// the acceptance text before the scripts were read: a listed environment red
// is tolerated on `TIMEOUT` as well as on `FAIL` (and only on a Darwin host:
// "on other OSes nothing is tolerated"); the workspace classify script fails
// any `await_holding_invalid_type` diagnostic with the lint live even when
// cargo exits 0, tolerates the Darwin dead_code only in exactly the two
// listed files, tails cargo's stderr when the invocation itself fails, and
// says "no error-level diagnostic" on an all-green run. Same stub toolchain
// as probe4/probe5: nothing here runs cargo or nextest. The last two arms pin
// the codeless error-level record: a compile or link failure next to the
// tolerated dead_code fails the run, while rustc's "aborting due to" summary
// that always closes the failed crate does not.

/// A listed Darwin red that nextest reports as `TIMEOUT` is the other shape
/// the environment red takes: it lands in the Summary's `timed out` bucket,
/// is tolerated, and the verdict names it with that token.
#[cfg(unix)]
#[test]
fn interop_script_tolerates_a_listed_darwin_red_on_timeout_and_prints_its_token() {
    let stub = CargoStub::new("interop-darwin-timeout");
    add_stub_tool(&stub, "uname", "Darwin");
    let transcript = [
        nextest_line("PASS", 1, 3, "python_to_rust_channel_roundtrip"),
        nextest_line("TIMEOUT", 2, 3, DARWIN_RED),
        nextest_line("PASS", 3, 3, "rust_to_python_link_roundtrip"),
        "     Summary [  61.000s] 3 tests run: 2 passed, 1 timed out, 0 skipped".to_string(),
    ]
    .join("\n");
    let (code, stdout, stderr) = run_interop_with_transcript(&stub, &transcript, 100);
    assert_eq!(
        code,
        Some(0),
        "a listed red on TIMEOUT is tolerated: stdout {stdout:?} stderr {stderr:?}"
    );
    assert!(stdout.contains("passed: 2, failed: 1"), "{stdout:?}");
    assert!(
        stdout
            .contains(&format!("OK: 2 passed; every failure is tolerated: {DARWIN_RED} (TIMEOUT)")),
        "the verdict names the tolerated red with its token, got {stdout:?}"
    );
    assert!(!stdout.contains("FAIL:"), "nothing is refused, got {stdout:?}");
}

/// The Darwin list applies on Darwin only. With `uname -s` answering `Linux`
/// the same listed name failing the same way is an untolerated failure: the
/// tolerated set printed before the batch holds only the pin-drift test, and
/// the run fails naming the red with its token.
#[cfg(unix)]
#[test]
fn interop_script_does_not_tolerate_a_darwin_red_when_the_host_is_linux() {
    let stub = CargoStub::new("interop-linux-darwin-red");
    add_stub_tool(&stub, "uname", "Linux");
    let transcript = [
        nextest_line("PASS", 1, 3, "python_to_rust_channel_roundtrip"),
        nextest_line("FAIL", 2, 3, DARWIN_RED),
        nextest_line("PASS", 3, 3, "rust_to_python_link_roundtrip"),
        summary_line(3, 2, 1),
    ]
    .join("\n");
    let (code, stdout, stderr) = run_interop_with_transcript(&stub, &transcript, 100);
    assert_eq!(
        code,
        Some(1),
        "a Darwin red is not tolerated on Linux: stdout {stdout:?} stderr {stderr:?}"
    );
    assert!(
        stdout.contains(&format!("tolerated failures (Linux): {PIN_DRIFT_TEST}")),
        "only the pin-drift test is tolerated on Linux, got {stdout:?}"
    );
    assert!(
        !stdout.contains(&format!("tolerated failures (Linux): {DARWIN_RED}")),
        "the Darwin list must not be announced on Linux, got {stdout:?}"
    );
    assert!(stdout.contains("FAIL: 1 failure(s) outside the tolerated set:"), "{stdout:?}");
    assert!(
        stdout.lines().any(|line| line.trim() == format!("{DARWIN_RED} (FAIL)")),
        "the red is named with its token, got {stdout:?}"
    );
    assert!(!stdout.contains("OK:"), "never OK, got {stdout:?}");
}

/// Runs the workspace classify script at the repository root with `uname -s`
/// answering `host` and the stub `cargo` emitting `records` then exiting
/// `status`; every tracked manifest of this tree is clean, so the manifest
/// gate passes and the verdict is decided by the records alone.
#[cfg(unix)]
fn run_classify_with_records(
    label: &str,
    host: &str,
    records: &[String],
    status: i32,
) -> (Option<i32>, String, String) {
    let stub = CargoStub::new(label);
    add_stub_tool(&stub, "uname", host);
    let mut out = records.to_vec();
    out.push(BUILD_FINISHED.to_string());
    run_script_with_stub_toolchain(WORKSPACE_CLIPPY_SCRIPT, &stub, &out.join("\n"), status, &[])
}

/// The lint is live at its default (warn) level, so a hit never fails cargo
/// itself; the script must still fail the run on one warning-level
/// diagnostic of the lint, whatever the host, counting it in its report.
#[cfg(unix)]
#[test]
fn classify_script_fails_one_warning_level_guard_lint_hit_even_when_cargo_exits_zero() {
    for host in ["Darwin", "Linux"] {
        let (code, stdout, stderr) = run_classify_with_records(
            &format!("classify-hit-{}", host.to_ascii_lowercase()),
            host,
            &[clippy_hit("crates/libs/rns-transport/src/transport/jobs.rs")],
            0,
        );
        assert_eq!(
            code,
            Some(1),
            "{host}: one hit fails the run: stdout {stdout:?} stderr {stderr:?}"
        );
        assert!(stdout.contains("cargo clippy (CI invocation) exit: 0"), "{host}: {stdout:?}");
        assert!(
            stdout.contains("await_holding_invalid_type diagnostics (any level): 1"),
            "{host}: the hit is counted, got {stdout:?}"
        );
        assert!(
            stdout.contains("FAIL: await_holding_invalid_type fired outside the per-target allows"),
            "{host}: the failure names the lint, got {stdout:?}"
        );
        assert!(!stdout.contains("OK:"), "{host}: never OK with a hit, got {stdout:?}");
    }
}

/// The Darwin tolerance is the two listed files exactly: the same dead_code
/// in a third file is refused on Darwin too, and the refusal names the file.
#[cfg(unix)]
#[test]
fn classify_script_refuses_dead_code_outside_the_two_listed_files_even_on_darwin() {
    let stray = "crates/apps/reticulumd/tests/python_channel_interop_parts/channel_roundtrip.rs";
    let (code, stdout, stderr) = run_classify_with_records(
        "classify-darwin-stray-dead-code",
        "Darwin",
        &[
            dead_code_error(
                "crates/apps/reticulumd/tests/python_channel_interop_parts/resource_memory_profile.rs",
            ),
            dead_code_error("crates/apps/reticulumd/tests/support/python_channel_process.rs"),
            dead_code_error(stray),
        ],
        101,
    );
    assert_eq!(
        code,
        Some(1),
        "a third dead_code file is refused on Darwin: stdout {stdout:?} stderr {stderr:?}"
    );
    assert!(stdout.contains("error-level diagnostics by code: {'dead_code': 3}"), "{stdout:?}");
    assert!(
        stdout.contains(&format!("error dead_code: ['{stray}']")),
        "the stray file is listed, got {stdout:?}"
    );
    assert!(
        stdout.contains("FAIL: 1 error diagnostic(s) outside the tolerated set (Darwin: the macOS-only dead_code"),
        "{stdout:?}"
    );
    assert!(!stdout.contains("OK:"), "never OK with a stray error, got {stdout:?}");
}

/// A clippy invocation that fails without any diagnostic (a build error, a
/// missing component) is not a pass, and the tail of cargo's stderr is
/// surfaced on stdout so the failure is readable from the script's output.
#[cfg(unix)]
#[test]
fn classify_script_fails_a_broken_clippy_invocation_and_tails_cargo_stderr() {
    let stub = CargoStub::new("classify-cargo-broken");
    add_stub_tool(&stub, "uname", "Darwin");
    let marker = "PROBE_CARGO_ERR: error: could not compile `reticulumd`";
    add_cargo_stub_with_stderr(&stub, marker);
    let (code, stdout, stderr) =
        run_script_with_stub_toolchain(WORKSPACE_CLIPPY_SCRIPT, &stub, BUILD_FINISHED, 101, &[]);
    assert_eq!(code, Some(1), "a broken invocation fails: stdout {stdout:?} stderr {stderr:?}");
    assert!(stdout.contains("cargo clippy (CI invocation) exit: 101"), "{stdout:?}");
    assert!(
        stdout.contains("cargo clippy stderr (last 40 lines):"),
        "the stderr tail is announced, got {stdout:?}"
    );
    assert!(stdout.contains(marker), "cargo's stderr reaches stdout, got {stdout:?}");
    assert!(
        !stderr.contains(marker),
        "cargo's stderr is captured, not passed through, got {stderr:?}"
    );
    assert!(
        stdout.contains("FAIL: cargo clippy exited 101 without any error-level diagnostic"),
        "the failure names the exit, got {stdout:?}"
    );
    assert!(!stdout.contains("OK:"), "never OK on a broken invocation, got {stdout:?}");
}

/// An all-green run says so without claiming a tolerated error: the verdict
/// ends in "no error-level diagnostic", the stderr tail is not printed, and
/// the host's tolerance is announced either way.
#[cfg(unix)]
#[test]
fn classify_script_reports_no_error_level_diagnostic_on_an_all_green_run() {
    for (host, tolerance) in [
        ("Darwin", "the macOS-only dead_code in the linux-only reticulumd test target"),
        ("Linux", "none"),
    ] {
        let (code, stdout, stderr) = run_classify_with_records(
            &format!("classify-green-{}", host.to_ascii_lowercase()),
            host,
            &[],
            0,
        );
        assert_eq!(
            code,
            Some(0),
            "{host}: a clean run passes: stdout {stdout:?} stderr {stderr:?}"
        );
        assert!(
            stdout.contains(&format!("tolerated errors ({host}): {tolerance}")),
            "{host}: {stdout:?}"
        );
        assert!(stdout.contains("error-level diagnostics by code: {}"), "{host}: {stdout:?}");
        assert!(
            stdout.contains("OK: zero await_holding_invalid_type diagnostics with the lint live at its default level; no error-level diagnostic"),
            "{host}: the verdict says no error was tolerated, got {stdout:?}"
        );
        assert!(
            !stdout.contains("every error is"),
            "{host}: no tolerance is claimed on a green run, got {stdout:?}"
        );
        assert!(
            !stdout.contains("cargo clippy stderr"),
            "{host}: no stderr tail on success, got {stdout:?}"
        );
    }
}

/// An error-level compiler-message with no lint or error code, as rustc
/// emits for parse, macro and linker failures and for its closing summary.
#[cfg(unix)]
fn codeless_error(message: &str) -> String {
    format!(
        r#"{{"reason":"compiler-message","message":{{"code":null,"level":"error","message":"{message}","spans":[]}}}}"#
    )
}

/// On Darwin the tolerated dead_code makes cargo exit 101 on every run, so a
/// broken unrelated crate hides behind it unless codeless error records are
/// counted: one linker failure next to the two tolerated records fails the
/// run, and the refusal names the failure's text.
#[cfg(unix)]
#[test]
fn classify_script_refuses_a_codeless_error_next_to_the_tolerated_darwin_dead_code() {
    let linker = "linking with `cc` failed: exit status: 1";
    let (code, stdout, stderr) = run_classify_with_records(
        "classify-darwin-codeless-error",
        "Darwin",
        &[
            dead_code_error(
                "crates/apps/reticulumd/tests/python_channel_interop_parts/resource_memory_profile.rs",
            ),
            dead_code_error("crates/apps/reticulumd/tests/support/python_channel_process.rs"),
            codeless_error(linker),
        ],
        101,
    );
    assert_eq!(
        code,
        Some(1),
        "a codeless error is never tolerated: stdout {stdout:?} stderr {stderr:?}"
    );
    assert!(stdout.contains("error-level diagnostics by code: {'dead_code': 2}"), "{stdout:?}");
    assert!(stdout.contains("codeless error-level diagnostics: 1"), "{stdout:?}");
    assert!(
        stdout.contains(&format!("  error (no code): {linker}")),
        "the codeless record is listed with its text, got {stdout:?}"
    );
    assert!(
        stdout.contains(&format!("FAIL: 1 codeless error-level diagnostic(s), a compile or link failure rather than a lint: {linker}")),
        "the refusal names the failure, got {stdout:?}"
    );
    assert!(!stdout.contains("OK:"), "never OK with a codeless error, got {stdout:?}");
}

/// rustc closes the crate carrying the tolerated dead_code with a codeless
/// "aborting due to N previous errors" record; it restates the two counted
/// errors and must not turn the tolerated Darwin run into a failure.
#[cfg(unix)]
#[test]
fn classify_script_skips_the_aborting_summary_that_closes_the_tolerated_darwin_crate() {
    let (code, stdout, stderr) = run_classify_with_records(
        "classify-darwin-aborting-summary",
        "Darwin",
        &[
            dead_code_error(
                "crates/apps/reticulumd/tests/python_channel_interop_parts/resource_memory_profile.rs",
            ),
            dead_code_error("crates/apps/reticulumd/tests/support/python_channel_process.rs"),
            codeless_error("aborting due to 2 previous errors"),
        ],
        101,
    );
    assert_eq!(code, Some(0), "the summary is skipped: stdout {stdout:?} stderr {stderr:?}");
    assert!(stdout.contains("error-level diagnostics by code: {'dead_code': 2}"), "{stdout:?}");
    assert!(stdout.contains("codeless error-level diagnostics: 0"), "{stdout:?}");
    assert!(
        stdout.contains("OK: zero await_holding_invalid_type diagnostics with the lint live at its default level; every error is the macOS-only dead_code"),
        "{stdout:?}"
    );
    assert!(!stdout.contains("FAIL:"), "nothing is refused, got {stdout:?}");
}
