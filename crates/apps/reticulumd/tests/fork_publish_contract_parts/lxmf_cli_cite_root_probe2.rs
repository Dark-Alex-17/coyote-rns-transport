// Spec-first pins for the fifteenth python-remote-relay patch of 0.16.0, the
// one that adopts the two probe fixtures of the thirteenth's fix: the
// CHANGELOG bullet now cites the helper's home (`status_hash.rs`), the
// config writer (`module_prelude.rs`) and both probe files in a `Pinned by`
// clause, and every one of those cites resolves; the ledger legend and
// header carry the adoption commit next to the two fix commits, the
// fifteenth narrative names both probe files, and the table has a row for
// each; each probe part is included exactly once from its test root and
// still defines the tests the ledger vouches for; and the parity-matrix row
// and evidence section that describe the two-relay shape cite files that
// exist. Each test fails against the tree as it was before the adoption.

const REMOTE_RELAY_PROBE_FILES: [&str; 2] = [
    "crates/apps/reticulumd/tests/fork_publish_contract_parts/lxmf_cli_cite_root_probe.rs",
    "crates/apps/lxmf-cli/tests/python_lxmd_remote_relay_parts/shared_instance_local_client_count_probe.rs",
];

/// The 0.16.0 bullet for the two-peer relay restructure, raw.
fn remote_relay_bullet_0_16_0() -> String {
    let section = changelog_0_16_0_section();
    section
        .split("\n- ")
        .find(|bullet| {
            bullet.contains("python_shared_instance_two_peer_relay_recovers_after_daemon_restart_e2e")
        })
        .expect("the 0.16.0 notes carry a bullet for the two-peer relay test restructure")
        .to_string()
}

/// The restructure bullet cites the helper's home, the config writer and
/// both adopted probes, the probes inside a `Pinned by` clause, and every
/// cite in the bullet resolves under the widened gate.
#[test]
fn changelog_0_16_0_remote_relay_bullet_cites_the_helper_home_the_writer_and_both_probes() {
    let bullet = remote_relay_bullet_0_16_0();
    let cited = cited_source_files(&bullet);
    for expected in [
        "crates/apps/lxmf-cli/tests/support/python_lxmd_remote_relay_parts/status_hash.rs",
        "crates/apps/lxmf-cli/tests/support/python_lxmd_remote_relay_parts/module_prelude.rs",
    ] {
        assert!(cited.contains(&expected), "the bullet cites {expected}, got {cited:?}");
    }
    let pinned_by = bullet
        .rfind("Pinned by")
        .map(|at| &bullet[at..])
        .expect("the bullet ends in a `Pinned by` clause naming the probe fixtures");
    for probe in REMOTE_RELAY_PROBE_FILES {
        assert!(cited.contains(&probe), "the bullet cites {probe}, got {cited:?}");
        assert!(
            pinned_by.contains(&format!("`{probe}`")),
            "the `Pinned by` clause names {probe}, got {pinned_by:?}"
        );
        assert!(repo_root().join(probe).is_file(), "probe premise: {probe} is in the tree");
    }
    let unresolved = unresolved_source_cites(&cited);
    assert!(unresolved.is_empty(), "every cite in the bullet resolves, got {unresolved:?}");
    assert!(
        bullet.contains("`wait_for_python_local_client_count`"),
        "the bullet names the helper the router-restart test waits through"
    );
}

/// The 0.16.0 ledger carries the adoption commit next to the two fix
/// commits in its legend and header, tables a row for each probe file under
/// the release patch, and the fifteenth narrative names both probe files.
#[test]
fn ledger_0_16_0_carries_the_probe_adoption_commit_rows_and_narrative() {
    let ledger = read_repo_file("docs/fork/PATCHSET.md");
    let section = ledger_section(&ledger, "0.16.0").expect("the 0.16.0 ledger section");
    let legend = ledger_legend("0.16.0", section);
    let header = ledger_header_hashes(section);
    for hash in ["7b168319", "00c419a0", "43e30bdc"] {
        assert!(legend.contains(&format!("`{hash}`")), "the legend names {hash}");
        assert!(header.contains(&hash), "the header names {hash}, got {header:?}");
    }
    for probe in REMOTE_RELAY_PROBE_FILES {
        let row = section
            .lines()
            .find(|line| line.starts_with(&format!("| `{probe}` |")))
            .unwrap_or_else(|| panic!("the 0.16.0 table has a row for {probe}"));
        assert!(row.contains("release 0.16.0"), "the {probe} row is attributed to the release, got {row:?}");
    }
    let narrative = section
        .find("The fifteenth adopts")
        .map(|at| &section[at..])
        .expect("the python-remote-relay patch narrative has a fifteenth sentence for the adoption");
    let narrative = narrative.split("\n\n").next().unwrap_or(narrative);
    for probe in REMOTE_RELAY_PROBE_FILES {
        assert!(narrative.contains(&format!("`{probe}`")), "the fifteenth names {probe}");
    }
}

