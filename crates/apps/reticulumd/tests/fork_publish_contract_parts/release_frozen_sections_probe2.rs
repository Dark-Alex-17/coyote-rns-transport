// Synthetic fixtures for the two slicers the frozen-sections pins rest on.
// The pins in `release_frozen_sections_probe.rs` compare the ledgers at the
// newest reachable fork tag with the working tree, so on a checkout where
// both agree (every checkout the pins are meant to pass on) the slicers are
// only ever exercised on well-formed input and a slicer that swallowed a
// heading or ran past a block boundary would still be green. These fixtures
// state what each slicer must do at the boundaries the rule names: an
// `## Unreleased` heading is not a release, a non-release heading below the
// releases closes the last one, and a README block ends at the next
// `What \`` marker or `## ` heading, whichever comes first. The last pin ties
// the newest section label of each ledger to the crate manifest version, the
// promotion rule the version-site pins state by literal.

/// A ledger with an `## Unreleased` stub above two releases and a
/// non-release heading below them splits into exactly the two releases, each
/// running from its heading to the next `## ` heading, in file order.
#[cfg(unix)]
#[test]
fn release_sections_keeps_only_numbered_headings_and_closes_at_the_next_heading() {
    let ledger = "# Changelog\n\nintro\n\n## Unreleased\n\n- pending\n\n\
                  ## 0.16.0 - 2026-10-25\n\n### Breaking\n\n- x\n\n\
                  ## 0.15.1 (upstream 0.14.0)\n\nbody\n\n\
                  ## Appendix\n\nnotes\n";
    let sections = release_sections(ledger);
    let labels: Vec<&str> = sections.iter().map(|(label, _)| *label).collect();
    assert_eq!(labels, ["0.16.0", "0.15.1"], "numbered headings only, in file order");
    assert_eq!(
        sections[0].1, "## 0.16.0 - 2026-10-25\n\n### Breaking\n\n- x\n\n",
        "a section runs from its heading up to the next `## ` heading"
    );
    assert_eq!(
        sections[1].1, "## 0.15.1 (upstream 0.14.0)\n\nbody\n\n",
        "a non-release heading below the releases closes the last section"
    );
    let joined: String = sections.iter().map(|(_, text)| *text).collect();
    assert!(ledger.contains(&joined), "adjacent release sections are contiguous in the source");
    assert!(
        sections.iter().all(|(_, text)| !text.contains("pending")),
        "the `## Unreleased` body belongs to no release section"
    );
}

/// A ledger whose last release runs to the end of the file yields that
/// section with the trailing text included, and a ledger with no numbered
/// heading yields nothing.
#[cfg(unix)]
#[test]
fn release_sections_runs_the_last_release_to_end_of_file_and_is_empty_without_one() {
    let ledger = "## 0.14.0 (upstream 0.14.0)\n\nfirst\n\nlast line without newline";
    let sections = release_sections(ledger);
    assert_eq!(sections.len(), 1);
    assert_eq!(sections[0].0, "0.14.0");
    assert_eq!(sections[0].1, ledger, "the last release runs to the end of the file");
    assert!(release_sections("# Title\n\n## Unreleased\n\n- x\n").is_empty());
}

/// The README block slicer takes the bullets after the `What \`<v>\` carries
/// on top` marker and stops at the next marker or `## ` heading, whichever is
/// first, dropping the marker sentence and trailing blank lines.
#[cfg(unix)]
#[test]
fn readme_patch_block_bullets_stops_at_the_next_block_or_heading() {
    let readme = "## Patch list\n\nintro. What `0.16.0` carries on top\nof `0.15.1`:\n\n\
                  - a\n  cont\n- b\n\n\
                  What `0.15.1` carries on top of `0.15.0`:\n\n- c\n\n\
                  ## Next\n\n- d\n";
    assert_eq!(readme_patch_block_bullets(readme, "0.16.0"), "- a\n  cont\n- b");
    assert_eq!(readme_patch_block_bullets(readme, "0.15.1"), "- c");
}

/// The newest release section of each ledger is the crate manifest version:
/// a version bump and the ledger promotions land together, and no
/// `## Unreleased` stub sits above the release. Derived from `Cargo.toml`
/// rather than a literal, so it also holds for the next promotion.
#[cfg(unix)]
#[test]
fn newest_ledger_sections_are_the_crate_manifest_version() {
    let manifest = read_repo_file("crates/libs/rns-transport/Cargo.toml");
    let version = manifest
        .lines()
        .find_map(|line| line.strip_prefix("version = \""))
        .and_then(|rest| rest.strip_suffix('"'))
        .expect("the crate manifest states its version on one line");
    for relative in ["crates/libs/rns-transport/CHANGELOG.md", "docs/fork/PATCHSET.md"] {
        let text = read_repo_file(relative);
        let sections = release_sections(&text);
        assert_eq!(
            sections.first().map(|(label, _)| *label),
            Some(version),
            "{relative} should open with the `## {version}` release section"
        );
        assert!(
            !text.contains("\n## Unreleased"),
            "{relative} should carry no `## Unreleased` stub once the release is cut"
        );
    }
}
