// Usage pins for the fork release tooling: the committed scripts behave as
// the runbook and README describe when a releaser runs them, the tracked-files
// scan keeps the needles and filters of `old_package_name_survives_only_as_prose`,
// and the docs that carry the ledger convention say what the ledger tests enforce.

/// Runs a `tools/scripts/*.sh` script from the repo root and returns
/// `(exit code, stdout, stderr)`.
#[cfg(unix)]
fn run_fork_script(script: &str, args: &[&str]) -> (Option<i32>, String, String) {
    let output = Command::new("bash")
        .arg(repo_root().join("tools/scripts").join(script))
        .args(args)
        .current_dir(repo_root())
        .output()
        .unwrap_or_else(|err| panic!("run tools/scripts/{script}: {err}"));
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// Whether a `coyote-rns-transport-v*` tag is reachable from HEAD, the
/// script's own precondition for its zero-argument form; a shallow checkout
/// or a branch cut before the first fork tag has none.
#[cfg(unix)]
fn fork_release_tag_reachable() -> bool {
    Command::new("git")
        .args(["tag", "--list", "coyote-rns-transport-v*", "--merged", "HEAD"])
        .current_dir(repo_root())
        .output()
        .map(|output| output.status.success() && !output.stdout.is_empty())
        .unwrap_or(false)
}

/// The runbook hands a releaser `patchset-ledger-check.sh`: with no arguments
/// (the CI form) it passes at a tree whose ledger is complete, a wrong arity
/// is a usage error (exit 2), and a label with no ledger section is a check
/// failure (exit 1) that names the missing heading. Without a reachable fork
/// tag the zero-argument form fails naming that precondition.
#[cfg(unix)]
#[test]
fn patchset_ledger_check_script_passes_at_head_and_reports_misuse() {
    if fork_release_tag_reachable() {
        let (code, stdout, stderr) = run_fork_script("patchset-ledger-check.sh", &[]);
        assert_eq!(code, Some(0), "zero-argument ledger check should pass at HEAD: {stderr}");
        assert!(
            stdout.starts_with("patchset ledger ok:"),
            "the ledger check should report success on stdout, got {stdout:?}"
        );
    } else {
        let (code, _, stderr) = run_fork_script("patchset-ledger-check.sh", &[]);
        assert_eq!(code, Some(1), "without a reachable fork tag the check fails, got {stderr:?}");
        assert!(
            stderr.contains("no coyote-rns-transport-v* tag is reachable from HEAD"),
            "the failure should name the missing fork tag, got {stderr:?}"
        );
    }

    let (code, _, stderr) = run_fork_script("patchset-ledger-check.sh", &["only-one-arg"]);
    assert_eq!(code, Some(2), "one argument is a usage error, got stderr {stderr:?}");
    assert!(stderr.contains("usage:"), "usage error should print the usage line, got {stderr:?}");

    // HEAD..HEAD changes nothing, so the only thing left to fail on is the
    // missing section for the label.
    let (code, _, stderr) = run_fork_script("patchset-ledger-check.sh", &["HEAD", "9.9.9"]);
    assert_eq!(
        code,
        Some(1),
        "a label without a ledger section is a check failure, got {stderr:?}"
    );
    assert!(
        stderr.contains("no '## 9.9.9 (upstream ...)' section"),
        "the failure should name the missing heading, got {stderr:?}"
    );
}

/// The scripts a releaser may call with the wrong shape refuse with a usage
/// line instead of running a two-minute clippy, a semver baseline build
/// against nothing or a scan with stray arguments.
#[cfg(unix)]
#[test]
fn release_scripts_reject_a_wrong_arity_with_usage() {
    let (code, _, stderr) = run_fork_script("workspace-clippy-classify.sh", &["unexpected"]);
    assert_eq!(code, Some(2), "clippy classify takes no arguments, got stderr {stderr:?}");
    assert!(stderr.contains("usage:"), "{stderr:?}");

    let (code, _, stderr) = run_fork_script("semver-major-findings-check.sh", &["baseline-only"]);
    assert_eq!(code, Some(2), "semver check needs a baseline and a count, got stderr {stderr:?}");
    assert!(stderr.contains("<baseline-rev> <expected-major-count>"), "{stderr:?}");

    let (code, _, stderr) = run_fork_script("old-name-tracked-scan.sh", &["unexpected"]);
    assert_eq!(code, Some(2), "old-name scan takes no arguments, got stderr {stderr:?}");
    assert!(stderr.contains("usage:"), "{stderr:?}");
}

/// The README calls `old-name-tracked-scan.sh` the tracked-files-only form of
/// `old_package_name_survives_only_as_prose` with the same needles, extension
/// and doc-prefix filters: the needle literals, scanned extensions, skipped
/// doc prefixes and skipped directories in the script are read back and
/// compared against this file's constants. The script itself is not run here;
/// `old_package_name_survives_only_as_prose` already scans the same set.
#[test]
fn old_name_tracked_scan_script_is_the_twin_of_the_prose_test() {
    let script = read_repo_file("tools/scripts/old-name-tracked-scan.sh");

    let old = script
        .lines()
        .find_map(|line| line.strip_prefix("old='").and_then(|rest| rest.strip_suffix('\'')))
        .expect("the scan script assembles its needles from an `old='...'` line");
    let needles_at = script.find("needles=(\n").expect("the scan script has a needles array");
    let needles_block = &script[needles_at + "needles=(\n".len()..];
    let needles_block = &needles_block[..needles_block.find("\n)").expect("needles array closes")];
    let mut needles: Vec<String> = needles_block
        .lines()
        .map(|line| {
            let line = line.trim();
            let line = line.strip_prefix('"').and_then(|l| l.strip_suffix('"')).unwrap_or(line);
            line.replace("${old}", old).replace("\\\"", "\"")
        })
        .collect();
    needles.sort();
    let mut expected: Vec<String> = OLD_PACKAGE_NAME_FORMS.iter().map(|s| s.to_string()).collect();
    expected.sort();
    assert_eq!(needles, expected, "the scan script's needles should equal OLD_PACKAGE_NAME_FORMS");

    for ext in SCANNED_EXTENSIONS {
        assert!(
            script.contains(&format!("*.{ext}")),
            "the scan script should scan *.{ext} like SCANNED_EXTENSIONS"
        );
    }
    for prefix in SKIPPED_DOC_PREFIXES {
        assert!(
            script.contains(&format!("{prefix}*")),
            "the scan script should skip {prefix}* like SKIPPED_DOC_PREFIXES"
        );
    }
    for dir in SKIPPED_DIRECTORIES.iter().filter(|dir| **dir != ".git") {
        assert!(
            script.contains(&format!("{dir}/*")),
            "the scan script should skip {dir}/ like SKIPPED_DIRECTORIES"
        );
    }
}

/// `cargo test -p coyote-rns-transport --tests` only builds every
/// `crates/libs/rns-transport/tests/*.rs` target when none is opted out of
/// the default test set: no `[[test]]` table may carry `test = false` or
/// `required-features`, and every `tests/*.rs` must be auto-discoverable.
#[test]
fn every_rns_transport_external_test_target_builds_under_the_ci_command() {
    let manifest = read_repo_file("crates/libs/rns-transport/Cargo.toml");
    let test_tables: Vec<&str> = manifest.split("[[test]]").skip(1).collect();
    for table in &test_tables {
        let table = table.split("\n[").next().unwrap_or(table);
        assert!(
            !table.contains("test = false") && !table.contains("required-features"),
            "a [[test]] target opted out of `cargo test --tests` would not build in PR CI:\n{table}"
        );
    }
    assert!(
        !manifest.contains("autotests = false"),
        "autotests = false would hide tests/*.rs from `cargo test --tests`"
    );

    let tests_dir = repo_root().join("crates/libs/rns-transport/tests");
    let targets: Vec<String> = fs::read_dir(&tests_dir)
        .expect("read crates/libs/rns-transport/tests")
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "rs"))
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    assert!(
        targets.len() >= 10,
        "the README's compile gate is the external tests; expected at least ten, found {targets:?}"
    );
}

