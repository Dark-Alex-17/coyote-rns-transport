// Pins over the 0.18.0 release artefacts beyond what the release contracts
// check. Those pin the heading date by shape, the `### Breaking` bullet by
// its `Migration:` clause and the version by its sites; these pins hold the
// heading to the day the release was cut, the one break to the lint name
// and the cast it rejects, the seven items `### Added` names, the semver
// verdict the version bullet records, the `### Fixed` subsection the
// release opens with the positive control its `await_link_activation`
// bullet cites, and the README patch-list order with the 0.18.0 block on
// top.

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

/// The identifiers the seven 0.18.0 items are known by, one group per item;
/// every needle of a group must appear in some `### Added` bullet.
const RELEASE_0_18_0_ADDED_ITEMS: [(&str, &[&str]); 7] = [
    ("owned request split", &["`InboundRequest::into_parts`", "`ResponseOutcomeTracker`"]),
    ("responder cap", &["`set_max_response_size`", "TooLarge"]),
    (
        "dial failures and first-connect timeout",
        &[
            "`with_dial_failure_events`",
            "`SharedInstanceEvent::DialFailed`",
            "with_first_connect_timeout",
        ],
    ),
    (
        "disconnect reasons",
        &["`with_disconnect_reason_events`", "`SharedInstanceEvent::DisconnectedWithReason`"],
    ),
    ("interface events", &["`Transport::interface_events`"]),
    ("destination resolve", &["`Transport::resolve_destination`"]),
    ("link proof strategy", &["`ProofStrategy`", "`Transport::prove_link_packet`"]),
];

/// `### Added` of 0.18.0 names every one of the seven items by the
/// identifiers a consumer will grep for; a bullet lost in a rebase or a
/// renamed identifier would read as a release that dropped an item.
#[test]
fn release_0_18_0_added_names_each_of_the_seven_items() {
    let changelog = read_repo_file("crates/libs/rns-transport/CHANGELOG.md");
    let section = changelog_release_section(&changelog, "0.18.0");
    let added: Vec<String> =
        subsection_bullets(&section, "Added").iter().map(|bullet| joined_prose(bullet)).collect();
    assert!(!added.is_empty(), "the 0.18.0 `### Added` subsection should carry bullets");
    for (item, needles) in RELEASE_0_18_0_ADDED_ITEMS {
        for needle in needles {
            assert!(
                added.iter().any(|bullet| bullet.contains(needle)),
                "the 0.18.0 `### Added` subsection should name the {item} by {needle:?}"
            );
        }
    }
}

/// The 0.18.0 version bullet, the first under `### Changed`, records the
/// semver verdict: the baseline tag, the single finding and its lint name,
/// the one the release check tolerates by name, and that nothing else was
/// found, so a reader learns the break list is complete.
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
        "and 0 others",
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

/// The `### Fixed` bullet for `await_link_activation` cites the adopted
/// positive control, and that file is on disk where the cite points.
#[test]
fn release_0_18_0_fixed_cites_the_adopted_positive_control() {
    let changelog = read_repo_file("crates/libs/rns-transport/CHANGELOG.md");
    let section = changelog_release_section(&changelog, "0.18.0");
    let bullet = subsection_bullets(&section, "Fixed")
        .iter()
        .map(|bullet| joined_prose(bullet))
        .find(|bullet| bullet.contains("`delivery::await_link_activation`"))
        .expect("a 0.18.0 `### Fixed` bullet should name `delivery::await_link_activation`");
    let cite = "`tests/link_activation_max_timeout_probe.rs`";
    assert!(
        bullet.contains(cite),
        "the await_link_activation fix should cite {cite}, got {bullet:?}"
    );
    let on_disk =
        repo_root().join("crates/libs/rns-transport/tests/link_activation_max_timeout_probe.rs");
    assert!(on_disk.is_file(), "the cited positive control should exist at {}", on_disk.display());
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
