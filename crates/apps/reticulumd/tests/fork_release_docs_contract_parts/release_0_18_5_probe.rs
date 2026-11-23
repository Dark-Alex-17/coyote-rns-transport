// Pins over the 0.18.5 release artefacts beyond what the release contracts
// check. Those pin the heading date by shape, the absence of a `### Breaking`
// subsection on a patch bump and the version by its sites; these pins hold
// the heading to the day the release was cut and to the top of the ledger,
// the subsection set an additive patch release with one fix carries, the
// four items `### Added` names, the patch digit and semver verdict the
// version bullet records, the `### Fixed` bullet's opener, the fourteen
// release-docs parts the restated include-set bullet lists, the README
// patch-list order with the 0.18.5 block on top, and the usage pins a
// consumer reads cold: the patch-set ledger opening on this release with
// the id-less release patch in its legend and this file rowed under it, the
// README block's six bullets and consumer note with the recovery recipe,
// and the publish contract's two hand pins reading the release version. The
// pins over the current tree, which the next release cut demotes, are the
// first-release-heading test, the ledger-opening assertion of the patch-set
// test, the README `[dependencies]` snippet pins of the consumer-note test,
// and the crate-manifest version and publish-contract literal count of the
// hand-pins test; the rest hold facts about the 0.18.5 section itself.
// `first_release_heading` and `patchset_opening_section` are shared with
// the 0.18.3 probe.

/// The 0.18.5 heading carries the day the release was cut and is the first
/// release heading of the CHANGELOG.
#[test]
fn release_0_18_5_heading_carries_the_release_date() {
    let changelog = read_repo_file("crates/libs/rns-transport/CHANGELOG.md");
    let heading = "\n## 0.18.5 - 2026-11-23\n";
    assert!(
        changelog.contains(heading),
        "the CHANGELOG should head the 0.18.5 section with its release date"
    );
    let first_release =
        first_release_heading(&changelog).expect("the CHANGELOG should carry a release heading");
    assert_eq!(
        first_release,
        heading.trim(),
        "the 0.18.5 heading should be the first release heading of the CHANGELOG"
    );
}

/// An additive patch release with one timing fix carries no `### Breaking`;
/// the 0.18.5 section is `### Added`, `### Changed`, `### Fixed` and nothing
/// else.
#[test]
fn release_0_18_5_subsections_run_added_changed_fixed() {
    let changelog = read_repo_file("crates/libs/rns-transport/CHANGELOG.md");
    let section = changelog_release_section(&changelog, "0.18.5");
    let names: Vec<&str> = subsections(&section).into_iter().map(|(name, _)| name).collect();
    assert_eq!(
        names,
        ["Added", "Changed", "Fixed"],
        "an additive patch release with a fix carries `### Added`, `### Changed` and `### Fixed` \
         only"
    );
}

/// `### Added` of 0.18.5 is four bullets in this order: the rnsd path calls,
/// `drop_path_settled`, `RpcError::is_transient()` and
/// `Transport::path_interface`, each opening on its name.
#[test]
fn release_0_18_5_added_names_the_path_calls_the_helper_the_predicate_and_the_interface() {
    let changelog = read_repo_file("crates/libs/rns-transport/CHANGELOG.md");
    let section = changelog_release_section(&changelog, "0.18.5");
    let added: Vec<String> =
        subsection_bullets(&section, "Added").iter().map(|bullet| joined_prose(bullet)).collect();
    let openers = [
        "- The path calls of rnsd's RPC listener",
        "- `drop_path_settled(",
        "- `RpcError::is_transient() -> bool`",
        "- `Transport::path_interface(&AddressHash) -> Option<PathIface>`",
    ];
    assert_eq!(
        added.len(),
        openers.len(),
        "the 0.18.5 `### Added` subsection carries four bullets: {added:#?}"
    );
    for (bullet, opener) in added.iter().zip(openers) {
        assert!(
            bullet.starts_with(opener),
            "expected an `### Added` bullet opening {opener:?}, got {bullet:?}"
        );
    }
}

