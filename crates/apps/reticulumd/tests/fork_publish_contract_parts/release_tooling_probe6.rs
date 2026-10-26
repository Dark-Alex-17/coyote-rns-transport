// Usage pins for the semver script's target dir. cargo-semver-checks writes
// rustdoc JSON for the current and the baseline crate under the active
// target dir, so with the workspace default both land in the same
// `target/doc` the rustdoc gate uses; a fresh `cargo doc` there lets one
// side parse the other side's JSON and the verdict becomes a false "no
// semver update required". The script therefore runs cargo under its own
// `semver-findings-check` dir. These pins put a recording `cargo` stub
// first on PATH and read back the `CARGO_TARGET_DIR` and the arguments the
// script handed it, with and without an operator-set `CARGO_TARGET_DIR`.

/// A directory holding an executable `cargo` that prints `$PROBE_CARGO_OUT`,
/// appends the `CARGO_TARGET_DIR` and arguments it was called with to
/// `$PROBE_CARGO_RECORD`, and exits with `$PROBE_CARGO_STATUS`; removed on
/// drop.
#[cfg(unix)]
struct RecordingCargoStub {
    dir: PathBuf,
}

#[cfg(unix)]
impl RecordingCargoStub {
    fn new(name: &str) -> Self {
        use std::os::unix::fs::PermissionsExt;
        let dir =
            std::env::temp_dir().join(format!("rns-fork-probe-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create the cargo stub directory");
        let stub = dir.join("cargo");
        fs::write(
            &stub,
            "#!/usr/bin/env bash\n\
             printf 'CARGO_TARGET_DIR=%s\\n' \"${CARGO_TARGET_DIR:-}\" >> \"${PROBE_CARGO_RECORD}\"\n\
             printf 'ARGS=%s\\n' \"$*\" >> \"${PROBE_CARGO_RECORD}\"\n\
             printf '%s\\n' \"${PROBE_CARGO_OUT}\"\n\
             exit \"${PROBE_CARGO_STATUS:-0}\"\n",
        )
        .expect("write the cargo stub");
        fs::set_permissions(&stub, fs::Permissions::from_mode(0o755))
            .expect("make the stub executable");
        Self { dir }
    }

    fn record_path(&self) -> PathBuf {
        self.dir.join("record")
    }

    /// Runs `tools/scripts/semver-major-findings-check.sh <args>` with this
    /// stub answering `cargo semver-checks`; `target_dir` is the operator's
    /// `CARGO_TARGET_DIR` for the child, or none. Returns the exit code and
    /// the stub's record.
    fn run_semver_check(&self, args: &[&str], target_dir: Option<&Path>) -> (Option<i32>, String) {
        let path = format!("{}:{}", self.dir.display(), std::env::var("PATH").unwrap_or_default());
        let mut command = Command::new("bash");
        command
            .arg(repo_root().join("tools/scripts/semver-major-findings-check.sh"))
            .args(args)
            .env("PATH", path)
            .env("PROBE_CARGO_OUT", "Summary no semver update required\n")
            .env("PROBE_CARGO_STATUS", "0")
            .env("PROBE_CARGO_RECORD", self.record_path())
            .current_dir(repo_root());
        match target_dir {
            Some(dir) => command.env("CARGO_TARGET_DIR", dir),
            None => command.env_remove("CARGO_TARGET_DIR"),
        };
        let output = command.output().expect("run tools/scripts/semver-major-findings-check.sh");
        let record = fs::read_to_string(self.record_path()).unwrap_or_default();
        (output.status.code(), record)
    }
}

#[cfg(unix)]
impl Drop for RecordingCargoStub {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

#[cfg(unix)]
const SEMVER_CHECK_ARGS: &str =
    "semver-checks check-release -p coyote-rns-transport --baseline-rev v0.0.0 --release-type patch";

/// Without an operator `CARGO_TARGET_DIR` the script isolates cargo under
/// `target/semver-findings-check` in the repository root, and the
/// check-release arguments are unchanged.
#[cfg(unix)]
#[test]
fn semver_check_runs_cargo_under_its_own_target_dir() {
    let stub = RecordingCargoStub::new("semver-target-dir");
    let (code, record) = stub.run_semver_check(&["v0.0.0", "0"], None);
    assert_eq!(code, Some(0), "a clean run passes: record {record:?}");
    let expected = repo_root().join("target/semver-findings-check");
    assert!(
        record.contains(&format!("CARGO_TARGET_DIR={}\n", expected.display())),
        "cargo must run under {}, got record {record:?}",
        expected.display()
    );
    assert!(
        record.contains(&format!("ARGS={SEMVER_CHECK_ARGS}\n")),
        "the check-release arguments are unchanged, got record {record:?}"
    );
}

/// An operator-set `CARGO_TARGET_DIR` is honoured as the parent of the
/// isolated dir rather than replaced by the workspace default.
#[cfg(unix)]
#[test]
fn semver_check_nests_its_target_dir_under_an_operator_cargo_target_dir() {
    let stub = RecordingCargoStub::new("semver-operator-target-dir");
    let operator_dir =
        PathBuf::from(format!("/tmp/rns-fork-probe-operator-target-{}", std::process::id()));
    let (code, record) = stub.run_semver_check(&["v0.0.0", "0"], Some(&operator_dir));
    assert_eq!(code, Some(0), "a clean run passes: record {record:?}");
    let expected = operator_dir.join("semver-findings-check");
    assert!(
        record.contains(&format!("CARGO_TARGET_DIR={}\n", expected.display())),
        "cargo must run under {}, got record {record:?}",
        expected.display()
    );
    assert!(
        record.contains(&format!("ARGS={SEMVER_CHECK_ARGS}\n")),
        "the check-release arguments are unchanged, got record {record:?}"
    );
}