/// The AGENTS.md "Parity Workflow" carves fork-only parity rows out of the
/// `docs/status/*` matrices and points at the two ledgers that do carry
/// them, both of which must exist where the sentence says.
#[test]
fn agents_parity_workflow_carves_out_fork_only_rows_to_the_ledgers_that_exist() {
    let agents = read_repo_file("AGENTS.md");
    let start = agents.find("## Parity Workflow").expect("AGENTS.md has a Parity Workflow section");
    let section = &agents[start..];
    let section = section[2..].find("\n## ").map_or(section, |end| &section[..end + 2]);
    assert!(
        section.contains("docs/fork/PATCHSET.md"),
        "the Parity Workflow should name docs/fork/PATCHSET.md as the fork-only ledger"
    );
    assert!(
        section.contains("Known divergences from Python RNS")
            && section.contains("crates/libs/rns-transport/README.md"),
        "the Parity Workflow should name the crate README's Known divergences section"
    );
    assert!(
        section.contains("docs/status/*") && section.contains("not updated per fork task"),
        "the carve-out should say the docs/status matrices are not updated per fork task"
    );

    let readme = read_repo_file("crates/libs/rns-transport/README.md");
    assert!(
        readme.contains("\n## Known divergences from Python RNS"),
        "the crate README should have the `## Known divergences from Python RNS` heading AGENTS.md points at"
    );
    assert!(repo_root().join("docs/fork/PATCHSET.md").is_file());
}