/// Each probe part is included exactly once from its test root, after the
/// part it pins, and defines the tests the ledger and release notes vouch
/// for; the ignored probe keeps its reason so a plain `cargo test` run does
/// not spawn Python.
#[test]
fn probe_parts_are_included_once_from_their_roots_and_define_their_tests() {
    let publish_root = read_repo_file("crates/apps/reticulumd/tests/fork_publish_contract.rs");
    let cite_include = "include!(\"fork_publish_contract_parts/lxmf_cli_cite_root_probe.rs\");";
    assert_eq!(publish_root.matches(cite_include).count(), 1, "the publish root includes the cite probe once");
    let cite_probe = read_repo_file(REMOTE_RELAY_PROBE_FILES[0]);
    assert_eq!(cite_probe.matches("\n#[test]\n").count(), 6, "the cite probe carries its six tests");
    for name in [
        "fn cite_gate_resolves_lxmf_cli_test_files_and_still_rejects_missing_ones()",
        "fn changelog_0_16_0_remote_relay_bullet_cites_the_files_that_carry_the_tests()",
        "fn two_peer_relay_test_lives_only_in_its_new_part_and_ci_still_names_it()",
        "fn two_peer_relay_test_keeps_the_restart_ordering_and_one_attach_per_relay()",
        "fn router_restart_test_waits_for_the_live_reattach_before_the_announce()",
        "fn ledger_0_16_0_carries_both_remote_relay_fix_commits()",
    ] {
        assert_eq!(cite_probe.matches(name).count(), 1, "the cite probe defines {name} once");
    }

    let relay_root = read_repo_file("crates/apps/lxmf-cli/tests/python_lxmd_remote_relay.rs");
    let count_include =
        "include!(\"python_lxmd_remote_relay_parts/shared_instance_local_client_count_probe.rs\");";
    let router_include = "include!(\"python_lxmd_remote_relay_parts/python_router_restart.rs\");";
    assert_eq!(relay_root.matches(count_include).count(), 1, "the relay root includes the count probe once");
    let count_at = relay_root.find(count_include).expect("count probe include");
    let router_at = relay_root.find(router_include).expect("router-restart include");
    let prelude_at = relay_root
        .find("include!(\"python_lxmd_remote_relay_parts/module_prelude.rs\");")
        .expect("the prelude include");
    assert!(prelude_at < count_at && count_at < router_at, "prelude, count probe, router-restart part, in that order");
    let count_probe = read_repo_file(REMOTE_RELAY_PROBE_FILES[1]);
    for name in [
        "fn python_shared_instance_status_local_client_count_tracks_the_live_attachment()",
        "fn wait_for_python_local_client_count_fails_within_its_bound_when_no_client_attaches()",
    ] {
        let at = count_probe.find(name).unwrap_or_else(|| panic!("the count probe defines {name}"));
        let above = &count_probe[..at];
        let attrs = above.rsplit("\n\n").next().unwrap_or(above);
        assert!(attrs.contains("#[ignore = \""), "{name} is ignored with a reason, got {attrs:?}");
    }
    assert!(count_probe.lines().count() <= 1200 && cite_probe.lines().count() <= 1200, "both probes fit the test-file budget");
}

/// The parity-matrix row for the opportunistic retry and the evidence
/// section that describe the two-relay shape cite files that exist under
/// the lxmf-cli tests, and the row names the test's new part, not the part
/// the test left.
#[test]
fn status_docs_for_the_two_relay_shape_cite_files_that_exist() {
    let matrix = read_repo_file("docs/status/reticulum-parity-matrix.md");
    let row = matrix
        .lines()
        .find(|line| line.starts_with("| `#609 queued OPPORTUNISTIC retry after Rust relay replacement` |"))
        .expect("the matrix has the opportunistic-retry row");
    let cited = cited_source_files(row);
    assert!(
        cited.contains(
            &"crates/apps/lxmf-cli/tests/python_lxmd_remote_relay_parts/shared_instance_two_peer_relay.rs"
        ),
        "the row cites the new part, got {cited:?}"
    );
    assert!(!row.contains("shared_instance_daemon.rs"), "the row no longer cites the part the test left");
    assert!(row.contains("both Rust relays are replaced"), "the row describes both relays being replaced");
    let unresolved = unresolved_source_cites(&cited);
    assert!(unresolved.is_empty(), "every cite in the row resolves, got {unresolved:?}");

    let evidence = read_repo_file(
        "docs/goals/reticulum-reference-parity-605/evidence/609-transport-local-shared.md",
    );
    for test in [
        "python_shared_instance_two_peer_relay_recovers_after_daemon_restart_e2e",
        "python_lxmf_router_delivery_cache_survives_process_restart_e2e",
    ] {
        assert!(evidence.contains(test), "the evidence names {test}");
    }
    assert!(
        evidence.contains("through two replaced Rust relays"),
        "the evidence describes the retry crossing two replaced relays"
    );
}
