// Usage pins for three release-script refusals: a releaser who names a
// previous version the ledger has no heading for, a scan over a checkout
// with nothing to scan, and a semver count that is not a number. Each case
// is reached through the script's command line only; the assertions are on
// the exact words a releaser reads.

/// The label of the first `## <label> (upstream <base>)` heading in the
/// working-tree ledger, whatever the current release state calls it.
#[cfg(unix)]
fn newest_ledger_label() -> String {
    let ledger = read_repo_file("docs/fork/PATCHSET.md");
    ledger
        .lines()
        .find_map(|line| {
            let rest = line.strip_prefix("## ")?;
            let (label, _) = rest.split_once(" (upstream ")?;
            Some(label.to_string())
        })
        .expect("the ledger has at least one `## <label> (upstream <base>)` heading")
}

/// Naming a previous version without a ledger heading is a check failure
/// that names the missing heading and the section it was meant to end, not
/// a silent span to the end of the file (which would make the section's
/// table disagree with the range and fail with a misleading table message).
/// `HEAD..HEAD` changes nothing, so the heading check is the only thing left
/// to fail on, and the order is fixed: the section's own heading is checked
/// first, then the previous one.
#[cfg(unix)]
#[test]
fn ledger_check_names_a_missing_previous_heading_instead_of_spanning_to_the_end() {
    let label = newest_ledger_label();
    let (code, stdout, stderr) =
        run_fork_script("patchset-ledger-check.sh", &["HEAD", &label, "9.9.9"]);
    assert_eq!(
        code,
        Some(1),
        "a previous version without a heading is a check failure: {stderr:?}"
    );
    assert!(
        stderr.contains("no '## 9.9.9 (upstream ...)' heading")
            && stderr.contains(&format!("to end the {label} section for HEAD..HEAD")),
        "the failure should name the missing previous heading and the section it ends, got {stderr:?}"
    );
    assert!(
        !stderr.contains("table") && !stdout.contains("patchset ledger ok"),
        "the heading check must refuse before any table comparison, got stdout {stdout:?} stderr {stderr:?}"
    );

    // The same range with a previous version the ledger does carry gets past
    // both heading checks and fails later, on the table (the section lists
    // files while HEAD..HEAD changes none), so the refusal above is the
    // previous-heading check and not the label's own or the table's.
    let ledger = read_repo_file("docs/fork/PATCHSET.md");
    let previous = ledger
        .lines()
        .filter_map(|line| line.strip_prefix("## ")?.split_once(" (upstream ").map(|(l, _)| l))
        .find(|candidate| *candidate != label)
        .expect("the ledger carries a second release heading to end the newest section");
    let (code, stdout, stderr) =
        run_fork_script("patchset-ledger-check.sh", &["HEAD", &label, previous]);
    assert_eq!(code, Some(1), "HEAD..HEAD with a populated section fails on the table: {stderr:?}");
    assert!(
        stderr.contains("table") && !stderr.contains("heading"),
        "with a real previous heading the failure is the table comparison, got stdout {stdout:?} stderr {stderr:?}"
    );
}

/// A checkout whose index holds no scanned file passes the old-name scan
/// with a `skipped:` message, so `old-name scan ok:` stays the mark of a
/// pass over files that were actually scanned. Without the guard the script
/// either dies on an unbound `files[@]` (bash before 4.4) or hands grep no
/// file operand so it reads stdin and reports the scanned-files pass message
/// as if files had been scanned; the exact wording is the contract.
#[cfg(unix)]
#[test]
fn old_name_scan_over_an_empty_index_says_there_was_nothing_to_scan() {
    let clone = ProbeClone::new("old-name-empty-index");
    let (code, stdout, stderr) = clone.run_script("old-name-tracked-scan.sh", &[]);
    assert_eq!(code, Some(0), "the clean clone passes first: {stderr}");
    assert_eq!(
        stdout.trim(),
        "old-name scan ok: no tracked scanned file carries an old-package-name form",
        "with files to scan the pass message says they were scanned"
    );

    // Empty the index only; the working tree keeps the script and the files.
    clone.git(&["rm", "-r", "-q", "--cached", "."]);
    assert_eq!(clone.git(&["ls-files"]), "", "the clone's index is empty");

    let (code, stdout, stderr) = clone.run_script("old-name-tracked-scan.sh", &[]);
    assert_eq!(code, Some(0), "an empty index is a pass, not an error: {stderr:?}");
    assert_eq!(
        stdout.trim(),
        "old-name scan skipped: no tracked scanned file",
        "the message must say the scan was skipped, not that scanned files were clean"
    );
    assert!(stderr.trim().is_empty(), "no diagnostic on stderr for the empty case, got {stderr:?}");
}

/// A non-numeric expected-major-count is a usage error refused before any
/// `cargo semver-checks` run: the usage line, exit 2, nothing on stdout. A
/// count that only fails later as a bare `[ -eq ]` comparison would first
/// spend the baseline build and then die with bash's "integer expression
/// expected" rather than the usage line.
#[cfg(unix)]
#[test]
fn semver_check_refuses_a_non_numeric_count_with_usage_before_running_cargo() {
    for count in ["abc", "-1", "", "1.5", "0x10"] {
        let started = std::time::Instant::now();
        let (code, stdout, stderr) =
            run_fork_script("semver-major-findings-check.sh", &["HEAD", count]);
        assert_eq!(code, Some(2), "count {count:?} is a usage error, got stderr {stderr:?}");
        assert!(
            stderr.contains("usage:") && stderr.contains("<baseline-rev> <expected-major-count>"),
            "count {count:?} should print the usage line, got {stderr:?}"
        );
        assert!(
            stderr.contains(&format!("must be an integer, got '{count}'")),
            "count {count:?} should be named as the non-integer, got {stderr:?}"
        );
        assert!(
            !stderr.contains("integer expression") && !stderr.contains("semver findings check failed"),
            "count {count:?} must be refused by the usage check, not by a later comparison: {stderr:?}"
        );
        assert!(stdout.is_empty(), "no summary line for a refused count {count:?}, got {stdout:?}");
        assert!(
            started.elapsed() < std::time::Duration::from_secs(10),
            "the refusal for {count:?} must not wait on a cargo semver-checks run"
        );
    }
}
