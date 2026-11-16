// Pins over the 0.18.2 release artefacts beyond what the release contracts
// check. Those pin the heading date by shape, the absence of a `### Breaking`
// subsection on a patch bump and the version by its sites; these pins hold
// the heading to the day the release was cut and to the top of the ledger,
// the subsection set a patch release with fixes carries, the tunnel opt-in
// builder `### Added` names, the single-request and fail-fast bullets under
// `### Changed`, the patch digit and semver verdict the version bullet
// records, the two `### Fixed` bullets by their opening words, and the README
// patch-list order with the 0.18.2 block on top.

/// The 0.18.2 heading carries the day the release was cut and is the first
/// release heading of the CHANGELOG.
#[test]
fn release_0_18_2_heading_carries_the_release_date() {
    let changelog = read_repo_file("crates/libs/rns-transport/CHANGELOG.md");
    let heading = "\n## 0.18.2 - 2026-11-16\n";
    assert!(
        changelog.contains(heading),
        "the CHANGELOG should head the 0.18.2 section with its release date"
    );
    let first_release = changelog
        .lines()
        .find(|line| line.starts_with("## ") && line[3..].starts_with(|c: char| c.is_ascii_digit()))
        .expect("the CHANGELOG should carry a release heading");
    assert_eq!(
        first_release,
        heading.trim(),
        "the 0.18.2 heading should be the first release heading of the CHANGELOG"
    );
}

/// A patch release whose fixes correct default behaviour carries `### Fixed`
/// after `### Changed`, never `### Breaking`; the 0.18.2 section is
/// `### Added`, `### Changed`, `### Fixed` and nothing else.
#[test]
fn release_0_18_2_subsections_run_added_changed_fixed() {
    let changelog = read_repo_file("crates/libs/rns-transport/CHANGELOG.md");
    let section = changelog_release_section(&changelog, "0.18.2");
    let names: Vec<&str> = subsections(&section).into_iter().map(|(name, _)| name).collect();
    assert_eq!(
        names,
        ["Added", "Changed", "Fixed"],
        "a patch release with fixes carries `### Added`, `### Changed` and `### Fixed` only"
    );
}

/// `### Added` of 0.18.2 names the tunnel opt-in by the builder, written
/// with its argument, and says it is off by default.
#[test]
fn release_0_18_2_added_names_the_tunnel_opt_in_builder() {
    let changelog = read_repo_file("crates/libs/rns-transport/CHANGELOG.md");
    let section = changelog_release_section(&changelog, "0.18.2");
    let added: Vec<String> =
        subsection_bullets(&section, "Added").iter().map(|bullet| joined_prose(bullet)).collect();
    assert!(!added.is_empty(), "the 0.18.2 `### Added` subsection should carry bullets");
    for needle in ["`SharedInstanceOptions::with_tunnel_synthesis(bool)`", "Off by default"] {
        assert!(
            added.iter().any(|bullet| bullet.contains(needle)),
            "the 0.18.2 `### Added` subsection should name the tunnel opt-in by {needle:?}"
        );
    }
}

/// `### Changed` of 0.18.2 carries the single-request bullet and the
/// fail-fast activation wait bullet, each found by its opening words; the
/// version bullet's summary of them does not satisfy the pin.
#[test]
fn release_0_18_2_changed_names_the_single_request_and_the_fail_fast_wait() {
    let changelog = read_repo_file("crates/libs/rns-transport/CHANGELOG.md");
    let section = changelog_release_section(&changelog, "0.18.2");
    let bullets = subsection_bullets(&section, "Changed");
    for opener in [
        "- A pending outbound link now sends its link request once",
        "- `delivery::await_link_activation` (`delivery.rs`) now returns",
    ] {
        assert!(
            bullets.iter().any(|bullet| bullet.starts_with(opener)),
            "the 0.18.2 `### Changed` subsection should carry a bullet opening {opener:?}"
        );
    }
}

/// The 0.18.2 version bullet, the first under `### Changed`, records why the
/// bump is a patch one and the semver verdict: the baseline tag, that nothing
/// was found, the 0.15.1 and 0.18.1 precedents, the absent `### Breaking`,
/// the gate the release check runs and the probe that pins the release.
#[test]
fn release_0_18_2_version_bullet_records_the_patch_digit_and_the_semver_verdict() {
    let changelog = read_repo_file("crates/libs/rns-transport/CHANGELOG.md");
    let section = changelog_release_section(&changelog, "0.18.2");
    let bullets = subsection_bullets(&section, "Changed");
    let first = bullets.first().expect("the 0.18.2 `### Changed` subsection should carry bullets");
    assert!(
        first.starts_with("- The crate version is `0.18.2`"),
        "the version bullet heads `### Changed`, got {:?}",
        first
    );
    let bullet =
        joined_prose(&single_bullet_starting_with(&bullets, "- The crate version is `0.18.2`"));
    for needle in [
        "`cargo semver-checks` against `v0.18.1` reports 0 findings",
        "The bump is a patch one under the README policy",
        "as `0.15.1` and `0.18.1` were",
        "no `### Breaking` subsection",
        "`tools/scripts/semver-major-findings-check.sh coyote-rns-transport-v0.18.1 0`",
        "`fork_release_docs_contract_parts/release_0_18_2_probe.rs`",
    ] {
        assert!(bullet.contains(needle), "the 0.18.2 version bullet should say {needle:?}");
    }
}

/// `### Fixed` of 0.18.2 carries the same-identity tunnel collision and the
/// shared-instance announce admission, each found by its opening words.
#[test]
fn release_0_18_2_fixed_names_the_tunnel_collision_and_the_announce_admission() {
    let changelog = read_repo_file("crates/libs/rns-transport/CHANGELOG.md");
    let section = changelog_release_section(&changelog, "0.18.2");
    let bullets = subsection_bullets(&section, "Fixed");
    for opener in [
        "- Two transports attached to one shared instance with one identity",
        "- A transport attached to a shared instance ran its announce ingress",
    ] {
        assert!(
            bullets.iter().any(|bullet| bullet.starts_with(opener)),
            "the 0.18.2 `### Fixed` subsection should carry a bullet opening {opener:?}"
        );
    }
}

/// The README patch list runs 0.18.2, then 0.18.1, then 0.18.0: the new
/// block opens the list and the two demoted blocks keep their order below it.
#[test]
fn release_0_18_2_readme_patch_list_runs_newest_first() {
    let readme = read_repo_file("crates/libs/rns-transport/README.md");
    let patch_list = joined_prose(crate_readme_section(&readme, "Patch list"));
    let positions: Vec<usize> = [
        "What `0.18.2` carries on top of `0.18.1`:",
        "What `0.18.1` carries on top of `0.18.0`:",
        "What `0.18.0` carries on top of `0.17.0`:",
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
        "the README patch list should run 0.18.2, 0.18.1, 0.18.0, got offsets {positions:?}"
    );
}