/// The parity matrix no longer duplicates fork commit hashes the PATCHSET
/// preamble says live in the PATCHSET: the segment-dispatch row names the
/// patch id and the ledger instead, and that patch id is defined there.
#[test]
fn parity_matrix_segment_dispatch_row_points_at_the_patchset_not_at_hashes() {
    let matrix = read_repo_file("docs/status/reticulum-parity-matrix.md");
    let row = matrix
        .lines()
        .find(|line| line.contains("bound-interface segment dispatch"))
        .expect("the parity matrix should keep its segment-dispatch sentence");
    assert!(
        row.contains("docs/fork/PATCHSET.md"),
        "the segment-dispatch row should point at docs/fork/PATCHSET.md"
    );
    assert!(
        row.contains("0.14.1 section"),
        "patch ids are per section, so the row should name the section it points into"
    );
    let patch_id = row
        .split(|c: char| !c.is_ascii_alphanumeric())
        .find(|token| is_patch_id(token))
        .expect("the segment-dispatch row should name the patch id (B1)");
    let hashes: Vec<&str> = row.split(' ').filter_map(commit_hash).collect();
    assert!(
        hashes.is_empty(),
        "the parity matrix should not duplicate fork commit hashes, found {hashes:?}"
    );

    let ledger = read_repo_file("docs/fork/PATCHSET.md");
    let section = ledger_section(&ledger, "0.14.1")
        .expect("docs/fork/PATCHSET.md should keep its 0.14.1 section");
    assert!(
        ledger_legend("0.14.1", section).contains(&format!("{patch_id} is ")),
        "the 0.14.1 legend should define patch id {patch_id} the matrix points at"
    );
}

/// The CHANGELOG preamble rule names the no-library-behaviour exception
/// generally (a test-only change, release tooling, CI), not only one kind of
/// it (a fixture hardening), since the Unreleased hygiene and tooling bullets
/// rely on it.
#[test]
fn changelog_preamble_allows_any_test_only_change_as_its_own_bullet() {
    let changelog = read_repo_file("crates/libs/rns-transport/CHANGELOG.md");
    let preamble = &changelog[..changelog.find("\n## ").expect("CHANGELOG has a section")];
    let rule = preamble.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        rule.contains("A change that alters no library behaviour (a test-only change"),
        "the CHANGELOG preamble should state the no-library-behaviour exception generally, got {rule:?}"
    );
    assert!(
        rule.contains("bullet of its own, marked as such"),
        "the exception bullet must be its own marked bullet, got {rule:?}"
    );
}
