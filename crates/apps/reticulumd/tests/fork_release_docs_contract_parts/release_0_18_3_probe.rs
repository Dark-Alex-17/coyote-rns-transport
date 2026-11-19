// Pins over the 0.18.3 release artefacts beyond what the release contracts
// check. Those pin the heading date by shape, the absence of a `### Breaking`
// subsection on a patch bump and the version by its sites; these pins hold
// the heading to the day the release was cut and to the top of the ledger,
// the subsection set a purely additive patch release carries, the three
// items `### Added` names, the patch digit and semver verdict the version
// bullet records, the twelve release-docs parts the restated include-set
// bullet lists, and the README patch-list order with the 0.18.3 block on top.

/// The 0.18.3 heading carries the day the release was cut and is the first
/// release heading of the CHANGELOG.
#[test]
fn release_0_18_3_heading_carries_the_release_date() {
    let changelog = read_repo_file("crates/libs/rns-transport/CHANGELOG.md");
    let heading = "\n## 0.18.3 - 2026-11-19\n";
    assert!(
        changelog.contains(heading),
        "the CHANGELOG should head the 0.18.3 section with its release date"
    );
    let first_release = changelog
        .lines()
        .find(|line| line.starts_with("## ") && line[3..].starts_with(|c: char| c.is_ascii_digit()))
        .expect("the CHANGELOG should carry a release heading");
    assert_eq!(
        first_release,
        heading.trim(),
        "the 0.18.3 heading should be the first release heading of the CHANGELOG"
    );
}

/// A purely additive patch release carries neither `### Breaking` nor
/// `### Fixed`; the 0.18.3 section is `### Added` then `### Changed` and
/// nothing else.
#[test]
fn release_0_18_3_subsections_run_added_changed() {
    let changelog = read_repo_file("crates/libs/rns-transport/CHANGELOG.md");
    let section = changelog_release_section(&changelog, "0.18.3");
    let names: Vec<&str> = subsections(&section).into_iter().map(|(name, _)| name).collect();
    assert_eq!(
        names,
        ["Added", "Changed"],
        "an additive patch release carries `### Added` and `### Changed` only"
    );
}

/// `### Added` of 0.18.3 names the three additions: the peer-count read with
/// its signature, the rpc client module and the announce emission field.
#[test]
fn release_0_18_3_added_names_the_peer_count_the_rpc_client_and_the_emission() {
    let changelog = read_repo_file("crates/libs/rns-transport/CHANGELOG.md");
    let section = changelog_release_section(&changelog, "0.18.3");
    let added: Vec<String> =
        subsection_bullets(&section, "Added").iter().map(|bullet| joined_prose(bullet)).collect();
    assert!(!added.is_empty(), "the 0.18.3 `### Added` subsection should carry bullets");
    for needle in [
        "`Transport::interface_peer_count(&AddressHash) -> Option<u64>`",
        "`transport::shared_instance_rpc`",
        "`AnnounceEvent::emitted`",
    ] {
        assert!(
            added.iter().any(|bullet| bullet.contains(needle)),
            "the 0.18.3 `### Added` subsection should name an addition by {needle:?}"
        );
    }
}

/// The 0.18.3 version bullet, the first under `### Changed`, records why the
/// bump is a patch one and the semver verdict: the baseline tag, that nothing
/// was found, the 0.15.1, 0.18.1 and 0.18.2 precedents, the absent
/// `### Breaking`, the gate the release check runs and the probe that pins
/// the release.
#[test]
fn release_0_18_3_version_bullet_records_the_patch_digit_and_the_semver_verdict() {
    let changelog = read_repo_file("crates/libs/rns-transport/CHANGELOG.md");
    let section = changelog_release_section(&changelog, "0.18.3");
    let bullets = subsection_bullets(&section, "Changed");
    let first = bullets.first().expect("the 0.18.3 `### Changed` subsection should carry bullets");
    assert!(
        first.starts_with("- The crate version is `0.18.3`"),
        "the version bullet heads `### Changed`, got {:?}",
        first
    );
    let bullet =
        joined_prose(&single_bullet_starting_with(&bullets, "- The crate version is `0.18.3`"));
    for needle in [
        "`cargo semver-checks` against `v0.18.2` reports 0 findings",
        "The bump is a patch one under the README policy",
        "as `0.15.1`, `0.18.1` and `0.18.2` were",
        "additive API a consumer need not opt into",
        "no public item removed or changed",
        "no `### Breaking` subsection",
        "`tools/scripts/semver-major-findings-check.sh coyote-rns-transport-v0.18.2 0`",
        "`fork_release_docs_contract_parts/release_0_18_3_probe.rs`",
    ] {
        assert!(bullet.contains(needle), "the 0.18.3 version bullet should say {needle:?}");
    }
}

/// The restated include-set bullet of 0.18.3 names all twelve parts of
/// `fork_release_docs_contract_parts/`, this file among them, and says the
/// ledger label pins read `0.18.3` first with no `Unreleased` label.
#[test]
fn release_0_18_3_include_set_bullet_names_the_twelve_parts() {
    let changelog = read_repo_file("crates/libs/rns-transport/CHANGELOG.md");
    let section = changelog_release_section(&changelog, "0.18.3");
    let bullets = subsection_bullets(&section, "Changed");
    let bullet = joined_prose(&single_bullet_starting_with(
        &bullets,
        "- Release docs contract, test-only: `fork_release_docs_contract.rs`",
    ));
    for needle in [
        "`crate_readme_sections.rs`",
        "`readme_patch_list.rs`",
        "`release_0_17_0_probe.rs`",
        "`release_0_17_0_probe2.rs`",
        "`release_0_17_0_probe3.rs`",
        "`release_0_17_0_probe4.rs`",
        "`release_0_18_0_probe.rs`",
        "`release_0_18_1_probe.rs`",
        "`release_0_18_2_probe.rs`",
        "`release_0_18_3_probe.rs`",
        "`release_notes_breaking.rs`",
        "`request_layer_api_docs.rs`",
        "read `0.18.3` first, with no `Unreleased` label",
    ] {
        assert!(bullet.contains(needle), "the 0.18.3 include-set bullet should say {needle:?}");
    }
}

/// The README patch list runs 0.18.3, then 0.18.2, then 0.18.1: the new
/// block opens the list and the two demoted blocks keep their order below it.
#[test]
fn release_0_18_3_readme_patch_list_runs_newest_first() {
    let readme = read_repo_file("crates/libs/rns-transport/README.md");
    let patch_list = joined_prose(crate_readme_section(&readme, "Patch list"));
    let positions: Vec<usize> = [
        "What `0.18.3` carries on top of `0.18.2`:",
        "What `0.18.2` carries on top of `0.18.1`:",
        "What `0.18.1` carries on top of `0.18.0`:",
    ]
    .iter()
    .map(|marker| {
        patch_list
            .find(marker)
            .unwrap_or_else(|| panic!("the README patch list should carry {marker:?}"))
    })
    .collect();
    assert!(
        positions.windows(2).all(|pair| pair[0] < pair[1]),
        "the README patch list should run 0.18.3, 0.18.2, 0.18.1, got offsets {positions:?}"
    );
}
