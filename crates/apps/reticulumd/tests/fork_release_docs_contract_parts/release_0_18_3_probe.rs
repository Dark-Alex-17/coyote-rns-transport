// Pins over the 0.18.3 release artefacts beyond what the release contracts
// check. Those pin the heading date by shape, the absence of a `### Breaking`
// subsection on a patch bump and the version by its sites; these pins hold
// the heading to the day the release was cut, the subsection set a purely
// additive patch release carries, the three items `### Added` names, the
// patch digit and semver verdict the version bullet records, the twelve
// release-docs parts the restated include-set bullet lists, the README
// patch-list order with the 0.18.3 block above the 0.18.2 and 0.18.1
// blocks, and the facts about the 0.18.3 section a consumer reads cold: the
// patch-set ledger's 0.18.3 section citing the release patch and the
// adopted probe, the emitted note naming the late-joiner probe, the README
// block's six bullets and consumer note, and the probe's module doc standing
// on the contract alone. The first release heading, the ledger opening, the
// README dependency snippet and the publish contract's hand pins with the
// manifest version belong to the 0.18.4 probe, which shares
// `first_release_heading` and `patchset_opening_section` from here; the
// 0.18.3 hand-pins test went with them, since its line-policy and `VERSION`
// checks would only repeat the 0.18.4 one's.

/// The first `## <digit>` heading line of the CHANGELOG; a `## Unreleased`
/// stub above it is skipped.
fn first_release_heading(changelog: &str) -> Option<&str> {
    changelog
        .lines()
        .find(|line| line.starts_with("## ") && line[3..].starts_with(|c: char| c.is_ascii_digit()))
}

