// Pins over the 0.18.4 release artefacts beyond what the release contracts
// check. Those pin the heading date by shape, the absence of a `### Breaking`
// subsection on a patch bump and the version by its sites; these pins hold
// the heading to the day the release was cut and to the top of the ledger,
// the subsection set a purely additive patch release carries, the one item
// `### Added` names, the patch digit and semver verdict the version bullet
// records, the thirteen release-docs parts the restated include-set bullet
// lists, the README patch-list order with the 0.18.4 block on top, and the
// usage pins a consumer reads cold: the patch-set ledger opening on this
// release with the release patch and the adopted probe commits cited, the
// README block's four bullets and consumer note with the retry rule, and the
// publish contract's two hand pins reading the release version. The pins
// over the current tree, which the next release cut demotes, are the
// first-release-heading test, the ledger-opening assertion of the patch-set
// test, the README `[dependencies]` snippet and four-bullet opener pins of
// the consumer-note test, and the crate-manifest version and
// publish-contract literal count of the hand-pins test; the rest hold facts
// about the 0.18.4 section itself. `first_release_heading` and
// `patchset_opening_section` are shared with the 0.18.3 probe.

/// The 0.18.4 heading carries the day the release was cut and is the first
/// release heading of the CHANGELOG.
#[test]
fn release_0_18_4_heading_carries_the_release_date() {
    let changelog = read_repo_file("crates/libs/rns-transport/CHANGELOG.md");
    let heading = "\n## 0.18.4 - 2026-11-20\n";
    assert!(
        changelog.contains(heading),
        "the CHANGELOG should head the 0.18.4 section with its release date"
    );
    let first_release =
        first_release_heading(&changelog).expect("the CHANGELOG should carry a release heading");
    assert_eq!(
        first_release,
        heading.trim(),
        "the 0.18.4 heading should be the first release heading of the CHANGELOG"
    );
}

/// A purely additive patch release carries neither `### Breaking` nor
/// `### Fixed`; the 0.18.4 section is `### Added` then `### Changed` and
/// nothing else.
#[test]
fn release_0_18_4_subsections_run_added_changed() {
    let changelog = read_repo_file("crates/libs/rns-transport/CHANGELOG.md");
    let section = changelog_release_section(&changelog, "0.18.4");
    let names: Vec<&str> = subsections(&section).into_iter().map(|(name, _)| name).collect();
    assert_eq!(
        names,
        ["Added", "Changed"],
        "an additive patch release carries `### Added` and `### Changed` only"
    );
}

/// `### Added` of 0.18.4 is the one io-error bullet: it opens on the new
/// variant and names the `io_kind()` accessor a matcher moves to.
#[test]
fn release_0_18_4_added_names_the_io_variant_and_the_kind_accessor() {
    let changelog = read_repo_file("crates/libs/rns-transport/CHANGELOG.md");
    let section = changelog_release_section(&changelog, "0.18.4");
    let added: Vec<String> =
        subsection_bullets(&section, "Added").iter().map(|bullet| joined_prose(bullet)).collect();
    assert_eq!(added.len(), 1, "the 0.18.4 `### Added` subsection carries one bullet: {added:#?}");
    assert!(
        added[0].starts_with("- `RpcError::Io(std::io::Error)`"),
        "the `### Added` bullet opens on the new variant, got {:?}",
        added[0]
    );
    assert!(
        added[0].contains("`RpcError::io_kind() -> Option<std::io::ErrorKind>`"),
        "the `### Added` bullet names the kind accessor with its signature"
    );
}

