// Pins over the 0.18.4 release artefacts beyond what the release contracts
// check. Those pin the heading date by shape, the absence of a `### Breaking`
// subsection on a patch bump and the version by its sites; these pins hold
// the heading to the day the release was cut, the subsection set a purely
// additive patch release carries, the one item `### Added` names, the patch
// digit and semver verdict the version bullet records, the past tense the
// framing bullet keeps for a pin that read the reopened section, the
// thirteen release-docs parts the restated include-set bullet lists, the
// README patch-list order with the 0.18.4 block above the 0.18.3 and 0.18.2
// blocks, and the facts about the 0.18.4 section a consumer reads cold: the
// patch-set ledger's 0.18.4 section citing the four release commits and the
// adopted probe commits, and the README block's four bullets and consumer
// note with the retry rule. Four more usage pins read the release against
// the tree it describes: the stage-boundary clauses of the `### Added`
// bullet and the README rpc section name tests the io kind probe carries,
// the reset pins are kind-based on either variant in the tests and both
// documents, the test-hygiene claims the ledger makes about `a58b664e` hold
// in the tree, and the ledger cites the adopted test commits under `rpc io
// error` with every file they touched rowed there, the io kind probe under
// the release patch as well since a release commit edited it. The first
// release heading, the ledger opening, the README dependency snippet and
// the publish contract's hand pins with the manifest version belong to the
// 0.18.5 probe, which shares `first_release_heading` and
// `patchset_opening_section` from the 0.18.3 probe; the two ledger tests
// here slice their section by its heading instead, and the 0.18.4
// hand-pins test went with them, since its line-policy and `VERSION` checks
// would only repeat the 0.18.5 one's.

/// The 0.18.4 heading carries the day the release was cut.
#[test]
fn release_0_18_4_heading_carries_the_release_date() {
    let changelog = read_repo_file("crates/libs/rns-transport/CHANGELOG.md");
    assert!(
        changelog.contains("\n## 0.18.4 - 2026-11-20\n"),
        "the CHANGELOG should head the 0.18.4 section with its release date"
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
/// gate the release check runs, the probe that pins the release and the
/// usage pins it carries down to the publish contract's hand pins.
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
        "the usage pins a consumer reads cold",
        "two hand pins reading `0.18.4`",
    ] {
        assert!(bullet.contains(needle), "the 0.18.4 version bullet should say {needle:?}");
    }
}

