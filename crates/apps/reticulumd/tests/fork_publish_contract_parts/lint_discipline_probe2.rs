// Usage probes for the classify script's manifest gate, written from the
// acceptance text: the script "refuses any tracked Cargo.toml mentioning the
// lint (manifest listing fatal on failure)". The sibling probe pins the
// refusal for one allowing reticulumd manifest; these pin the three clauses
// around it: a mention at any level in any tracked manifest is refused, an
// untracked mention is not, and a manifest listing that fails stops the
// script instead of letting it run clippy over an unchecked tree. A stub
// `cargo` first on PATH answers the clippy run in milliseconds, so a script
// that falls through to it is caught by the "no clippy run" assertion and not
// by a timeout.

/// Runs the working-tree classify script over `clone` with a stub `cargo`
/// that prints one `build-finished` record, i.e. a clippy run that found
/// nothing. Returns `(exit code, stdout, stderr)`.
#[cfg(unix)]
fn run_classify_with_stub_cargo(
    clone: &ProbeClone,
    stub: &CargoStub,
) -> (Option<i32>, String, String) {
    let script = "workspace-clippy-classify.sh";
    fs::copy(
        repo_root().join("tools/scripts").join(script),
        clone.path("tools/scripts").join(script),
    )
    .expect("copy the working-tree classify script into the clone");
    let path = format!("{}:{}", stub.dir.display(), std::env::var("PATH").unwrap_or_default());
    let output = Command::new("bash")
        .arg(clone.path("tools/scripts").join(script))
        .env("PATH", path)
        .env("PROBE_CARGO_OUT", r#"{"reason":"build-finished","success":true}"#)
        .env("PROBE_CARGO_STATUS", "0")
        .current_dir(clone.path(""))
        .output()
        .expect("run tools/scripts/workspace-clippy-classify.sh in the clone");
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[cfg(unix)]
const CLIPPY_RAN: &str = "cargo clippy (CI invocation) exit";

/// Positive control for the stub harness: with every tracked manifest clean
/// the script reaches the clippy run and reports zero diagnostics, and a
/// manifest that mentions the lint but is not tracked does not change that.
#[cfg(unix)]
#[test]
fn classify_script_ignores_an_untracked_manifest_and_reaches_the_clippy_run() {
    let clone = ProbeClone::new("classify-untracked-manifest");
    let stub = CargoStub::new("classify-untracked");
    fs::create_dir_all(clone.path("probe-untracked")).expect("create the untracked crate dir");
    fs::write(
        clone.path("probe-untracked/Cargo.toml"),
        format!("[package]\nname = \"probe\"\n\n[lints.clippy]\n{GUARD_LINT} = \"allow\"\n"),
    )
    .expect("write the untracked manifest");

    let (code, stdout, stderr) = run_classify_with_stub_cargo(&clone, &stub);
    assert_eq!(code, Some(0), "a clean tracked tree passes: stdout {stdout:?} stderr {stderr:?}");
    assert!(stdout.contains(CLIPPY_RAN), "the clippy run is reached, got {stdout:?}");
    assert!(
        stdout.contains("await_holding_invalid_type diagnostics (any level): 0"),
        "the zero count is reported, got {stdout:?}"
    );
    assert!(
        stdout.contains("OK: zero await_holding_invalid_type"),
        "the verdict is OK, got {stdout:?}"
    );
    assert!(!stdout.contains("FAIL:"), "nothing is refused, got {stdout:?}");
}

/// Any mention is refused, not only `allow` and not only in reticulumd: a
/// `warn` entry in the workspace table of the root manifest fails the gate by
/// name before any cargo run.
#[cfg(unix)]
#[test]
fn classify_script_refuses_a_tracked_root_manifest_that_mentions_the_lint_at_any_level() {
    let clone = ProbeClone::new("classify-root-manifest-warn");
    let stub = CargoStub::new("classify-root-warn");
    let manifest = "Cargo.toml";
    let mut staged =
        fs::read_to_string(clone.path(manifest)).expect("read the clone's root manifest");
    staged.push_str(&format!("\n[workspace.lints.clippy]\n{GUARD_LINT} = \"warn\"\n"));
    fs::write(clone.path(manifest), staged).expect("write the clone's root manifest");
    clone.git(&["add", manifest]);

    let (code, stdout, stderr) = run_classify_with_stub_cargo(&clone, &stub);
    assert_eq!(
        code,
        Some(1),
        "a mention at warn level fails the gate: stdout {stdout:?} stderr {stderr:?}"
    );
    assert!(
        stdout.contains("FAIL: a Cargo.toml mentions await_holding_invalid_type"),
        "the refusal names the gate, got {stdout:?}"
    );
    assert!(
        stdout.lines().any(|line| line.trim() == manifest),
        "the refusal lists the root manifest on its own line, got {stdout:?}"
    );
    assert!(!stdout.contains(CLIPPY_RAN), "the gate refuses before the clippy run, got {stdout:?}");
}

/// A manifest listing that fails is fatal: the script stops with git's error
/// and never reaches the clippy run, instead of treating "no manifests" as
/// "no mentions" and passing an unchecked tree.
#[cfg(unix)]
#[test]
fn classify_script_stops_when_the_manifest_listing_fails_instead_of_running_clippy() {
    let clone = ProbeClone::new("classify-listing-fatal");
    let stub = CargoStub::new("classify-listing-fatal");
    // Point the clone's `.git` at a directory that does not exist: every git
    // command in the clone then fails at once without walking up to a parent
    // repository, whatever directory the temp dir lives under.
    let git_dir = clone.path(".git");
    fs::remove_dir_all(&git_dir).expect("remove the clone's .git directory");
    fs::write(&git_dir, format!("gitdir: {}\n", clone.path(".git-missing").display()))
        .expect("write a dangling gitdir pointer");

    let started = std::time::Instant::now();
    let (code, stdout, stderr) = run_classify_with_stub_cargo(&clone, &stub);
    assert!(
        matches!(code, Some(c) if c != 0),
        "a failed manifest listing fails the script: code {code:?} stdout {stdout:?} stderr {stderr:?}"
    );
    assert!(
        stderr.contains("not a git repository") || stderr.contains("fatal:"),
        "git's own error reaches the caller, got stderr {stderr:?}"
    );
    assert!(
        !stdout.contains(CLIPPY_RAN),
        "the script never reaches the clippy run, got {stdout:?}"
    );
    assert!(!stdout.contains("OK:"), "no verdict is printed, got {stdout:?}");
    assert!(
        started.elapsed() < std::time::Duration::from_secs(10),
        "the failure is immediate, not a clippy run"
    );
}
