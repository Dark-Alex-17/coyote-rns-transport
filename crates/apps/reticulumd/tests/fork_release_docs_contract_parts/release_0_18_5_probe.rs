// Pins over the 0.18.5 release artefacts beyond what the release contracts
// check. Those pin the heading date by shape, the absence of a `### Breaking`
// subsection on a patch bump and the version by its sites; these pins hold
// the heading to the day the release was cut and to the top of the ledger,
// the subsection set an additive patch release with one fix carries, the
// four items `### Added` names, the patch digit and semver verdict the
// version bullet records, the `### Fixed` bullet's opener, the past tense
// the ledger-pin bullet keeps for a pin that read the reopened section, the
// fourteen release-docs parts the restated include-set bullet lists, the
// README patch-list order with the 0.18.5 block on top, and the usage pins
// a consumer reads cold: the patch-set ledger opening on this release with
// the id-less release patch in its legend citing the release commits both
// there and in its paragraph header and this file rowed under it, the
// README block's six bullets and consumer note with the recovery recipe,
// and the publish contract's two hand pins reading the release version. Two
// more usage pins read the release against the tree it describes: the
// README block, CHANGELOG section and ledger section are ASCII with no plan
// or task id, and the test-hygiene claims the three documents make hold in
// the tree. One more holds the carried doc nits to the patches they belong
// to: `dfb85779` under rnsd path rpc and path interface, `91c32571` under
// link establishment timeout, in legend and header, and neither under the
// release patch. Two more read the release against the tree and the
// history it ships with: every call, record, attribute and file the README
// block names resolves in the crate, and the ledger section cites exactly
// the commits of the release's own range, with the id-less release patch
// citing no ledger-only commit. The pins over the current tree, which the
// next release cut demotes, are the first-release-heading test, the
// ledger-opening assertion of the patch-set test, the README
// `[dependencies]` snippet pins of the consumer-note test, the
// crate-manifest version and publish-contract literal count of the
// hand-pins test, and the range test, whose range runs from the previous
// tag to the release tag once that is reachable and to HEAD before; the
// rest hold facts about the 0.18.5 section itself. The ledger pins other
// than the opening one slice the 0.18.5 section by its heading, so a
// reopened `## Unreleased` stub above it leaves them green.
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

/// The 0.18.5 ledger-pin bullet under `### Changed` reads in the past tense:
/// the full-hash pin covered the reopened `## Unreleased` section while the
/// work was in progress and no longer extends it, since the release tree
/// carries no such section.
#[test]
fn release_0_18_5_ledger_pin_bullet_reads_in_the_past_tense() {
    let changelog = read_repo_file("crates/libs/rns-transport/CHANGELOG.md");
    let section = changelog_release_section(&changelog, "0.18.5");
    let bullets: Vec<String> =
        subsection_bullets(&section, "Changed").iter().map(|bullet| joined_prose(bullet)).collect();
    let bullet = single_bullet_starting_with(&bullets, "- Test-only: the full-hash ledger pin in");
    assert!(
        bullet.contains("covered the reopened `## Unreleased` section"),
        "the ledger-pin bullet should read in the past tense: {bullet}"
    );
    assert!(
        !bullet.contains("extends the full-hash ledger pin"),
        "the ledger-pin bullet should not read in the present tense: {bullet}"
    );
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
/// `release 0.18.5` patch and cites the release commits both there and in
/// the release paragraph header, and the table rows this file under
/// `release 0.18.5`. The citations are `contains` checks so the ledger may
/// cite later commits too; the only commits this pin cannot name are the
/// ones that touch this file in the same commit.
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
    let release_marker = "The `release 0.18.5` patch carries no other id and is fork commits";
    let release_legend_start =
        prose.find(release_marker).expect("the 0.18.5 legend names the id-less release patch");
    let release_legend = &prose[release_legend_start + release_marker.len()..];
    let release_legend = &release_legend[..release_legend.find('.').expect("sentence end")];
    let release_commits = ["`8679349e`", "`12239075`", "`80e0cb4d`", "`728464e5`"];
    for commit in release_commits {
        assert!(
            release_legend.contains(commit),
            "the `release 0.18.5` legend should cite {commit}: {release_legend}"
        );
    }
    let release_label = ": release 0.18.5.";
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
        section.lines().any(|line| {
            line == "| `crates/apps/reticulumd/tests/fork_release_docs_contract_parts/release_0_18_5_probe.rs` | release 0.18.5 |"
        }),
        "the 0.18.5 table should row this probe under `release 0.18.5`"
    );
}

