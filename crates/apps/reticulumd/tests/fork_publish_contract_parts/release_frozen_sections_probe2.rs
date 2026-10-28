// Synthetic fixtures for the two slicers the frozen-sections pins rest on.
// The pins in `release_frozen_sections_probe.rs` compare the ledgers at the
// newest reachable fork tag with the working tree, so on a checkout where
// both agree (every checkout the pins are meant to pass on) the slicers are
// only ever exercised on well-formed input and a slicer that swallowed a
// heading or ran past a block boundary would still be green. These fixtures
// state what each slicer must do at the boundaries the rule names: an
// `## Unreleased` heading is not a release, a non-release heading below the
// releases closes the last one, and a README block ends at the next
// `What \`` marker or `## ` heading, whichever comes first. The last pins tie
// the newest release section of each ledger to the crate manifest version,
// the promotion rule the version-site pins state by literal, and admit an
// `## Unreleased` stub above it only between releases.

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

/// Whether a ledger may carry an `## Unreleased` stub, and where. The stub
/// sits above the `## <manifest version>` section, never below it. Between
/// releases the manifest version equals the newest fork tag reachable from
/// HEAD and the stub collects the work since that tag; in a release tree the
/// manifest version is newer than that tag and the stub must already be
/// promoted. Returns `None` when the ledger is acceptable and the reason
/// otherwise. Without a reachable tag the ledger is accepted, as the
/// frozen-sections pins skip on such a checkout.
#[cfg(unix)]
fn unreleased_stub_violation(
    ledger: &str,
    manifest_version: &str,
    newest_tag_version: Option<&str>,
) -> Option<String> {
    let newest_tag_version = newest_tag_version?;
    let stub = ledger.find("\n## Unreleased")?;
    let release = ledger.find(&format!("\n## {manifest_version}"));
    if release.is_some_and(|release| stub > release) {
        return Some(format!(
            "the `## Unreleased` stub should sit above the `## {manifest_version}` section, \
             not below it"
        ));
    }
    if manifest_version == newest_tag_version {
        return None;
    }
    Some(format!(
        "an `## Unreleased` stub may sit above the `## {manifest_version}` section only while \
         {manifest_version} is the newest reachable fork tag, which is {newest_tag_version}"
    ))
}

/// The newest release section of each ledger is the crate manifest version:
/// a version bump and the ledger promotions land together. An `## Unreleased`
/// stub may sit above the release only between releases, while the manifest
/// version is still the newest reachable fork tag; a release tree carries
/// none. Derived from `Cargo.toml` and the reachable tags rather than a
/// literal, so it also holds for the next promotion.
#[cfg(unix)]
#[test]
fn newest_ledger_sections_are_the_crate_manifest_version() {
    let manifest = read_repo_file("crates/libs/rns-transport/Cargo.toml");
    let version = manifest
        .lines()
        .find_map(|line| line.strip_prefix("version = \""))
        .and_then(|rest| rest.strip_suffix('"'))
        .expect("the crate manifest states its version on one line");
    let newest_tag = newest_reachable_fork_tag();
    if newest_tag.is_none() {
        eprintln!(
            "skipping the stub rule: no coyote-rns-transport-v* tag reachable from HEAD in this checkout"
        );
    }
    for relative in ["crates/libs/rns-transport/CHANGELOG.md", "docs/fork/PATCHSET.md"] {
        let text = read_repo_file(relative);
        let sections = release_sections(&text);
        assert_eq!(
            sections.first().map(|(label, _)| *label),
            Some(version),
            "{relative} should open with the `## {version}` release section"
        );
        let violation =
            unreleased_stub_violation(&text, version, newest_tag.as_ref().map(|(_, v)| v.as_str()));
        assert_eq!(violation, None, "{relative}");
    }
}

/// Between releases, with the manifest at the newest reachable tag, a stub
/// above the release is acceptable; once the manifest moves past that tag the
/// same stub names a release tree that still carries unpromoted work. A stub
/// below the release is a violation even between releases.
#[cfg(unix)]
#[test]
fn unreleased_stub_sits_above_the_release_and_is_admitted_only_at_the_newest_tag() {
    let ledger = "# Changelog\n\nintro\n\n## Unreleased\n\n### Added\n\n- pending\n\n\
                  ## 0.16.0 - 2026-10-25\n\n### Breaking\n\n- x\n";
    assert_eq!(unreleased_stub_violation(ledger, "0.16.0", Some("0.16.0")), None);
    let reason = unreleased_stub_violation(ledger, "0.17.0", Some("0.16.0"))
        .expect("a stub in a release tree is a violation");
    assert!(reason.contains("0.17.0") && reason.contains("0.16.0"), "{reason}");
    let below = "# Changelog\n\n## 0.16.0 - 2026-10-25\n\n### Breaking\n\n- x\n\n\
                 ## Unreleased\n\n- pending\n";
    let reason = unreleased_stub_violation(below, "0.16.0", Some("0.16.0"))
        .expect("a stub below the release is a violation");
    assert!(reason.contains("not below it"), "{reason}");
}

/// A ledger without a stub is acceptable whether or not the manifest has
/// moved past the newest reachable tag, and so is any ledger on a checkout
/// with no reachable fork tag, where the frozen-sections pins skip.
#[cfg(unix)]
#[test]
fn unreleased_stub_rule_passes_without_a_stub_and_skips_without_a_tag() {
    let released = "# Changelog\n\nintro\n\n## 0.16.0 - 2026-10-25\n\n### Breaking\n\n- x\n";
    assert_eq!(unreleased_stub_violation(released, "0.16.0", Some("0.16.0")), None);
    assert_eq!(unreleased_stub_violation(released, "0.17.0", Some("0.16.0")), None);
    let stubbed = "# Changelog\n\n## Unreleased\n\n- pending\n\n## 0.16.0 - 2026-10-25\n\n- x\n";
    assert_eq!(unreleased_stub_violation(stubbed, "0.17.0", None), None);
}