/// The README fork framing bullet of 0.18.4 describes the full-hash ledger
/// pin's cover of the reopened `## Unreleased` section in the past tense,
/// since the release tree carries no such section.
#[test]
fn release_0_18_4_framing_bullet_reads_the_ledger_pin_in_the_past_tense() {
    let changelog = read_repo_file("crates/libs/rns-transport/CHANGELOG.md");
    let section = changelog_release_section(&changelog, "0.18.4");
    let bullets = subsection_bullets(&section, "Changed");
    let bullet = joined_prose(&single_bullet_starting_with(
        &bullets,
        "- README fork framing, docs only, no library change",
    ));
    assert!(
        bullet.contains("covered the reopened `## Unreleased` section"),
        "the framing bullet should read the ledger pin in the past tense: {bullet}"
    );
    assert!(
        !bullet.contains("admits the reopened"),
        "the framing bullet should not read the ledger pin in the present tense: {bullet}"
    );
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

/// The README patch list runs 0.18.4, then 0.18.3, then 0.18.2: the 0.18.4
/// block sits above the 0.18.3 and 0.18.2 blocks in that order; the 0.18.5
/// block opens the list.
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

/// The `## 0.18.4 (upstream 0.14.0)` section of the patch-set ledger, up to
/// the next `## ` heading.
fn patchset_section_0_18_4(ledger: &str) -> &str {
    let heading = "\n## 0.18.4 (upstream 0.14.0)\n";
    let start = ledger.find(heading).expect("the patch-set ledger carries the 0.18.4 section") + 1;
    let section = &ledger[start..];
    match section[3..].find("\n## ") {
        Some(end) => &section[..end + 3],
        None => section,
    }
}

/// The `## 0.18.4 (upstream 0.14.0)` section of the patch-set ledger names
/// the id-less `release 0.18.4` patch in its legend and cites the four
/// release commits both there and in the release paragraph header, the
/// `rpc io error` legend cites the two probe commits adopted since 0.18.3,
/// and the table rows this file under `release 0.18.4`. The citations are
/// `contains` checks so the ledger may cite later commits too; only this
/// probe's own commit cannot cite itself.
#[test]
fn release_0_18_4_patchset_opens_on_the_release_patch() {
    let ledger = read_repo_file("docs/fork/PATCHSET.md");
    let section = patchset_section_0_18_4(&ledger);
    let prose = joined_prose(section);
    let release_marker = "The `release 0.18.4` patch carries no other id and is fork commits";
    let release_legend_start =
        prose.find(release_marker).expect("the 0.18.4 legend names the id-less release patch");
    let release_legend = &prose[release_legend_start + release_marker.len()..];
    let release_legend = &release_legend[..release_legend.find('.').expect("sentence end")];
    let release_commits = ["`acae9a72`", "`58e4d93d`", "`c2630318`", "`e9ccadcd`"];
    for commit in release_commits {
        assert!(
            release_legend.contains(commit),
            "the `release 0.18.4` legend should cite {commit}: {release_legend}"
        );
    }
    let release_label = ": release 0.18.4.";
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
/// backoff, and the one observable change for an existing matcher.
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
}

/// The five stage-boundary pins the 0.18.4 `### Added` bullet and the README
/// rpc section promise exist in `tests/shared_instance_rpc_io_kind_probe.rs`
/// under the classification the prose gives them, and both documents carry
/// the clause for each: a half-written header then silence as `Timeout`, a
/// non-`#WELCOME#` verdict after a correct answer as `Authentication`, an
/// oversized reply header after an honest handshake as `Protocol`, a
/// well-framed reply of the wrong shape as `Decode`, and an honest handshake
/// then a reset instead of a reply as `Io` of a retryable kind.
#[test]
fn release_0_18_4_stage_boundary_pins_match_the_added_bullet_and_the_readme() {
    let changelog = read_repo_file("crates/libs/rns-transport/CHANGELOG.md");
    let section = changelog_release_section(&changelog, "0.18.4");
    let bullets = subsection_bullets(&section, "Added");
    let added =
        joined_prose(&single_bullet_starting_with(&bullets, "- `RpcError::Io(std::io::Error)`"));
    let readme = read_repo_file("crates/libs/rns-transport/README.md");
    let rpc = joined_prose(crate_readme_section(&readme, "Reading rnsd's interface statistics"));
    let probe =
        read_repo_file("crates/libs/rns-transport/tests/shared_instance_rpc_io_kind_probe.rs");
    let clauses_and_pins = [
        (
            "a half-written header then silence as `Timeout`",
            "async fn partial_header_then_silence_is_timeout_without_an_io_kind()",
        ),
        (
            "a non-`#WELCOME#` verdict after a correct answer as `Authentication`",
            "async fn non_welcome_verdict_after_a_correct_answer_is_authentication_without_an_io_kind()",
        ),
        (
            "an oversized reply header after an honest handshake as `Protocol`",
            "async fn honest_handshake_then_oversized_reply_header_is_protocol_without_an_io_kind()",
        ),
        (
            "a well-framed reply of the wrong shape as `Decode`",
            "async fn honest_handshake_then_a_reply_of_the_wrong_shape_is_decode_without_an_io_kind()",
        ),
        (
            "an honest handshake then a reset instead of a reply as `Io` of a retryable kind",
            "async fn honest_handshake_then_reset_instead_of_a_reply_is_io_retryable()",
        ),
    ];
    for (clause, pin) in clauses_and_pins {
        assert!(added.contains(clause), "the 0.18.4 `### Added` bullet should say {clause:?}");
        assert!(rpc.contains(clause), "the README rpc section should say {clause:?}");
        assert!(probe.contains(pin), "the io kind probe should carry {pin:?}");
    }
    assert!(
        added.contains("none with an io kind"),
        "the `### Added` bullet says the four non-io stage boundaries carry no io kind"
    );
    assert!(
        added.contains("`tests/shared_instance_rpc_io_kind_probe.rs`")
            && rpc.contains("`tests/shared_instance_rpc_io_kind_probe.rs`"),
        "both documents name the probe file the stage-boundary clauses describe"
    );
}

/// The two reset pins the ledger says were made kind-based carry their new
/// names and no trace of the old ones, the CHANGELOG `### Added` bullet and
/// the README rpc section describe each as `Io` or `Connect` of a retryable
/// kind with the macOS reason, and neither document still reads the reset
/// pins as `Io` alone.
#[test]
fn release_0_18_4_reset_pins_are_kind_based_on_either_variant_in_tests_and_docs() {
    let changelog = read_repo_file("crates/libs/rns-transport/CHANGELOG.md");
    let section = changelog_release_section(&changelog, "0.18.4");
    let bullets = subsection_bullets(&section, "Added");
    let added =
        joined_prose(&single_bullet_starting_with(&bullets, "- `RpcError::Io(std::io::Error)`"));
    let readme = read_repo_file("crates/libs/rns-transport/README.md");
    let rpc = joined_prose(crate_readme_section(&readme, "Reading rnsd's interface statistics"));
    let io_kind_probe =
        read_repo_file("crates/libs/rns-transport/tests/shared_instance_rpc_io_kind_probe.rs");
    let api = read_repo_file("crates/libs/rns-transport/tests/shared_instance_rpc_api.rs");
    for (file, text, new_name, old_name) in [
        (
            "shared_instance_rpc_io_kind_probe.rs",
            &io_kind_probe,
            "async fn accept_then_reset_before_any_byte_is_a_retryable_kind_on_either_variant()",
            "accept_then_reset_before_any_byte_is_io_retryable",
        ),
        (
            "shared_instance_rpc_api.rs",
            &api,
            "async fn server_that_resets_after_a_whole_challenge_is_a_retryable_kind_on_either_variant()",
            "server_that_resets_after_a_whole_challenge_is_io_of_a_retryable_kind",
        ),
    ] {
        assert!(text.contains(new_name), "{file} should carry the renamed pin {new_name:?}");
        assert!(!text.contains(old_name), "{file} should no longer carry the old pin name {old_name:?}");
    }
    for clause in [
        "an accept-then-RST before any byte as `Io` or `Connect` of a retryable kind",
        "resets after a whole challenge as `Io` or `Connect` of a retryable kind",
        "on macOS the reset can land before the dial's `connect()` returns",
    ] {
        assert!(added.contains(clause), "the 0.18.4 `### Added` bullet should say {clause:?}");
    }
    for clause in [
        "an accept-then-RST before any byte as `Io` or `Connect` of a retryable kind",
        "reset-after-the-challenge loopback servers (the last `Io` or `Connect` of a retryable kind",
        "on macOS the reset can land before the dial's `connect()` returns",
    ] {
        assert!(rpc.contains(clause), "the README rpc section should say {clause:?}");
    }
    for stale in ["an accept-then-RST before any byte as `Io`;", "reset after a whole challenge),"]
    {
        assert!(
            !added.contains(stale),
            "the `### Added` bullet should not keep the `Io`-only reading {stale:?}"
        );
    }
    assert!(
        !rpc.contains("an accept-then-RST before any byte;"),
        "the README rpc section should not keep the kind-less accept-then-RST clause"
    );
}

/// The test-hygiene claims the 0.18.4 ledger makes about `a58b664e` hold in
/// the tree: `assert_running` on the hil rnsd fixture is `#[cfg(unix)]`, the
/// `Script::Close` doc says why the script does not drain, and both CPython
/// reset pins carry the POSIX `struct linger` layout clause.
#[test]
fn release_0_18_4_ledger_hygiene_claims_hold_in_the_tree() {
    let fixture = read_repo_file(
        "crates/apps/reticulumd/tests/python_channel_interop_parts/python_shared_instance_rpc.rs",
    );
    assert!(
        fixture.contains("    #[cfg(unix)]\n    fn assert_running("),
        "`assert_running` on the hil rnsd fixture should be gated `#[cfg(unix)]` like its callers"
    );
    let mock = read_repo_file(
        "crates/libs/rns-transport/src/transport/shared_instance_rpc/tests/mock_server.rs",
    );
    let close_doc =
        mock[..mock.find("\n    Close,\n").expect("`Script::Close` exists")].to_string();
    let close_doc: String = close_doc
        [close_doc.rfind("\n    Silent,\n").expect("`Script::Silent` precedes `Close`")..]
        .lines()
        .filter_map(|line| line.trim().strip_prefix("///"))
        .collect::<Vec<_>>()
        .join(" ");
    let close_doc = joined_prose(&close_doc);
    assert!(
        close_doc.contains("no drain, since the client never writes before its first read"),
        "the `Script::Close` doc should say why the script does not drain: {close_doc}"
    );
    let python_probe = read_repo_file(
        "crates/libs/rns-transport/tests/shared_instance_rpc_departed_server_python_probe.rs",
    );
    assert_eq!(
        python_probe.matches("`struct linger` layout").count(),
        2,
        "both CPython reset pins carry the POSIX `struct linger` layout clause"
    );
}

/// The two test-adoption commits the release took before its promotion
/// commits (`9a0eb46a`, the five stage-boundary probes, and `a58b664e`, the
/// kind-based reset pins) are cited under the `rpc io error` patch of the
/// `## 0.18.4` ledger, both in the legend and in the patch paragraph, and
/// every test file they touched is rowed under that patch: three
/// rns-transport test files under it alone, the io kind probe under it and
/// `release 0.18.4` since a release commit edited that file too, and the
/// hil rnsd fixture under it and the Python interop patch. Each row lists
/// the union of touching patches in legend order.
#[test]
fn release_0_18_4_ledger_cites_the_adopted_test_commits_under_the_rpc_io_error_patch() {
    let ledger = read_repo_file("docs/fork/PATCHSET.md");
    let section = patchset_section_0_18_4(&ledger);
    let prose = joined_prose(section);
    let legend_start = prose.find("The `rpc io error` patch is fork commits").expect("legend");
    let legend = &prose[legend_start..];
    let legend = &legend[..legend.find(". ").expect("the legend sentence ends")];
    let header_end = prose.find(": P2, rpc io error.").expect("the rpc io error patch header");
    let header_start = prose[..header_end].rfind("Fork commits `").expect("the header opens");
    let header = &prose[header_start..header_end];
    for sha in ["9a0eb46a", "a58b664e"] {
        assert!(legend.contains(&format!("`{sha}`")), "the legend cites {sha} under rpc io error");
        assert!(
            header.contains(&format!("`{sha}`")),
            "the rpc io error patch header cites {sha}: {header}"
        );
    }
    for file in [
        "crates/libs/rns-transport/tests/shared_instance_rpc_api.rs",
        "crates/libs/rns-transport/tests/shared_instance_rpc_departed_server_python_probe.rs",
        "crates/libs/rns-transport/src/transport/shared_instance_rpc/tests/mock_server.rs",
    ] {
        let row = section
            .lines()
            .find(|line| line.starts_with(&format!("| `{file}` |")))
            .unwrap_or_else(|| panic!("{file} is rowed in the 0.18.4 table"));
        assert_eq!(
            row,
            format!("| `{file}` | rpc io error |"),
            "{file} is rowed under the rpc io error patch alone"
        );
    }
    let io_kind_probe = "crates/libs/rns-transport/tests/shared_instance_rpc_io_kind_probe.rs";
    let row = section
        .lines()
        .find(|line| line.starts_with(&format!("| `{io_kind_probe}` |")))
        .expect("the io kind probe is rowed in the 0.18.4 table");
    assert_eq!(
        row,
        format!("| `{io_kind_probe}` | rpc io error, release 0.18.4 |"),
        "the io kind probe is rowed under rpc io error and the release patch"
    );
    let fixture =
        "crates/apps/reticulumd/tests/python_channel_interop_parts/python_shared_instance_rpc.rs";
    let row = section
        .lines()
        .find(|line| line.starts_with(&format!("| `{fixture}` |")))
        .expect("the hil fixture a58b664e touched is rowed in the 0.18.4 table");
    assert_eq!(
        row,
        format!("| `{fixture}` | rpc io error, Python-in-the-loop rpc io error interop |"),
        "the hil fixture is rowed under rpc io error and the Python interop patch"
    );
}
