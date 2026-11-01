// Pins over the 0.18.0 release artefacts beyond what the release contracts
// check. Those pin the heading date by shape, the `### Breaking` bullet by
// its `Migration:` clause and the version by its sites; these pins hold the
// heading to the day the release was cut, the one break to the lint name
// and the cast it rejects, the semver verdict the version bullet records,
// the `### Fixed` subsection the release opens, and the README patch-list
// order with the 0.18.0 block on top.

/// The 0.18.0 heading carries the day the release was cut.
#[test]
fn release_0_18_0_heading_carries_the_release_date() {
    let changelog = read_repo_file("crates/libs/rns-transport/CHANGELOG.md");
    assert!(
        changelog.contains("\n## 0.18.0 - 2026-11-01\n"),
        "the CHANGELOG should head the 0.18.0 section with its release date"
    );
}

/// The 0.18.0 `### Breaking` is the first subsection and lists the one
/// break: the data-carrying `SharedInstanceEvent` variants, which stop an
/// integer cast of the unit variants from compiling, named by the lint
/// `cargo semver-checks` reports and followed by a `Migration:` clause.
#[test]
fn release_0_18_0_breaking_lists_the_one_enum_cast_break() {
    let changelog = read_repo_file("crates/libs/rns-transport/CHANGELOG.md");
    let section = changelog_release_section(&changelog, "0.18.0");
    let subs = subsections(&section);
    let (first_name, _) = subs.first().expect("the 0.18.0 section should have subsections");
    assert_eq!(
        *first_name, "Breaking",
        "the first subsection of a minor release is `### Breaking`"
    );
    let bullets = subsection_bullets(&section, "Breaking");
    assert_eq!(bullets.len(), 1, "0.18.0 lists exactly one break, got {bullets:?}");
    let bullet = joined_prose(&bullets[0]);
    for needle in [
        "`SharedInstanceEvent`",
        "`enum_discriminants_undefined_non_unit_variant`",
        "as isize",
        "Migration:",
    ] {
        assert!(bullet.contains(needle), "the 0.18.0 `### Breaking` bullet should say {needle:?}");
    }
}

/// The 0.18.0 version bullet, the first under `### Changed`, records the
/// semver verdict: the baseline tag, the single finding and its lint name,
/// the one the release check tolerates by name.
#[test]
fn release_0_18_0_version_bullet_records_the_semver_verdict() {
    let changelog = read_repo_file("crates/libs/rns-transport/CHANGELOG.md");
    let section = changelog_release_section(&changelog, "0.18.0");
    let bullets = subsection_bullets(&section, "Changed");
    assert!(
        bullets[0].starts_with("- The crate version is `0.18.0`"),
        "the version bullet heads `### Changed`, got {:?}",
        bullets[0]
    );
    let bullet =
        joined_prose(&single_bullet_starting_with(&bullets, "- The crate version is `0.18.0`"));
    for needle in [
        "`cargo semver-checks` against `v0.17.0`",
        "exactly one finding",
        "`enum_discriminants_undefined_non_unit_variant`",
    ] {
        assert!(bullet.contains(needle), "the 0.18.0 version bullet should say {needle:?}");
    }
}

/// 0.18.0 opens a `### Fixed` subsection for the two deadline fixes, each
/// named by the function it corrects, so a consumer reading the release for
/// bug fixes finds them apart from the additions and the hygiene bullets.
#[test]
fn release_0_18_0_fixed_names_both_deadline_fixes() {
    let changelog = read_repo_file("crates/libs/rns-transport/CHANGELOG.md");
    let section = changelog_release_section(&changelog, "0.18.0");
    let fixed = subsection_bullets(&section, "Fixed");
    for function in ["`Transport::await_path`", "`delivery::await_link_activation`"] {
        assert!(
            fixed.iter().any(|bullet| joined_prose(bullet).contains(function)),
            "a 0.18.0 `### Fixed` bullet should name {function}, got {fixed:?}"
        );
    }
}

/// The README patch list runs 0.18.0, then 0.17.0, then 0.16.0: the new
/// block opens the list and the two demoted blocks keep their order below it.
#[test]
fn release_0_18_0_readme_patch_list_runs_newest_first() {
    let readme = read_repo_file("crates/libs/rns-transport/README.md");
    let patch_list = joined_prose(crate_readme_section(&readme, "Patch list"));
    let positions: Vec<usize> = [
        "What `0.18.0` carries on top of `0.17.0`:",
        "What `0.17.0` carries on top of `0.16.0`:",
        "What `0.16.0` carries on top of `0.15.1`:",
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
        "the README patch list should run 0.18.0, 0.17.0, 0.16.0, got offsets {positions:?}"
    );
}