/// The 0.18.5 version bullet, the first under `### Changed`, records why the
/// bump is a patch one and the semver verdict: the baseline tag, that nothing
/// was found, the 0.15.1, 0.18.1, 0.18.2, 0.18.3 and 0.18.4 precedents, the
/// absent `### Breaking`, the timing correction `### Fixed` carries as the
/// one observable behaviour change, the gate the release check runs, the
/// probe that pins the release and the usage pins it carries down to the
/// publish contract's hand pins.
#[test]
fn release_0_18_5_version_bullet_records_the_patch_digit_and_the_semver_verdict() {
    let changelog = read_repo_file("crates/libs/rns-transport/CHANGELOG.md");
    let section = changelog_release_section(&changelog, "0.18.5");
    let bullets = subsection_bullets(&section, "Changed");
    let first = bullets.first().expect("the 0.18.5 `### Changed` subsection should carry bullets");
    assert!(
        first.starts_with("- The crate version is `0.18.5`"),
        "the version bullet heads `### Changed`, got {:?}",
        first
    );
    let bullet =
        joined_prose(&single_bullet_starting_with(&bullets, "- The crate version is `0.18.5`"));
    for needle in [
        "`cargo semver-checks` against `v0.18.4` reports 0 findings",
        "The bump is a patch one under the README policy",
        "as `0.15.1`, `0.18.1`, `0.18.2`, `0.18.3` and `0.18.4` were",
        "additive API a consumer need not opt into",
        "no public item removed or changed",
        "no `### Breaking` subsection",
        "the `### Fixed` timing correction that sizes the first hop as Python does",
        "the shorter link establishment timeout named in `### Fixed`",
        "`tools/scripts/semver-major-findings-check.sh coyote-rns-transport-v0.18.4 0`",
        "`fork_release_docs_contract_parts/release_0_18_5_probe.rs`",
        "the usage pins a consumer reads cold",
        "two hand pins reading `0.18.5`",
    ] {
        assert!(bullet.contains(needle), "the 0.18.5 version bullet should say {needle:?}");
    }
}

/// `### Fixed` of 0.18.5 is the one link establishment timeout bullet: it
/// opens on the timeout `Transport::link` gives a new link and names the
/// Python sizing it now follows.
#[test]
fn release_0_18_5_fixed_names_the_link_establishment_timeout() {
    let changelog = read_repo_file("crates/libs/rns-transport/CHANGELOG.md");
    let section = changelog_release_section(&changelog, "0.18.5");
    let fixed: Vec<String> =
        subsection_bullets(&section, "Fixed").iter().map(|bullet| joined_prose(bullet)).collect();
    assert_eq!(fixed.len(), 1, "the 0.18.5 `### Fixed` subsection carries one bullet: {fixed:#?}");
    assert!(
        fixed[0]
            .starts_with("- The outbound link establishment timeout `Transport::link` gives a new"),
        "the `### Fixed` bullet opens on the link establishment timeout, got {:?}",
        fixed[0]
    );
    assert!(
        fixed[0].contains("fixed 500-byte Reticulum MTU (`RNS/Link.py:282-283`"),
        "the `### Fixed` bullet names the Python sizing with its cite"
    );
}

/// The restated include-set bullet of 0.18.5 names all fourteen parts of
/// `fork_release_docs_contract_parts/`, this file among them, and says the
/// ledger label pins read `0.18.5` first with no `Unreleased` label.
#[test]
fn release_0_18_5_include_set_bullet_names_the_fourteen_parts() {
    let changelog = read_repo_file("crates/libs/rns-transport/CHANGELOG.md");
    let section = changelog_release_section(&changelog, "0.18.5");
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
        "`release_0_18_4_probe.rs`",
        "`release_0_18_5_probe.rs`",
        "`release_notes_breaking.rs`",
        "`request_layer_api_docs.rs`",
        "read `0.18.5` first, with no `Unreleased` label",
    ] {
        assert!(bullet.contains(needle), "the 0.18.5 include-set bullet should say {needle:?}");
    }
}

/// The README patch list runs 0.18.5, then 0.18.4, then 0.18.3: the new
/// block opens the list and the two demoted blocks keep their order below it.
#[test]
fn release_0_18_5_readme_patch_list_runs_newest_first() {
    let readme = read_repo_file("crates/libs/rns-transport/README.md");
    let patch_list = joined_prose(crate_readme_section(&readme, "Patch list"));
    let positions: Vec<usize> = [
        "What `0.18.5` carries on top of `0.18.4`:",
        "What `0.18.4` carries on top of `0.18.3`:",
        "What `0.18.3` carries on top of `0.18.2`:",
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
        "the README patch list should run 0.18.5, 0.18.4, 0.18.3, got offsets {positions:?}"
    );
}