/// The carried doc nits are ledgered under the patches they belong to and
/// not under the release patch: `dfb85779` is cited by the rnsd path rpc
/// and path interface legend sentences and patch headers, `91c32571` by the
/// link establishment timeout legend sentence and patch header, and the
/// release paragraph header cites neither.
#[test]
fn release_0_18_5_ledger_cites_the_carried_nits_under_their_own_patches() {
    let ledger = read_repo_file("docs/fork/PATCHSET.md");
    let prose = joined_prose(patchset_0_18_5_section(&ledger));
    let header_before = |label: &str| {
        let end = prose.find(label).unwrap_or_else(|| panic!("a patch header ends with {label:?}"));
        let start =
            prose[..end].rfind("Fork commits `").expect("the patch header opens on `Fork commits`");
        &prose[start..end]
    };
    for (patch, label, sha) in [
        ("rnsd path rpc", ": Q1, rnsd path rpc.", "`dfb85779`"),
        ("path interface", ": Q2, path interface.", "`dfb85779`"),
        ("link establishment timeout", ": Q3, link establishment timeout.", "`91c32571`"),
    ] {
        let opener = format!("The `{patch}` patch is fork commits");
        let legend_start = prose
            .find(&opener)
            .unwrap_or_else(|| panic!("the 0.18.5 legend names the `{patch}` patch"));
        let legend = &prose[legend_start + opener.len()..];
        let legend = &legend[..legend.find('.').expect("the legend sentence ends")];
        assert!(legend.contains(sha), "the `{patch}` legend cites {sha}: {legend}");
        let header = header_before(label);
        assert!(header.contains(sha), "the `{patch}` patch header cites {sha}: {header}");
    }
    let release_header = header_before(": release 0.18.5.");
    for sha in ["`dfb85779`", "`91c32571`"] {
        assert!(
            !release_header.contains(sha),
            "the release paragraph header should not cite the carried nit {sha}: {release_header}"
        );
    }
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

/// The 0.18.5 README block, CHANGELOG section and patch-set section are
/// plain ASCII and carry no plan or task id: a consumer reads the release
/// artefacts without the planning vocabulary the commit messages may carry.
#[test]
fn release_0_18_5_artefacts_are_ascii_and_carry_no_plan_ids() {
    let readme = read_repo_file("crates/libs/rns-transport/README.md");
    let block = readme_0_18_5_block(&readme);
    let changelog = read_repo_file("crates/libs/rns-transport/CHANGELOG.md");
    let section = changelog_release_section(&changelog, "0.18.5");
    let ledger = read_repo_file("docs/fork/PATCHSET.md");
    let patchset = patchset_0_18_5_section(&ledger);
    let plan_ids = [concat!("TASK", "-"), concat!("PLAN", "-")];
    for (name, text) in [
        ("README 0.18.5 block", block),
        ("CHANGELOG 0.18.5 section", section.as_str()),
        ("PATCHSET 0.18.5 section", patchset),
    ] {
        let non_ascii: Vec<(usize, &str)> = text
            .lines()
            .enumerate()
            .filter(|(_, line)| !line.is_ascii())
            .map(|(index, line)| (index + 1, line))
            .collect();
        assert!(non_ascii.is_empty(), "{name} should be ASCII, got {non_ascii:?}");
        for id in plan_ids {
            let hits: Vec<&str> = text.lines().filter(|line| line.contains(id)).collect();
            assert!(hits.is_empty(), "{name} should carry no `{id}` ids, got {hits:?}");
        }
    }
}

/// The test-only hygiene claims the 0.18.5 README bullet, CHANGELOG bullet
/// and ledger narrative make hold in the tree: the four shared support
/// files exist, the mock peer serves a sequence and aborts on drop, the
/// paths probe (not the api file) carries the abort-on-drop guard the
/// CHANGELOG credits to it, the harness and support helpers exist under the
/// names the docs give them, `path_row.rs` has replaced `path_table.rs`, the
/// vanished-server case is gated off Windows with the Winsock rationale, the
/// docs-pin header admits the read-by-field rows, the restored-row-age note
/// is platform-neutral, and the ledger keeps the 400 ms ceiling with
/// `7edc5c91` and the 700 ms budget with `8bc21180`.
#[test]
fn release_0_18_5_hygiene_claims_hold_in_the_tree() {
    for support in
        ["stdlib_rpc_handshake.rs", "stdlib_rpc_frame.rs", "msgpack_value.rs", "every_rpc_error.rs"]
    {
        let path = format!("crates/libs/rns-transport/tests/support/{support}");
        assert!(repo_root().join(&path).is_file(), "{path} should exist");
    }
    let rpc_dir = repo_root().join("crates/libs/rns-transport/src/transport/shared_instance_rpc");
    assert!(rpc_dir.join("path_row.rs").is_file(), "`path_row.rs` should exist");
    assert!(!rpc_dir.join("path_table.rs").exists(), "`path_table.rs` should be gone");
    let mock = read_repo_file(
        "crates/libs/rns-transport/src/transport/shared_instance_rpc/tests/mock_server.rs",
    );
    for needle in ["fn spawn_sequence(", "fn finish_all(", "impl Drop for MockServer"] {
        assert!(mock.contains(needle), "the mock rnsd peer should carry {needle:?}");
    }
    let paths_probe =
        read_repo_file("crates/libs/rns-transport/tests/shared_instance_rpc_paths_probe.rs");
    let api = read_repo_file("crates/libs/rns-transport/tests/shared_instance_rpc_api.rs");
    assert!(
        paths_probe.contains("impl<T> Drop for AbortOnDrop<T>"),
        "the paths probe aborts its canned servers on drop"
    );
    assert!(!api.contains("AbortOnDrop"), "the api file carries no abort-on-drop guard of its own");
    let changelog = read_repo_file("crates/libs/rns-transport/CHANGELOG.md");
    let section = joined_prose(&changelog_release_section(&changelog, "0.18.5"));
    assert!(
        section.contains(
            "`tests/shared_instance_rpc_paths_probe.rs` aborts its canned servers on drop"
        ),
        "the CHANGELOG hygiene bullet credits the abort-on-drop guard to the paths probe"
    );
    assert!(
        !section.contains("`tests/shared_instance_rpc_api.rs` aborts its canned servers on drop"),
        "the CHANGELOG hygiene bullet no longer credits the guard to the api file"
    );
    let vanished =
        "async fn drop_path_settled_whose_server_vanishes_mid_poll_never_reports_settled()";
    let at = api.find(vanished).expect("the vanished-server case exists");
    let preamble_start =
        api[..at].rfind("\n\n").expect("a blank line precedes the vanished-server case");
    let preamble = &api[preamble_start..at];
    assert!(
        preamble.contains("#[cfg(not(windows))]\n#[tokio::test]\n"),
        "the vanished-server case is gated off Windows: {preamble}"
    );
    assert!(preamble.contains("Winsock"), "the Windows gate carries the Winsock rationale");
    for (path, needle) in [
        (
            "crates/apps/reticulumd/tests/python_channel_interop_parts/python_shared_instance_rpc.rs",
            "fn spawn_with_interfaces(",
        ),
        (
            "crates/apps/reticulumd/tests/python_channel_interop_parts/python_shared_instance_live.rs",
            "fn with_transport(",
        ),
        ("crates/apps/reticulumd/tests/support/python_channel_events.rs", "fn wait_for_out_link_closed("),
        ("crates/apps/reticulumd/tests/support/python_channel_process.rs", "fn terminate_child_gracefully("),
        (
            "crates/apps/reticulumd/tests/fork_release_docs_contract_parts/request_layer_api_docs.rs",
            "read-by-field rows",
        ),
        (
            "crates/libs/rns-transport/src/transport/tests_parts/path_interface_api.rs",
            "boot-relative on every platform",
        ),
    ] {
        assert!(read_repo_file(path).contains(needle), "{path} should carry {needle:?}");
    }
    let ledger = joined_prose(patchset_0_18_5_section(&read_repo_file("docs/fork/PATCHSET.md")));
    let support_move = ledger
        .find("`7edc5c91` moves the private copies")
        .expect("the Q1 narrative credits the support-file move to `7edc5c91`");
    let support_move = &ledger[support_move..];
    let support_move =
        &support_move[..support_move.find("`1867e6e4`").expect("the next hygiene commit follows")];
    assert!(
        support_move.contains("under a 400 ms ceiling, caps the silent-server case at 1200 ms"),
        "the ledger keeps the 400 ms ceiling with `7edc5c91`: {support_move}"
    );
    assert!(
        ledger.contains("`8bc21180` bounds the vanished-server case by its 700 ms budget instead of a 400 ms ceiling"),
        "the ledger keeps the 700 ms budget with `8bc21180`"
    );
}

/// `git merge-base --is-ancestor <commit> <of>` from the repository root:
/// `Some(true)` when `commit` is reachable from `of`, `Some(false)` when it
/// is not, `None` when either name does not resolve.
fn is_ancestor_of(commit: &str, of: &str) -> Option<bool> {
    let status = std::process::Command::new("git")
        .args(["merge-base", "--is-ancestor", commit, of])
        .current_dir(repo_root())
        .status()
        .expect("git merge-base should run");
    match status.code() {
        Some(0) => Some(true),
        Some(1) => Some(false),
        _ => None,
    }
}

/// The lines `git <args>` prints from the repository root.
fn git_lines(args: &[&str]) -> Vec<String> {
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(repo_root())
        .output()
        .expect("git should run");
    assert!(output.status.success(), "git {args:?} should succeed");
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}

/// The distinct backticked eight-hex-digit tokens in `text`, in order of
/// first appearance: the short commit hashes a ledger section cites.
fn cited_short_hashes(text: &str) -> Vec<&str> {
    let mut hashes: Vec<&str> = Vec::new();
    let mut rest = text;
    while let Some(open) = rest.find('`') {
        let after = &rest[open + 1..];
        let Some(close) = after.find('`') else { break };
        let token = &after[..close];
        if token.len() == 8
            && token.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
            && !hashes.contains(&token)
        {
            hashes.push(token);
        }
        rest = &after[close + 1..];
    }
    hashes
}

/// The distinct backticked `.rs` / `.py` paths in `text`, in order of first
/// appearance; a `RNS/<file>.py:<line>` cite ends in digits and is not one.
fn cited_source_paths(text: &str) -> Vec<&str> {
    let mut cited: Vec<&str> = Vec::new();
    let mut rest = text;
    while let Some(open) = rest.find('`') {
        let after = &rest[open + 1..];
        let Some(close) = after.find('`') else { break };
        let token = &after[..close];
        let is_path = (token.ends_with(".rs") || token.ends_with(".py"))
            && token.chars().all(|c| c.is_ascii_alphanumeric() || "/_.-".contains(c));
        if is_path && !cited.contains(&token) {
            cited.push(token);
        }
        rest = &after[close + 1..];
    }
    cited
}

/// Every `.rs` / `.py` file under `dir`, recursively, as a repo-relative
/// forward-slash path, skipping any `target` directory.
fn source_paths_under(root: &std::path::Path, dir: &std::path::Path, into: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|name| name == "target") {
                continue;
            }
            source_paths_under(root, &path, into);
        } else if path.extension().is_some_and(|ext| ext == "rs" || ext == "py") {
            let relative = path.strip_prefix(root).unwrap_or(&path);
            into.push(relative.to_string_lossy().replace('\\', "/"));
        }
    }
}