/// The 0.18.3 heading carries the day the release was cut.
#[test]
fn release_0_18_3_heading_carries_the_release_date() {
    let changelog = read_repo_file("crates/libs/rns-transport/CHANGELOG.md");
    assert!(
        changelog.contains("\n## 0.18.3 - 2026-11-19\n"),
        "the CHANGELOG should head the 0.18.3 section with its release date"
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

/// The README patch list runs 0.18.3, then 0.18.2, then 0.18.1: the 0.18.3
/// block sits above the 0.18.2 and 0.18.1 blocks in that order; the 0.18.4
/// block opens the list.
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

/// The first `## <digit>` section of the patch-set ledger, heading line
/// included, up to the next `## ` heading; a `## Unreleased` stub above it
/// is skipped.
fn patchset_opening_section(ledger: &str) -> &str {
    let start = ledger
        .match_indices("\n## ")
        .map(|(at, _)| at + 1)
        .find(|&at| ledger[at + 3..].starts_with(|c: char| c.is_ascii_digit()))
        .expect("the patch-set ledger has a release heading");
    let section = &ledger[start..];
    match section[3..].find("\n## ") {
        Some(end) => &section[..end + 3],
        None => section,
    }
}

/// A reopened `## Unreleased` stub above the release section in either
/// ledger is skipped: the patch-set slice opens on the first `## <digit>`
/// heading and runs to the next heading, and the CHANGELOG first-release
/// filter lands on the dated heading below the stub.
#[test]
fn release_0_18_3_patchset_opening_section_skips_an_unreleased_stub() {
    let ledger = "# Patch set\n\nintro\n\n## Unreleased (upstream 0.14.0)\n\nwip\n\n\
                  ## 0.18.3 (upstream 0.14.0)\n\nbody\n\n## 0.18.2 (upstream 0.14.0)\n\nolder\n";
    let section = patchset_opening_section(ledger);
    assert!(section.starts_with("## 0.18.3 (upstream 0.14.0)\n"), "got {section:?}");
    assert!(section.contains("body"), "the section keeps its own body: {section:?}");
    assert!(!section.contains("wip"), "the stub body is left out: {section:?}");
    assert!(!section.contains("older"), "the next section is left out: {section:?}");

    let changelog =
        "# Changelog\n\n## Unreleased\n\n## 0.18.3 - 2026-11-19\n\n## 0.18.2 - 2026-11-16\n";
    assert_eq!(
        first_release_heading(changelog),
        Some("## 0.18.3 - 2026-11-19"),
        "the first-release filter skips the stub"
    );
}

/// The `## 0.18.3 (upstream 0.14.0)` section of the patch-set ledger cites
/// `a2a548a7`, `27d866af`, `af9d5694` and `9c3cada9` in the id-less
/// `release 0.18.3` legend sentence and in the release paragraph header,
/// closes the release paragraph on the two ledger files, rows the adopted
/// late-joiner probe under `announce emitted` and cites the two probe
/// commits in both the `announce emitted` legend and the O3 patch header.
#[test]
fn release_0_18_3_patchset_opens_on_the_release_patch_and_the_adopted_probe() {
    let ledger = read_repo_file("docs/fork/PATCHSET.md");
    let heading = "\n## 0.18.3 (upstream 0.14.0)\n";
    let start = ledger.find(heading).expect("the patch-set ledger carries the 0.18.3 section") + 1;
    let section = &ledger[start..];
    let section = match section[3..].find("\n## ") {
        Some(end) => &section[..end + 3],
        None => section,
    };
    let prose = joined_prose(section);
    let release_marker = "The `release 0.18.3` patch carries no other id and is fork commits";
    let release_legend_start =
        prose.find(release_marker).expect("the 0.18.3 legend names the id-less release patch");
    let release_legend = &prose[release_legend_start + release_marker.len()..];
    let release_legend = &release_legend[..release_legend.find('.').expect("sentence end")];
    let release_commits = ["`a2a548a7`", "`27d866af`", "`af9d5694`", "`9c3cada9`"];
    for commit in release_commits {
        assert!(
            release_legend.contains(commit),
            "the `release 0.18.3` legend should cite {commit}: {release_legend}"
        );
    }
    let release_label = ": release 0.18.3.";
    let release_header_end =
        prose.find(release_label).expect("the release paragraph header carries its label");
    let release_header_start = prose[..release_header_end]
        .rfind("Fork commits ")
        .expect("the release paragraph header opens on `Fork commits`");
    let release_header = &prose[release_header_start..release_header_end];
    for commit in release_commits {
        assert!(
            release_header.contains(commit),
            "the release paragraph header should cite {commit}: {release_header}"
        );
    }
    assert!(
        prose.contains("touch only this file and the crate `CHANGELOG.md`."),
        "the release paragraph should close on the two ledger files"
    );
    let announce_legend_start = prose
        .find("The `announce emitted` patch is fork commits")
        .expect("the 0.18.3 legend names the `announce emitted` patch");
    let announce_legend = &prose[announce_legend_start..];
    let announce_legend = &announce_legend[..announce_legend.find('.').expect("sentence end")];
    for commit in ["`79cee904`", "`6236b9e1`"] {
        assert!(
            announce_legend.contains(commit),
            "the `announce emitted` legend should cite {commit}: {announce_legend}"
        );
    }
    assert!(
        section.lines().any(|line| {
            line
            == "| `crates/libs/rns-transport/tests/announce_emitted_probe.rs` | announce emitted |"
        }),
        "the 0.18.3 table should row the adopted probe under `announce emitted`"
    );
    let o3_header_start = prose.find("Fork commits `02cc9498`").expect("the O3 patch header");
    let o3_header = &prose[o3_header_start..];
    let o3_header = &o3_header[..o3_header.find("O3, announce emitted.").expect("O3 label")];
    for commit in ["`79cee904`", "`6236b9e1`"] {
        assert!(
            o3_header.contains(commit),
            "the O3 patch header should cite {commit}: {o3_header}"
        );
    }
}

/// The `AnnounceEvent::emitted` bullet of 0.18.3 names the adopted
/// late-joiner probe in its Pinned-by clause.
#[test]
fn release_0_18_3_emitted_bullet_pins_the_late_joiner_probe() {
    let changelog = read_repo_file("crates/libs/rns-transport/CHANGELOG.md");
    let section = changelog_release_section(&changelog, "0.18.3");
    let bullets = subsection_bullets(&section, "Added");
    let emitted = joined_prose(&single_bullet_starting_with(
        &bullets,
        "- `AnnounceEvent::emitted`, `AnnounceEvent::random_blob`",
    ));
    let pinned_by = emitted.find("Pinned by").expect("the emitted bullet has a Pinned-by clause");
    assert!(
        emitted[pinned_by..].contains("`tests/announce_emitted_probe.rs`"),
        "the Pinned-by clause should name the adopted probe: {}",
        &emitted[pinned_by..]
    );
}

/// The README 0.18.3 patch-list block carries one bullet per shipped change
/// (six, the consumer note last) and the consumer note spells out the rnsd
/// path: the `for_instance` endpoint on Linux or Android, the TCP endpoint
/// with the default control port elsewhere, the identity-derived key from
/// `<configdir>/storage/transport_identity`, the `kind` filter and the rule
/// to compare against `emitted`, never arrival; the dependency snippet pins
/// the release.
#[test]
fn release_0_18_3_readme_consumer_note_names_the_rnsd_path_and_the_emitted_rule() {
    let readme = read_repo_file("crates/libs/rns-transport/README.md");
    let marker = "What `0.18.3` carries on top";
    let at = readme.find(marker).expect("the README patch list has the 0.18.3 block");
    let block = &readme[at + marker.len()..];
    let block =
        &block[..block.find("\nWhat `0.18.2` carries on top").expect("the 0.18.2 block follows")];
    assert!(
        joined_prose(block).starts_with("of `0.18.2`:"),
        "the 0.18.3 marker sentence names the 0.18.2 baseline"
    );
    let bullets: Vec<String> =
        top_level_bullets(block).iter().map(|bullet| joined_prose(bullet)).collect();
    assert_eq!(
        bullets.len(),
        6,
        "the 0.18.3 block carries one bullet per shipped change: peer count, rpc client, announce \
         emission, Python proofs, hygiene and the consumer note; got {bullets:#?}"
    );
    let openers = [
        "- The auto peer count:",
        "- The rnsd RPC client:",
        "- Announce emission:",
        "- The Python-in-the-loop proofs:",
        "- Test-only hygiene:",
        "- For a consumer: pin `coyote-rns-transport = \"0.18.3\"`;",
    ];
    for (bullet, opener) in bullets.iter().zip(openers) {
        assert!(bullet.starts_with(opener), "expected a bullet opening {opener:?}, got {bullet:?}");
    }
    let consumer = &bullets[5];
    for needle in [
        "there is nothing to opt into",
        "`Transport::interface_peer_count`",
        "`transport::shared_instance_rpc::interface_stats`",
        "`RpcEndpoint::for_instance(\"default\")`",
        "Linux or Android",
        "`RpcEndpoint::Tcp(127.0.0.1:<instance_control_port>)`",
        "37429",
        "`RpcKey::from_transport_identity_file`",
        "`<configdir>/storage/transport_identity`",
        "`kind == \"AutoInterface\"`",
        "`AnnounceEvent::emitted`",
        "never against the arrival time",
    ] {
        assert!(consumer.contains(needle), "the consumer note should say {needle:?}: {consumer}");
    }
}

/// The adopted late-joiner probe lives in the crate's black-box suite, its
/// module doc rests on the Python citations alone and points at no document
/// outside the repository, and it carries its two tokio tests.
#[test]
fn release_0_18_3_announce_emitted_probe_stands_on_the_contract_alone() {
    let probe = read_repo_file("crates/libs/rns-transport/tests/announce_emitted_probe.rs");
    let module_doc: Vec<&str> = probe.lines().filter(|line| line.starts_with("//!")).collect();
    assert!(!module_doc.is_empty(), "the probe has a module doc");
    assert!(
        !module_doc.iter().any(|line| line.contains("plans/")),
        "the probe's module doc should not reference `plans/`"
    );
    for cite in ["`RNS/Transport.py:3459-3473`", "`RNS/Transport.py:2238`"] {
        assert!(module_doc.iter().any(|line| line.contains(cite)), "the module doc cites {cite}");
    }
    assert_eq!(
        probe.matches("#[tokio::test]").count(),
        2,
        "the adopted probe carries its two late-joiner tests"
    );
}