/// The first release section of the patch-set ledger is
/// `## 0.18.5 (upstream 0.14.0)`; its legend names the id-less
/// `release 0.18.5` patch and the table rows this file under
/// `release 0.18.5`.
#[test]
fn release_0_18_5_patchset_opens_on_the_release_patch() {
    let ledger = read_repo_file("docs/fork/PATCHSET.md");
    let section = patchset_opening_section(&ledger);
    assert_eq!(
        section.lines().next(),
        Some("## 0.18.5 (upstream 0.14.0)"),
        "the first release section of the patch-set ledger should be 0.18.5"
    );
    let prose = joined_prose(section);
    assert!(
        prose.contains("The `release 0.18.5` patch carries no other id and is fork commits"),
        "the 0.18.5 legend names the id-less release patch"
    );
    assert!(
        section.lines().any(|line| {
            line == "| `crates/apps/reticulumd/tests/fork_release_docs_contract_parts/release_0_18_5_probe.rs` | release 0.18.5 |"
        }),
        "the 0.18.5 table should row this probe under `release 0.18.5`"
    );
}

/// The README 0.18.5 patch-list block carries one bullet per shipped change
/// (six, the consumer note last) and the consumer note spells out the
/// recovery recipe: `drop_path_settled` then one `request_path`, retries
/// classified with `is_transient()`, the in-process `path_interface` read
/// with `interface_events()` as the up/down stream, and the shorter link
/// establishment timeout as the one observable behaviour change; the
/// dependency snippet pins the release and no 0.18.4 snippet survives.
#[test]
fn release_0_18_5_readme_consumer_note_names_the_recovery_recipe() {
    let readme = read_repo_file("crates/libs/rns-transport/README.md");
    let marker = "What `0.18.5` carries on top";
    let at = readme.find(marker).expect("the README patch list has the 0.18.5 block");
    let block = &readme[at + marker.len()..];
    let block =
        &block[..block.find("\nWhat `0.18.4` carries on top").expect("the 0.18.4 block follows")];
    assert!(
        joined_prose(block).starts_with("of `0.18.4`:"),
        "the 0.18.5 marker sentence names the 0.18.4 baseline"
    );
    let bullets: Vec<String> =
        top_level_bullets(block).iter().map(|bullet| joined_prose(bullet)).collect();
    assert_eq!(
        bullets.len(),
        6,
        "the 0.18.5 block carries one bullet per shipped change: the rnsd path calls, the path \
         interface, the link establishment timeout fix, the Python proofs, the test-only \
         hygiene and the consumer note; got {bullets:#?}"
    );
    let openers = [
        "- The rnsd path calls:",
        "- The path interface:",
        "- The link establishment timeout fix:",
        "- The Python-in-the-loop proofs:",
        "- Test-only hygiene:",
        "- For a consumer: pin `coyote-rns-transport = \"0.18.5\"`;",
    ];
    for (bullet, opener) in bullets.iter().zip(openers) {
        assert!(bullet.starts_with(opener), "expected a bullet opening {opener:?}, got {bullet:?}");
    }
    let consumer = &bullets[5];
    for needle in [
        "there is nothing to opt into",
        "`drop_path_settled`",
        "`request_path`",
        "`is_transient()`",
        "`interface_stats`",
        "`path_interface`",
        "`interface_events()`",
        "about 12 s",
    ] {
        assert!(consumer.contains(needle), "the consumer note should say {needle:?}: {consumer}");
    }
    let snippet = "coyote-rns-transport = \"0.18.5\"";
    assert!(
        readme.contains(&format!("[dependencies]\n{snippet}\n")),
        "the README dependency snippet should pin {snippet:?}"
    );
    assert!(
        !readme.contains("[dependencies]\ncoyote-rns-transport = \"0.18.4\""),
        "the README dependency snippet should no longer pin 0.18.4"
    );
}

/// The publish contract's two hand pins (the bare version and the manifest
/// line) read the release version with no stale 0.18.4 pin beside them, the
/// crate manifest declares it, and the root `VERSION` file stays.
#[test]
fn release_0_18_5_publish_contract_hand_pins_read_the_release_version() {
    let contract = read_repo_file("crates/apps/reticulumd/tests/fork_publish_contract.rs");
    assert!(
        contract.lines().count() <= 1200,
        "fork_publish_contract.rs stays within the 1200-line test policy"
    );
    let pins: Vec<&str> = contract.lines().filter(|line| line.contains("0.18.5")).collect();
    assert_eq!(pins.len(), 2, "the publish contract carries its two hand pins, got {pins:?}");
    assert!(
        !contract.contains("0.18.4\""),
        "no stale 0.18.4 hand pin survives in the publish contract"
    );
    let manifest = read_repo_file("crates/libs/rns-transport/Cargo.toml");
    assert!(manifest.contains("\nversion = \"0.18.5\"\n"), "the crate manifest declares 0.18.5");
    assert_eq!(read_repo_file("VERSION").trim(), "0.14.0", "the root VERSION file stays");
}