/// The README 0.18.5 block, from its marker sentence to the 0.18.4 block.
fn readme_0_18_5_block(readme: &str) -> &str {
    let marker = "What `0.18.5` carries on top";
    let at = readme.find(marker).expect("the README patch list has the 0.18.5 block");
    let block = &readme[at..];
    &block[..block.find("\nWhat `0.18.4` carries on top").expect("the 0.18.4 block follows")]
}

/// The `## 0.18.5 (upstream 0.14.0)` section of the patch-set ledger,
/// heading line included, up to the next `## ` heading, wherever the
/// section sits below a reopened `## Unreleased` stub.
fn patchset_0_18_5_section(ledger: &str) -> &str {
    let heading = "\n## 0.18.5 (upstream 0.14.0)\n";
    let start = ledger.find(heading).expect("the patch-set ledger has the 0.18.5 section") + 1;
    let section = &ledger[start..];
    match section[3..].find("\n## ") {
        Some(end) => &section[..end + 3],
        None => section,
    }
}

/// Every public item the README 0.18.5 block tells a consumer to call or
/// read is declared `pub` in the crate under the name and file the block
/// gives, the two decoded records carry `#[non_exhaustive]` as the block
/// says, and every backticked `.rs` path the block cites names a file under
/// the crate tree or the reticulumd tests: the block a consumer reads cold
/// points at code that exists.
#[test]
fn release_0_18_5_readme_block_claims_resolve_in_the_tree() {
    let readme = read_repo_file("crates/libs/rns-transport/README.md");
    let block = readme_0_18_5_block(&readme);
    // The block wraps its signatures across lines; read it joined.
    let prose = joined_prose(block);
    let rpc = "crates/libs/rns-transport/src/transport/shared_instance_rpc";
    let paths = read_repo_file(&format!("{rpc}/paths.rs"));
    for call in [
        "path_table",
        "next_hop_interface_name",
        "drop_path",
        "drop_all_via",
        "first_hop_timeout",
        "drop_path_settled",
    ] {
        assert!(prose.contains(&format!("`{call}`")), "the README block names `{call}`");
        assert!(
            paths.contains(&format!("\npub async fn {call}(")),
            "`{call}` should be a public async fn of shared_instance_rpc/paths.rs"
        );
    }
    let rpc_root = read_repo_file("crates/libs/rns-transport/src/transport/shared_instance_rpc.rs");
    assert!(prose.contains("`RpcError::is_transient() -> bool`"), "the block names the predicate");
    assert!(
        rpc_root.contains("pub fn is_transient(&self) -> bool"),
        "`RpcError::is_transient` should be a public method returning bool"
    );
    let path_interface = read_repo_file(
        "crates/libs/rns-transport/src/transport/links_parts/transport_sections/path_interface.rs",
    );
    assert!(
        prose.contains("`Transport::path_interface(&AddressHash) -> Option<PathIface>`"),
        "the block names the in-process read"
    );
    assert!(
        path_interface.contains(
            "pub async fn path_interface(&self, destination: &AddressHash) -> Option<PathIface>"
        ),
        "`Transport::path_interface` should be a public method of the shape the README gives"
    );
    let path_row = read_repo_file(&format!("{rpc}/path_row.rs"));
    for (what, source, item) in [
        ("PathIface", &path_interface, "pub struct PathIface"),
        ("PathRow", &path_row, "pub struct PathRow"),
    ] {
        assert!(prose.contains(&format!("`{what}`")), "the README block names `{what}`");
        let at = source.find(item).unwrap_or_else(|| panic!("{item} should be declared"));
        assert!(
            attribute_block_above(source, at).contains("#[non_exhaustive]"),
            "`{what}` should carry `#[non_exhaustive]` as the README block says"
        );
    }
    let events =
        read_repo_file("crates/libs/rns-transport/src/transport/core_parts/interface_events.rs");
    assert!(prose.contains("`Transport::interface_events()`"), "the block names the stream");
    assert!(
        events.contains("pub fn interface_events(&self)"),
        "`Transport::interface_events` should be a public method"
    );

    let root = repo_root();
    let mut tree: Vec<String> = Vec::new();
    for subtree in ["crates/libs/rns-transport", "crates/apps/reticulumd/tests"] {
        source_paths_under(&root, &root.join(subtree), &mut tree);
    }
    let cited = cited_source_paths(block);
    assert!(cited.len() >= 10, "the README 0.18.5 block cites its pin files, found {cited:?}");
    let unresolved: Vec<&str> = cited
        .iter()
        .copied()
        .filter(|token| {
            !tree.iter().any(|path| path == token || path.ends_with(&format!("/{token}")))
        })
        .collect();
    assert!(
        unresolved.is_empty(),
        "every file the README 0.18.5 block cites should exist under the crate or the reticulumd \
         tests:\n{}",
        unresolved.join("\n")
    );
}

