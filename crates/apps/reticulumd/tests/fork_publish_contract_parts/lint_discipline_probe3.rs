// Usage probes for the two evidence scripts that sit next to the classify
// script: the cross-target clippy run and the Python-in-the-loop interop
// batch. Both take minutes for real, so the probes pin the words, the
// invocation and the early exits (a skip without zig, a usage refusal
// without the Python checkout) and never reach cargo.

#[cfg(unix)]
fn is_executable(relative: &str) -> bool {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(repo_root().join(relative))
        .map(|meta| meta.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

/// Runs `tools/scripts/<script>` from the repository root through
/// `/bin/bash` with `env` replacing the named variables (`None` removes one)
/// and returns `(exit code, stdout, stderr)`. The absolute interpreter path
/// lets a probe empty `PATH` without losing bash itself.
#[cfg(unix)]
fn run_repo_script(script: &str, env: &[(&str, Option<&str>)]) -> (Option<i32>, String, String) {
    let mut command = Command::new("/bin/bash");
    command.arg(repo_root().join("tools/scripts").join(script)).current_dir(repo_root());
    for (key, value) in env {
        match value {
            Some(value) => command.env(key, value),
            None => command.env_remove(key),
        };
    }
    let output =
        command.output().unwrap_or_else(|err| panic!("run tools/scripts/{script}: {err}"));
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// The cross-target script runs clippy for the Linux target with zig as the
/// C compiler over the workspace minus the two dbus-dependent crates, never
/// under `-D warnings`, and skips (exit 0, `SKIP:`) when zig is absent
/// before any cargo run.
#[cfg(unix)]
#[test]
fn cross_target_classify_script_names_its_invocation_and_skips_without_zig() {
    let script = "cross-target-clippy-classify.sh";
    let relative = format!("tools/scripts/{script}");
    assert!(is_executable(&relative), "{relative} exists and is executable");
    let text = read_repo_file(&relative);
    for needle in [
        "x86_64-unknown-linux-gnu",
        "zig cc",
        "zig ar",
        "CC_x86_64_unknown_linux_gnu",
        "AR_x86_64_unknown_linux_gnu",
        "--exclude reticulumd",
        "--exclude rns-tools",
        "--no-deps",
        "--message-format=json",
        "a skip is not a pass",
    ] {
        assert!(text.contains(needle), "{relative} must say `{needle}`");
    }
    let invocation = text
        .lines()
        .find(|line| line.starts_with("cargo clippy"))
        .expect("the script runs cargo clippy on a line of its own");
    assert!(
        !invocation.contains("-D warnings"),
        "the cross-target run does not deny warnings, got `{invocation}`"
    );

    let empty = std::env::temp_dir().join(format!("rns-fork-probe-empty-path-{}", std::process::id()));
    fs::create_dir_all(&empty).expect("create the empty PATH directory");
    let empty_path = empty.to_str().expect("utf-8 temp path");
    let (code, stdout, stderr) = run_repo_script(script, &[("PATH", Some(empty_path))]);
    let _ = fs::remove_dir_all(&empty);
    assert_eq!(code, Some(0), "a missing zig skips: stdout {stdout:?} stderr {stderr:?}");
    assert!(
        stdout.lines().any(|line| line.starts_with("SKIP:") && line.contains("zig")),
        "the skip names zig, got {stdout:?}"
    );
    assert!(!stdout.contains("cargo clippy"), "no cargo run on a skip, got {stdout:?}");
    assert!(!stdout.contains("OK:"), "a skip is not a pass, got {stdout:?}");
}

/// The tolerated-failure array near the top of the interop script: the
/// block between `DARWIN_ENVIRONMENT_FAILURES=(` and its closing paren.
#[cfg(unix)]
fn tolerated_block(script: &str) -> &str {
    let start = script.find("DARWIN_ENVIRONMENT_FAILURES=(").expect("the tolerated array");
    let end = script[start..].find(')').expect("the array closes");
    &script[start..start + end]
}

/// The interop script refuses to run without the Python checkout, tolerates
/// exactly the recorded Darwin environment reds plus the pin test, and
/// decides the pin test from the checkout revision against the constant in
/// the reference test.
#[cfg(unix)]
#[test]
fn python_interop_classify_script_refuses_without_the_checkout_and_lists_its_tolerated_reds() {
    let script = "python-interop-classify.sh";
    let relative = format!("tools/scripts/{script}");
    assert!(is_executable(&relative), "{relative} exists and is executable");
    let text = read_repo_file(&relative);
    for needle in [
        "cargo nextest run -j 1 --no-fail-fast",
        "-p reticulumd --test python_channel_interop --run-ignored only --no-capture",
        "uname -s",
        "rev-parse HEAD",
        "PINNED_RETICULUM",
        "python_channel_interop_parts/resource_timeout_reference.rs",
        "PIN_DRIFT_FAILURE=pinned_python_resource_cancels_after_missing_part_retry_budget",
    ] {
        assert!(text.contains(needle), "{relative} must say `{needle}`");
    }
    let block = tolerated_block(&text);
    let names: Vec<&str> = block.lines().skip(1).map(str::trim).filter(|l| !l.is_empty()).collect();
    let expected = [
        "python_to_rust_backbone_channel_roundtrip",
        "python_to_rust_backbone_link_data_roundtrip",
        "python_to_rust_backbone_request_response_roundtrip",
        "python_to_rust_backbone_resource_backed_request_response_roundtrip",
        "rust_to_python_backbone_channel_roundtrip",
        "rust_to_python_backbone_link_data_roundtrip",
        "rust_to_python_backbone_raw_resource_roundtrip",
        "rust_to_python_backbone_request_response_roundtrip",
        "rust_to_python_channel_buffer_roundtrip",
        "python_rust_ax25_kiss_serial_channel_roundtrip_over_pty",
        "python_rust_ifac_kiss_serial_channel_roundtrip_over_pty",
    ];
    assert_eq!(names, expected, "the Darwin array lists exactly the recorded environment reds");
    assert!(
        text.lines().position(|l| l.starts_with("DARWIN_ENVIRONMENT_FAILURES=(")).unwrap_or(usize::MAX)
            < 40,
        "the tolerated array sits near the top of the script"
    );

    let (code, stdout, stderr) = run_repo_script(script, &[("RETICULUM_PY_REPO", None)]);
    assert!(
        matches!(code, Some(c) if c != 0),
        "no checkout refuses: code {code:?} stdout {stdout:?} stderr {stderr:?}"
    );
    assert!(
        stderr.lines().any(|line| line.starts_with("usage:") && line.contains("RETICULUM_PY_REPO")),
        "the refusal is the usage line, got stderr {stderr:?}"
    );
    assert!(!stdout.contains("cargo nextest"), "no nextest run without the checkout, got {stdout:?}");
}
