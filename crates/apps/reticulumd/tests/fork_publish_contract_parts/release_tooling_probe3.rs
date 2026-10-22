// Usage pin for the agreement between the ledger script's zero-argument
// precondition and the clone probes' skip predicate: both must read "a
// coyote-rns-transport-v* tag that is an ancestor of HEAD". A checkout whose
// only fork tag sits on a commit HEAD does not contain is the case that
// separates the two readings (`git tag --list` alone would say "tag
// present"), and the script's failure names that case in so many words.

/// With the only fork tag on an unmerged commit, the zero-argument check
/// fails with the unmerged-branch clause of its message, and the probes'
/// skip predicate reports no reachable tag, so a probe that skips on it
/// skips exactly when the script itself would refuse to run.
#[cfg(unix)]
#[test]
fn an_unmerged_only_fork_tag_fails_the_zero_argument_check_and_trips_the_skip_predicate() {
    let clone = ProbeClone::new("ledger-unmerged-only");
    if !clone.has_a_reachable_fork_tag() {
        eprintln!("skipping: no coyote-rns-transport-v* tag reachable from HEAD in this checkout");
        return;
    }
    let (code, _, stderr) = clone.run_script("patchset-ledger-check.sh", &[]);
    assert_eq!(code, Some(0), "clean clone should pass first: {stderr}");

    // Move every fork tag onto a commit that HEAD does not contain. The
    // tags still exist, so a presence-only predicate would call them usable.
    let dangling = clone.git(&["commit-tree", "-m", "probe", "HEAD^{tree}"]);
    let tags = clone.fork_tags();
    assert!(!tags.is_empty(), "the clone carried at least one fork tag");
    for tag in &tags {
        clone.git(&["tag", "-f", tag, &dangling]);
    }
    assert_eq!(clone.fork_tags(), tags, "the tags are still present");
    assert!(
        !clone.has_a_reachable_fork_tag(),
        "a fork tag on an unmerged commit must not count as reachable"
    );

    let (code, stdout, stderr) = clone.run_script("patchset-ledger-check.sh", &[]);
    assert_eq!(
        code,
        Some(1),
        "with only an unmerged fork tag the zero-argument check must fail; stdout {stdout:?}"
    );
    assert!(
        stderr.contains("no coyote-rns-transport-v* tag is reachable from HEAD")
            && stderr.contains("a release tag on an unmerged branch does not count"),
        "the failure should name the unmerged-branch case, got {stderr:?}"
    );
    assert!(
        !stdout.contains("patchset ledger ok"),
        "the script must not report success before refusing, got {stdout:?}"
    );
}
