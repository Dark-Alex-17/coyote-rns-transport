// Fixture pins for the ledger-contract helpers. The live-docs tests assert
// that the helpers report nothing wrong, so a helper that returned an empty
// Vec for any input would pass them all; these build the drift on synthetic
// sections and require the helpers to name it.

/// A ledger section whose legend names `legend_hash` and whose one patch
/// header names `header_hash`.
fn ledger_section_fixture(legend_hash: &str, header_hash: &str) -> String {
    format!(
        "## 0.15.1 (upstream 0.14.0)\n\n\
         `X1` is example; the `example` patch is fork commit `{legend_hash}`.\n\n\
         | File | Patch |\n| --- | --- |\n| `a.rs` | example |\n\n\
         Fork commit `{header_hash}`: X1, example. Does the one thing.\n"
    )
}

#[test]
fn header_hashes_without_a_legend_mention_names_the_unmentioned_header_hash() {
    let drifted = ledger_section_fixture("11111111", "deadbeef");
    let legend = ledger_legend("0.15.1", &drifted);
    assert_eq!(header_hashes_without_a_legend_mention(&drifted, &legend), ["deadbeef"]);

    let agreed = ledger_section_fixture("11111111", "11111111");
    let legend = ledger_legend("0.15.1", &agreed);
    assert_eq!(header_hashes_without_a_legend_mention(&agreed, &legend), Vec::<&str>::new());
}

#[test]
fn legend_hashes_without_a_header_names_the_legend_hash_no_header_carries() {
    let drifted = ledger_section_fixture("cafebabe", "deadbeef");
    assert_eq!(legend_hashes_without_a_header(&drifted, &["cafebabe"]), ["cafebabe"]);

    let agreed = ledger_section_fixture("cafebabe", "cafebabe");
    assert_eq!(legend_hashes_without_a_header(&agreed, &["cafebabe"]), Vec::<&str>::new());
}

#[test]
fn changelog_bullet_cite_helpers_return_the_gated_body_and_its_uncited_bullets() {
    let changelog = "# x\n\n## Unreleased\n\n- a (`src/lib.rs`)\n\n\
                     ## 0.15.1 - 2026-01-01\n\n### Changed\n\n- cited bullet (`src/lib.rs`)\n\
                     - uncited bullet\n  with a continuation\n\n\
                     ## 0.15.0 - 2025-12-01\n\n- old uncited\n";
    let body = "2026-01-01\n\n### Changed\n\n- cited bullet (`src/lib.rs`)\n\
                - uncited bullet\n  with a continuation\n";
    assert_eq!(
        changelog_sections_under_bullet_cite_contract(changelog),
        [("0.15.1".to_string(), body)]
    );
    assert_eq!(
        changelog_bullets_without_a_cite("0.15.1", body),
        ["- uncited bullet\n  with a continuation".to_string()]
    );
    assert_eq!(
        changelog_bullets_without_a_cite("0.15.1", "- a (`src/lib.rs`)\n"),
        Vec::<String>::new()
    );
}

#[test]
fn full_hash_contract_gate_admits_unreleased_and_versions_from_its_floor() {
    let ledger = "# l\n\n\
                  ## Unreleased (upstream 0.14.0)\n\nwip\n\n\
                  ## 0.16.0 (upstream 0.14.0)\n\nminor\n\n\
                  ## 0.15.1 (upstream 0.14.0)\n\nfloor\n\n\
                  ## 0.15.0 (upstream 0.14.0)\n\nbelow\n\n\
                  ## 0.14.1 (upstream 0.14.0)\n\nolder\n";
    let labels: Vec<String> = ledger_sections_under_full_hash_contract(ledger)
        .into_iter()
        .map(|(label, _)| label)
        .collect();
    assert_eq!(labels, ["Unreleased", "0.16.0", "0.15.1"]);
}

#[test]
fn bullet_cite_contract_gate_admits_released_versions_from_its_floor() {
    let changelog = "# x\n\n\
                     ## 0.16.0 - d\n\n- a\n\n\
                     ## 0.15.1 - d\n\n- a\n\n\
                     ## 0.15.0 - d\n\n- a\n\n\
                     ## 0.14.1 - d\n\n- a\n";
    let labels: Vec<String> = changelog_sections_under_bullet_cite_contract(changelog)
        .into_iter()
        .map(|(label, _)| label)
        .collect();
    assert_eq!(labels, ["0.16.0", "0.15.1"]);
}

#[test]
fn release_version_orders_bare_versions_against_the_contract_floors() {
    assert_eq!(release_version("0.15.1"), Some((0, 15, 1)));
    assert!(release_version("0.15.0") < Some(FULL_HASH_CONTRACT_FROM));
    assert!(release_version("0.14.1") < Some(BULLET_CITE_CONTRACT_FROM));
    assert_eq!(release_version("Unreleased"), None);
    assert_eq!(release_version("0.15.1.2"), None);
}

#[test]
fn ledger_section_does_not_let_a_label_prefix_match_a_longer_version() {
    let ledger = "# l\n\n\
                  ## 0.15.10 (upstream 0.14.0)\n\nten\n\n\
                  ## 0.15.1 (upstream 0.14.0)\n\none\n";
    assert_eq!(ledger_section(ledger, "0.15.1"), Some("## 0.15.1 (upstream 0.14.0)\n\none\n"));
    assert_eq!(ledger_section(ledger, "0.15.10"), Some("## 0.15.10 (upstream 0.14.0)\n\nten\n"));
}