/// The 0.18.5 patch-set section cites exactly the commits of its range:
/// every short hash in its legend and patch headers is a commit reachable
/// from the `coyote-rns-transport-v0.18.5` tag when that is reachable from
/// HEAD, else from HEAD, and not from the `coyote-rns-transport-v0.18.4`
/// tag; every commit in that range that touched a file other than the two
/// ledgers (`CHANGELOG.md`, `PATCHSET.md`) is cited at least once, and the
/// id-less `release 0.18.5` patch cites no ledger-only commit, as the
/// 0.18.4 cut did not. Skips, in so many words, on a checkout without the
/// previous tag.
#[test]
fn release_0_18_5_ledger_cites_exactly_the_commits_of_its_range() {
    let previous = "coyote-rns-transport-v0.18.4";
    let release = "coyote-rns-transport-v0.18.5";
    if is_ancestor_of(previous, "HEAD") != Some(true) {
        eprintln!("skipping: tag {previous} is not reachable from HEAD in this checkout");
        return;
    }
    // Once the release tag is reachable the range closes on it: later
    // commits belong to the next section. Before the tag exists the tree is
    // the release in progress and HEAD closes the range.
    let head = if is_ancestor_of(release, "HEAD") == Some(true) { release } else { "HEAD" };
    let ledger = read_repo_file("docs/fork/PATCHSET.md");
    let section = patchset_0_18_5_section(&ledger);
    let hashes = cited_short_hashes(section);
    assert!(
        hashes.len() >= 40,
        "the 0.18.5 section cites the Q1, Q2, Q3, interop and release commits, found {hashes:?}"
    );
    let mut wrong: Vec<String> = Vec::new();
    for hash in &hashes {
        match (is_ancestor_of(hash, head), is_ancestor_of(hash, previous)) {
            (Some(true), Some(false)) => {}
            (None, _) | (_, None) => wrong.push(format!("`{hash}` does not resolve to a commit")),
            (Some(false), _) => wrong.push(format!("`{hash}` is not reachable from {head}")),
            (_, Some(true)) => wrong.push(format!("`{hash}` was already released in {previous}")),
        }
    }
    assert!(
        wrong.is_empty(),
        "the 0.18.5 ledger should cite only {previous}..{head} commits:\n{}",
        wrong.join("\n")
    );

    let ledgers = ["crates/libs/rns-transport/CHANGELOG.md", "docs/fork/PATCHSET.md"];
    let ledger_only = |hash: &str| {
        git_lines(&["show", "--name-only", "--format=", hash])
            .iter()
            .all(|file| ledgers.contains(&file.as_str()))
    };
    let range = format!("{previous}..{head}");
    let uncited: Vec<String> = git_lines(&["rev-list", "--abbrev-commit", "--abbrev=8", &range])
        .into_iter()
        .filter(|hash| !ledger_only(hash) && !hashes.contains(&hash.as_str()))
        .collect();
    assert!(
        uncited.is_empty(),
        "every {range} commit that touched more than the ledgers should be cited by the 0.18.5 \
         section:\n{}",
        uncited.join("\n")
    );

    let release_marker = "The `release 0.18.5` patch carries no other id and is fork commits";
    let prose = joined_prose(section);
    let legend =
        &prose[prose.find(release_marker).expect("the release legend") + release_marker.len()..];
    let legend = &legend[..legend.find('.').expect("the legend sentence ends")];
    let release_hashes = cited_short_hashes(legend);
    assert!(release_hashes.len() >= 3, "the release legend cites its commits: {legend}");
    for hash in release_hashes {
        assert!(
            !ledger_only(hash),
            "the `release 0.18.5` patch cites `{hash}`, a ledger-only commit the 0.18.4 precedent \
             leaves out of the patch headers"
        );
    }
}