/// The 0.18.4 version bullet, the first under `### Changed`, records why the
/// bump is a patch one and the semver verdict: the baseline tag, that nothing
/// was found, the 0.15.1, 0.18.1, 0.18.2 and 0.18.3 precedents, the absent
/// `### Breaking`, the one observable change for an existing matcher, the
/// gate the release check runs and the probe that pins the release.
#[test]
fn release_0_18_4_version_bullet_records_the_patch_digit_and_the_semver_verdict() {
    let changelog = read_repo_file("crates/libs/rns-transport/CHANGELOG.md");
    let section = changelog_release_section(&changelog, "0.18.4");
    let bullets = subsection_bullets(&section, "Changed");
    let first = bullets.first().expect("the 0.18.4 `### Changed` subsection should carry bullets");
    assert!(
        first.starts_with("- The crate version is `0.18.4`"),
        "the version bullet heads `### Changed`, got {:?}",
        first
    );
    let bullet =
        joined_prose(&single_bullet_starting_with(&bullets, "- The crate version is `0.18.4`"));
    for needle in [
        "`cargo semver-checks` against `v0.18.3` reports 0 findings",
        "The bump is a patch one under the README policy",
        "as `0.15.1`, `0.18.1`, `0.18.2` and `0.18.3` were",
        "additive API a consumer need not opt into",
        "no public item removed or changed",
        "no `### Breaking` subsection",
        "surfacing as `Io` instead of `Protocol`",
        "`tools/scripts/semver-major-findings-check.sh coyote-rns-transport-v0.18.3 0`",
        "`fork_release_docs_contract_parts/release_0_18_4_probe.rs`",
    ] {
        assert!(bullet.contains(needle), "the 0.18.4 version bullet should say {needle:?}");
    }
}

/// The restated include-set bullet of 0.18.4 names all thirteen parts of
/// `fork_release_docs_contract_parts/`, this file among them, and says the
/// ledger label pins read `0.18.4` first with no `Unreleased` label.
#[test]
fn release_0_18_4_include_set_bullet_names_the_thirteen_parts() {
    let changelog = read_repo_file("crates/libs/rns-transport/CHANGELOG.md");
    let section = changelog_release_section(&changelog, "0.18.4");
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
        "`release_notes_breaking.rs`",
        "`request_layer_api_docs.rs`",
        "read `0.18.4` first, with no `Unreleased` label",
    ] {
        assert!(bullet.contains(needle), "the 0.18.4 include-set bullet should say {needle:?}");
    }
}

/// The README patch list runs 0.18.4, then 0.18.3, then 0.18.2: the new
/// block opens the list and the two demoted blocks keep their order below it.
#[test]
fn release_0_18_4_readme_patch_list_runs_newest_first() {
    let readme = read_repo_file("crates/libs/rns-transport/README.md");
    let patch_list = joined_prose(crate_readme_section(&readme, "Patch list"));
    let positions: Vec<usize> = [
        "What `0.18.4` carries on top of `0.18.3`:",
        "What `0.18.3` carries on top of `0.18.2`:",
        "What `0.18.2` carries on top of `0.18.1`:",
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
        "the README patch list should run 0.18.4, 0.18.3, 0.18.2, got offsets {positions:?}"
    );
}

/// The first release section of the patch-set ledger is
/// `## 0.18.4 (upstream 0.14.0)`; its legend names the id-less
/// `release 0.18.4` patch, the `rpc io error` legend cites the two probe
/// commits adopted since 0.18.3, and the table rows this file under
/// `release 0.18.4`. The release commits themselves are not pinned here:
/// their hashes exist only once the ledger commit is cut.
#[test]
fn release_0_18_4_patchset_opens_on_the_release_patch() {
    let ledger = read_repo_file("docs/fork/PATCHSET.md");
    let section = patchset_opening_section(&ledger);
    assert_eq!(
        section.lines().next(),
        Some("## 0.18.4 (upstream 0.14.0)"),
        "the first release section of the patch-set ledger should be 0.18.4"
    );
    let prose = joined_prose(section);
    assert!(
        prose.contains("The `release 0.18.4` patch carries no other id and is fork commits"),
        "the 0.18.4 legend names the id-less release patch"
    );
    let io_legend_start = prose
        .find("The `rpc io error` patch is fork commits")
        .expect("the 0.18.4 legend names the `rpc io error` patch");
    let io_legend = &prose[io_legend_start..];
    let io_legend = &io_legend[..io_legend.find('.').expect("sentence end")];
    for commit in ["`9a0eb46a`", "`a58b664e`"] {
        assert!(
            io_legend.contains(commit),
            "the `rpc io error` legend should cite {commit}: {io_legend}"
        );
    }
    assert!(
        section.lines().any(|line| {
            line == "| `crates/apps/reticulumd/tests/fork_release_docs_contract_parts/release_0_18_4_probe.rs` | release 0.18.4 |"
        }),
        "the 0.18.4 table should row this probe under `release 0.18.4`"
    );
}

