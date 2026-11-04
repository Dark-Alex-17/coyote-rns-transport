// Pins over the 0.18.1 release artefacts beyond what the release contracts
// check. Those pin the heading date by shape, the absence of a `### Breaking`
// subsection on a patch bump and the version by its sites; these pins hold
// the heading to the day the release was cut and to the top of the ledger,
// the subsection set a patch release carries, the connect-count items
// `### Added` names, the cadence wording of the failed-connect bullet, the
// patch digit and semver verdict the version bullet records, the CI-only
// bullet for the lxmd smoke ports, and the README patch-list order with the
// 0.18.1 block on top.

/// The 0.18.1 heading carries the day the release was cut and is the first
/// release heading of the CHANGELOG.
#[test]
fn release_0_18_1_heading_carries_the_release_date() {
    let changelog = read_repo_file("crates/libs/rns-transport/CHANGELOG.md");
    let heading = "\n## 0.18.1 - 2026-11-03\n";
    assert!(
        changelog.contains(heading),
        "the CHANGELOG should head the 0.18.1 section with its release date"
    );
    let first_release = changelog
        .lines()
        .find(|line| line.starts_with("## ") && line[3..].starts_with(|c: char| c.is_ascii_digit()))
        .expect("the CHANGELOG should carry a release heading");
    assert_eq!(
        first_release,
        heading.trim(),
        "the 0.18.1 heading should be the first release heading of the CHANGELOG"
    );
}

/// A patch release carries neither `### Breaking` nor `### Fixed`; the
/// 0.18.1 section is `### Added` then `### Changed` and nothing else.
#[test]
fn release_0_18_1_has_no_breaking_subsection_as_a_patch_release() {
    let changelog = read_repo_file("crates/libs/rns-transport/CHANGELOG.md");
    let section = changelog_release_section(&changelog, "0.18.1");
    let names: Vec<&str> = subsections(&section).into_iter().map(|(name, _)| name).collect();
    assert_eq!(
        names,
        ["Added", "Changed"],
        "a patch release carries `### Added` and `### Changed` only"
    );
}

/// `### Added` of 0.18.1 names the connect counter by the transport getter,
/// written with its argument, and the status key a consumer will grep for.
#[test]
fn release_0_18_1_added_names_the_connect_count_api() {
    let changelog = read_repo_file("crates/libs/rns-transport/CHANGELOG.md");
    let section = changelog_release_section(&changelog, "0.18.1");
    let added: Vec<String> =
        subsection_bullets(&section, "Added").iter().map(|bullet| joined_prose(bullet)).collect();
    assert!(!added.is_empty(), "the 0.18.1 `### Added` subsection should carry bullets");
    for needle in ["`Transport::interface_connect_count(", "`\"connected_streams\"`"] {
        assert!(
            added.iter().any(|bullet| bullet.contains(needle)),
            "the 0.18.1 `### Added` subsection should name the connect counter by {needle:?}"
        );
    }
}

/// The failed-connect bullet of 0.18.1, found by its opening words, names
/// the dial-warn cadence by its stride constant and the attempt suffix every
/// failed-connect line ends in; the version bullet's mention of the cadence
/// does not satisfy the pin.
#[test]
fn release_0_18_1_changed_names_the_dial_warn_stride() {
    let changelog = read_repo_file("crates/libs/rns-transport/CHANGELOG.md");
    let section = changelog_release_section(&changelog, "0.18.1");
    let bullets = subsection_bullets(&section, "Changed");
    let bullet = joined_prose(&single_bullet_starting_with(
        &bullets,
        "- The failed-connect line of `TcpClient`",
    ));
    for needle in ["`DIAL_WARN_STRIDE`", " (attempt N)"] {
        assert!(
            bullet.contains(needle),
            "the 0.18.1 failed-connect bullet should name the cadence by {needle:?}"
        );
    }
}

/// The one `### Changed` bullet opening `- No library behaviour change (CI):`
/// is the lxmd smoke script's port derivation: the script, the `127.0.0.1:0`
/// bind it takes its ports from and the probe that pins it.
#[test]
fn release_0_18_1_changed_marks_the_lxmd_smoke_ports_as_ci_only() {
    let changelog = read_repo_file("crates/libs/rns-transport/CHANGELOG.md");
    let section = changelog_release_section(&changelog, "0.18.1");
    let bullets = subsection_bullets(&section, "Changed");
    let bullet =
        joined_prose(&single_bullet_starting_with(&bullets, "- No library behaviour change (CI):"));
    for needle in [
        "`tools/scripts/python-lxmd-rust-lxmd-smoke.sh`",
        "`127.0.0.1:0`",
        "`crates/apps/reticulumd/tests/fork_publish_contract_parts/lxmd_smoke_free_ports_probe.rs`",
    ] {
        assert!(bullet.contains(needle), "the 0.18.1 CI-only bullet should say {needle:?}");
    }
}

/// The 0.18.1 version bullet, the first under `### Changed`, records why the
/// bump is a patch one and the semver verdict: the baseline tag, that nothing
/// was found, the 0.15.1 precedent, the absent `### Breaking` and the gate
/// the release check runs.
#[test]
fn release_0_18_1_version_bullet_records_the_patch_digit_and_the_semver_verdict() {
    let changelog = read_repo_file("crates/libs/rns-transport/CHANGELOG.md");
    let section = changelog_release_section(&changelog, "0.18.1");
    let bullets = subsection_bullets(&section, "Changed");
    assert!(
        bullets[0].starts_with("- The crate version is `0.18.1`"),
        "the version bullet heads `### Changed`, got {:?}",
        bullets[0]
    );
    let bullet =
        joined_prose(&single_bullet_starting_with(&bullets, "- The crate version is `0.18.1`"));
    for needle in [
        "`cargo semver-checks` against `v0.18.0` reports 0 findings",
        "The bump is a patch one under the README policy",
        "as `0.15.1` was",
        "no `### Breaking` subsection",
        "`tools/scripts/semver-major-findings-check.sh coyote-rns-transport-v0.18.0 0`",
    ] {
        assert!(bullet.contains(needle), "the 0.18.1 version bullet should say {needle:?}");
    }
}

/// The README patch list runs 0.18.1, then 0.18.0, then 0.17.0: the new
/// block opens the list and the two demoted blocks keep their order below it.
#[test]
fn release_0_18_1_readme_patch_list_runs_newest_first() {
    let readme = read_repo_file("crates/libs/rns-transport/README.md");
    let patch_list = joined_prose(crate_readme_section(&readme, "Patch list"));
    let positions: Vec<usize> = [
        "What `0.18.1` carries on top of `0.18.0`:",
        "What `0.18.0` carries on top of `0.17.0`:",
        "What `0.17.0` carries on top of `0.16.0`:",
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
        "the README patch list should run 0.18.1, 0.18.0, 0.17.0, got offsets {positions:?}"
    );
}
