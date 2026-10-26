// Usage pins for the semver summary verdict and the probe clone's identity.
// The semver script's verdict is reached only after a `cargo semver-checks`
// run that takes minutes against a real baseline, so these pins put a stub
// `cargo` first on PATH that prints a scripted summary; the assertions are
// on the exit code and the words a releaser reads. The identity pin is a
// source walk: every commit the probes create must go through the clone
// that carries its own `user.name`/`user.email`, or the probe passes on a
// host that auto-detects an identity and fails on a CI runner that cannot.

/// A directory holding an executable `cargo` that prints `$PROBE_CARGO_OUT`
/// and exits with `$PROBE_CARGO_STATUS`, removed on drop.
#[cfg(unix)]
struct CargoStub {
    dir: PathBuf,
}

#[cfg(unix)]
impl CargoStub {
    fn new(name: &str) -> Self {
        use std::os::unix::fs::PermissionsExt;
        let dir =
            std::env::temp_dir().join(format!("rns-fork-probe-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create the cargo stub directory");
        let stub = dir.join("cargo");
        fs::write(
            &stub,
            "#!/usr/bin/env bash\nprintf '%s\\n' \"${PROBE_CARGO_OUT}\"\nexit \"${PROBE_CARGO_STATUS:-0}\"\n",
        )
        .expect("write the cargo stub");
        fs::set_permissions(&stub, fs::Permissions::from_mode(0o755))
            .expect("make the stub executable");
        Self { dir }
    }

    /// Runs `tools/scripts/semver-major-findings-check.sh <args>` with this
    /// stub answering `cargo semver-checks` with `out` and `status`.
    fn run_semver_check(
        &self,
        args: &[&str],
        out: &str,
        status: i32,
    ) -> (Option<i32>, String, String) {
        let path = format!("{}:{}", self.dir.display(), std::env::var("PATH").unwrap_or_default());
        let output = Command::new("bash")
            .arg(repo_root().join("tools/scripts/semver-major-findings-check.sh"))
            .args(args)
            .env("PATH", path)
            .env("PROBE_CARGO_OUT", out)
            .env("PROBE_CARGO_STATUS", status.to_string())
            .current_dir(repo_root())
            .output()
            .expect("run tools/scripts/semver-major-findings-check.sh");
        (
            output.status.code(),
            String::from_utf8_lossy(&output.stdout).into_owned(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
        )
    }
}

#[cfg(unix)]
impl Drop for CargoStub {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

/// The verdict compares the summary's major count as a whole number: an
/// expected count of 1 is not satisfied by a summary reporting 11 (or 21)
/// major findings, and a minor finding is never accepted. The matching
/// summary passes, echoes the summary line, names each requested lint it
/// finds and refuses one it does not.
#[cfg(unix)]
#[test]
fn semver_check_matches_the_major_count_as_a_whole_number() {
    let stub = CargoStub::new("semver-count");
    let summary = |major: &str, minor: &str| {
        format!(
            "--- failure enum_variant_added: enum variant added on exhaustive enum ---\n\
             Summary semver requires new major version: {major} major and {minor} minor checks failed\n"
        )
    };

    for larger in ["11", "21", "10"] {
        let (code, stdout, stderr) =
            stub.run_semver_check(&["v0.0.0", "1"], &summary(larger, "0"), 1);
        assert_eq!(
            code,
            Some(1),
            "{larger} major findings must not satisfy an expected count of 1: stdout {stdout:?} stderr {stderr:?}"
        );
        assert!(
            stderr.contains("semver findings check failed: expected 1 major and 0 minor findings"),
            "the refusal names the expected counts, got {stderr:?}"
        );
        assert!(!stdout.contains("semver ok"), "no pass line on a count mismatch, got {stdout:?}");
    }

    let (code, stdout, stderr) = stub.run_semver_check(&["v0.0.0", "1"], &summary("1", "1"), 1);
    assert_eq!(
        code,
        Some(1),
        "a minor finding is never accepted: stdout {stdout:?} stderr {stderr:?}"
    );
    assert!(stderr.contains("expected 1 major and 0 minor findings"), "{stderr:?}");

    let (code, stdout, stderr) =
        stub.run_semver_check(&["v0.0.0", "1", "enum_variant_added"], &summary("1", "0"), 1);
    assert_eq!(code, Some(0), "the matching summary passes: stderr {stderr:?}");
    assert!(
        stdout.contains("1 major and 0 minor checks failed"),
        "the summary line is echoed for the release notes, got {stdout:?}"
    );
    assert!(stdout.contains("found enum_variant_added"), "{stdout:?}");
    assert!(
        stdout.trim_end().ends_with("semver ok: 1 major and 0 minor findings against v0.0.0"),
        "the pass line names the counts and the baseline, got {stdout:?}"
    );

    let (code, stdout, stderr) =
        stub.run_semver_check(&["v0.0.0", "1", "function_missing"], &summary("1", "0"), 1);
    assert_eq!(code, Some(1), "a requested lint absent from the findings fails: stdout {stdout:?}");
    assert!(
        stderr.contains(
            "semver findings check failed: expected lint function_missing in the findings"
        ),
        "{stderr:?}"
    );
}

/// With an expected count of 0 the verdict is `cargo semver-checks`' own
/// exit status: a clean run passes with a line that says so, a run that
/// found anything fails naming the status, whatever the summary says.
#[cfg(unix)]
#[test]
fn semver_check_with_zero_expected_findings_follows_the_cargo_exit_status() {
    let stub = CargoStub::new("semver-zero");
    let (code, stdout, stderr) = stub.run_semver_check(&["v0.0.0", "0"], "Checked 10 items\n", 0);
    assert_eq!(code, Some(0), "a clean run passes: stderr {stderr:?}");
    assert!(stdout.contains("semver ok: no findings against v0.0.0"), "{stdout:?}");

    let (code, stdout, stderr) = stub.run_semver_check(
        &["v0.0.0", "0"],
        "Summary semver requires new major version: 1 major and 0 minor checks failed\n",
        1,
    );
    assert_eq!(code, Some(1), "findings fail the zero-count form: stdout {stdout:?}");
    assert!(
        stderr.contains(
            "semver findings check failed: expected no findings, cargo semver-checks exited 1"
        ),
        "{stderr:?}"
    );
    assert!(!stdout.contains("semver ok"), "{stdout:?}");
}

/// Every commit the release-tooling probes create goes through the probe
/// clone, whose constructor sets the clone-local identity; a `commit-tree`
/// (or `commit`) issued any other way would pass on a host that
/// auto-detects an identity from its hostname and fail on a CI runner that
/// has none, where `probe_clone_commits_without_a_global_git_identity`
/// cannot see it.
#[test]
fn probe_commits_are_all_created_through_the_identity_bearing_clone() {
    let parts = repo_root().join("crates/apps/reticulumd/tests/fork_publish_contract_parts");
    let mut commit_sites = Vec::new();
    for entry in fs::read_dir(&parts).expect("read the fork_publish_contract_parts directory") {
        let path = entry.expect("read a directory entry").path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }
        let source = fs::read_to_string(&path).expect("read a probe source file");
        for (index, line) in source.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") {
                continue;
            }
            if trimmed.contains("\"commit-tree\"") || trimmed.contains("\"commit\"") {
                let through_clone =
                    trimmed.contains("clone.git(&[") || trimmed.contains("clone.git_with_env(&[");
                assert!(
                    through_clone,
                    "{}:{}: a probe commit must be created through `clone.git`/`clone.git_with_env` so it carries the clone-local identity:\n{line}",
                    path.display(),
                    index + 1
                );
                commit_sites.push(format!("{}:{}", path.display(), index + 1));
            }
        }
    }
    assert!(
        commit_sites.len() >= 3,
        "the probes create commits in at least three places (two ledger tag probes and the identity pin), found {commit_sites:?}"
    );

    let probe2 = read_repo_file(
        "crates/apps/reticulumd/tests/fork_publish_contract_parts/release_tooling_probe2.rs",
    );
    // `ProbeClone::new` is the first `-> Self` constructor in the file and
    // ends by returning the clone; the walk stops at that `clone\n    }`.
    let constructor = probe2
        .split(") -> Self {")
        .nth(1)
        .and_then(|rest| rest.split("\n        clone\n    }\n").next())
        .expect("ProbeClone::new is defined in release_tooling_probe2.rs");
    assert!(
        constructor.contains("\"config\", \"user.name\"")
            && constructor.contains("\"config\", \"user.email\", PROBE_CLONE_EMAIL"),
        "ProbeClone::new must set the clone-local user.name and user.email, got:\n{constructor}"
    );
}