/// The README 0.18.4 patch-list block carries one bullet per shipped change
/// (four, the consumer note last) and the consumer note spells out the retry
/// rule: `io_kind()` over both variants, the four retryable kinds,
/// `ConnectionRefused` kept on its own `Connect` arm, bounded attempts with a
/// backoff, and the one observable change for an existing matcher; the
/// dependency snippet pins the release and no 0.18.3 snippet survives.
#[test]
fn release_0_18_4_readme_consumer_note_names_the_retry_rule() {
    let readme = read_repo_file("crates/libs/rns-transport/README.md");
    let marker = "What `0.18.4` carries on top";
    let at = readme.find(marker).expect("the README patch list has the 0.18.4 block");
    let block = &readme[at + marker.len()..];
    let block =
        &block[..block.find("\nWhat `0.18.3` carries on top").expect("the 0.18.3 block follows")];
    assert!(
        joined_prose(block).starts_with("of `0.18.3`:"),
        "the 0.18.4 marker sentence names the 0.18.3 baseline"
    );
    let bullets: Vec<String> =
        top_level_bullets(block).iter().map(|bullet| joined_prose(bullet)).collect();
    assert_eq!(
        bullets.len(),
        4,
        "the 0.18.4 block carries one bullet per shipped change: the rpc io error, the README \
         fork framing, the Python proof and the consumer note; got {bullets:#?}"
    );
    let openers = [
        "- The rpc io error:",
        "- The README fork framing",
        "- The Python-in-the-loop proof:",
        "- For a consumer: pin `coyote-rns-transport = \"0.18.4\"`;",
    ];
    for (bullet, opener) in bullets.iter().zip(openers) {
        assert!(bullet.starts_with(opener), "expected a bullet opening {opener:?}, got {bullet:?}");
    }
    let consumer = &bullets[3];
    for needle in [
        "there is nothing to opt into",
        "`io_kind()`",
        "`ConnectionReset`",
        "`ConnectionAborted`",
        "`BrokenPipe`",
        "`UnexpectedEof`",
        "`ConnectionRefused`",
        "backoff",
        "`Io` instead of `Protocol`",
    ] {
        assert!(consumer.contains(needle), "the consumer note should say {needle:?}: {consumer}");
    }
    let snippet = "coyote-rns-transport = \"0.18.4\"";
    assert!(
        readme.contains(&format!("[dependencies]\n{snippet}\n")),
        "the README dependency snippet should pin {snippet:?}"
    );
    assert!(
        !readme.contains("[dependencies]\ncoyote-rns-transport = \"0.18.3\""),
        "the README dependency snippet should no longer pin 0.18.3"
    );
}

/// The publish contract's two hand pins (the bare version and the manifest
/// line) read the release version with no stale 0.18.3 pin beside them, the
/// crate manifest declares it, and the root `VERSION` file stays.
#[test]
fn release_0_18_4_publish_contract_hand_pins_read_the_release_version() {
    let contract = read_repo_file("crates/apps/reticulumd/tests/fork_publish_contract.rs");
    assert!(
        contract.lines().count() <= 1200,
        "fork_publish_contract.rs stays within the 1200-line test policy"
    );
    let pins: Vec<&str> = contract.lines().filter(|line| line.contains("0.18.4")).collect();
    assert_eq!(pins.len(), 2, "the publish contract carries its two hand pins, got {pins:?}");
    assert!(
        !contract.contains("0.18.3\""),
        "no stale 0.18.3 hand pin survives in the publish contract"
    );
    let manifest = read_repo_file("crates/libs/rns-transport/Cargo.toml");
    assert!(manifest.contains("\nversion = \"0.18.4\"\n"), "the crate manifest declares 0.18.4");
    assert_eq!(read_repo_file("VERSION").trim(), "0.14.0", "the root VERSION file stays");
}
