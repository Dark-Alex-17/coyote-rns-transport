// Pins on the ledger and changelog prose around the sequential peer-data
// lock order: the CHANGELOG 0.16.0 entry describes the sequential form
// under its fn-level allowance and no longer vouches for the joined one; the
// PATCHSET narrative says why the joined form was wrong (it counted sites,
// not tasks) and which patch undid it; the PATCHSET header confines the
// joined form to the commits it lived in and says it shipped in no release;
// the README row carries every patch attribution that touched it; and the
// code-identifier rule the allowance-reason probes share treats "e.g." and
// "i.e." as prose while keeping dotted paths. Both 0.16.0 sections describe
// the probe9 census pin in its split form. Released sections are frozen,
// so the literal 0.16.0 label is the anchor rather than the newest heading,
// and the negative pins guard that released section against tampering.

/// The `## 0.16.0` section of the crate changelog, whitespace-normalised.
fn changelog_0_16_0_flat() -> String {
    let changelog = read_repo_file("crates/libs/rns-transport/CHANGELOG.md");
    let (_, rest) = changelog
        .split_once("\n## 0.16.0 - ")
        .expect("the CHANGELOG has a released 0.16.0 section");
    let section = rest.split("\n## ").next().unwrap_or(rest);
    section.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The `## 0.16.0` section of the patch-set ledger, whitespace-normalised.
fn patchset_0_16_0_flat() -> String {
    let ledger = read_repo_file("docs/fork/PATCHSET.md");
    let section = ledger_section(&ledger, "0.16.0").expect("the 0.16.0 ledger section");
    section.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The CHANGELOG describes the sequential lock order under its allowance
/// and carries neither of the two claims the joined form once made.
#[test]
fn changelog_0_16_0_names_the_sequential_lock_order_under_its_allowance() {
    let flat = changelog_0_16_0_flat();
    assert!(
        flat.contains(
            "keeps taking `state` and then `dedupe` in sequence under a fn-level allowance"
        ),
        "the CHANGELOG describes the sequential lock order under its allowance"
    );
    assert!(
        !flat.contains("neither guard spans an await"),
        "the CHANGELOG no longer claims the guards never span an await"
    );
    assert!(
        !flat.contains("carries no allowance (`iface/auto_runtime_parts"),
        "the CHANGELOG no longer says the peer-data loop carries no allowance"
    );
}

/// The thirty-ninth narrative itself (not a later one quoting it) names the
/// flaw in the joined form's premise and the patch that undid it, without
/// repeating the claim the premise rested on.
#[test]
fn patchset_thirty_ninth_narrative_no_longer_vouches_for_the_joined_form() {
    let flat = patchset_0_16_0_flat();
    let (_, rest) = flat
        .split_once("The thirty-ninth has")
        .expect("the ledger narrates the thirty-ninth patch");
    let (narrative, _) =
        rest.split_once("The fortieth").expect("the fortieth narrative follows the thirty-ninth");
    assert!(
        narrative.contains("counted sites, not tasks"),
        "the narrative names the flawed premise, got {narrative:?}"
    );
    assert!(
        narrative.contains("the forty-third undoes it"),
        "the narrative names the undoing patch, got {narrative:?}"
    );
    assert!(
        !narrative.contains("so neither guard spans an await"),
        "the narrative no longer vouches for the joined form"
    );
}

/// The header's behaviour sentence confines the joined form to the commits
/// between the patch that introduced it and the one that undid it.
#[test]
fn patchset_header_confines_the_joined_form_to_the_commits_it_lived_in() {
    let flat = patchset_0_16_0_flat();
    let (_, rest) = flat
        .split_once("Everything else keeps its behaviour")
        .expect("the header has the behaviour sentence");
    let sentence = rest.split(';').next().unwrap_or(rest);
    for needle in ["thirty-ninth", "forty-third", "never in a release"] {
        assert!(
            sentence.contains(needle),
            "the behaviour sentence says {needle:?}, got {sentence:?}"
        );
    }
}

/// The README row is attributed to every patch that touched it.
#[test]
fn patchset_readme_row_is_attributed_to_every_patch_that_touched_it() {
    let ledger = read_repo_file("docs/fork/PATCHSET.md");
    let section = ledger_section(&ledger, "0.16.0").expect("the 0.16.0 ledger section");
    let readme = section
        .lines()
        .filter(|line| line.starts_with("| `"))
        .find_map(|line| {
            let mut cells = line.trim_matches('|').split('|').map(str::trim);
            let file = cells.next().unwrap_or_default().trim_matches('`');
            (file == "crates/libs/rns-transport/README.md")
                .then(|| cells.next().unwrap_or_default())
        })
        .expect("the README has a row");
    assert_eq!(
        readme,
        "release tooling, guard discipline, shared-instance hardening, carried hygiene, \
         release 0.16.0"
    );
}

/// The forty-sixth narrative and the CHANGELOG agree on how the lease probe
/// reproduces the removed lock-span scenario: spec 1 the pass and the
/// handoff, spec 2 the queued wait, not a half of spec 1.
#[test]
fn patchset_forty_sixth_narrative_states_the_lease_spec_split() {
    let flat = patchset_0_16_0_flat();
    let (_, rest) = flat
        .split_once("The forty-sixth is the run-closing hygiene pass")
        .expect("the ledger narrates the forty-sixth carried-hygiene commit");
    let (narrative, _) = rest
        .split_once("The forty-seventh")
        .expect("the forty-seventh narrative follows the forty-sixth");
    assert!(
        narrative.contains("spec 1 the pass and the handoff, spec 2 the queued wait"),
        "the narrative splits the scenario across the two specs, got {narrative:?}"
    );
    assert!(
        !narrative.contains("first half of spec 1"),
        "the narrative no longer confines the scenario to a half of spec 1"
    );
    assert!(
        changelog_0_16_0_flat()
            .contains("spec 1 drives the pass and the handoff, spec 2 the queued wait"),
        "the CHANGELOG states the same spec split"
    );
}

/// Both 0.16.0 sections describe the probe9 census pin as it is after the
/// split: the live comparison holds only for an Unreleased section, so the
/// older wording that promised the ledger census always matches the tree is
/// gone. The pinned-by list is the last place either section cites probe9
/// with a parenthetical. The superseded phrases are assembled so a search
/// for them across the tree finds only the ledgers.
#[test]
fn ledgers_0_16_0_describe_the_split_census_pin() {
    let superseded =
        [concat!("census ", "matches the tree"), concat!("census sentence ", "matches the tree")];
    for (name, flat) in
        [("PATCHSET", patchset_0_16_0_flat()), ("CHANGELOG", changelog_0_16_0_flat())]
    {
        let (_, rest) = flat
            .rsplit_once("lint_discipline_probe9.rs` (")
            .unwrap_or_else(|| panic!("the {name} 0.16.0 section cites probe9 with a parenthetical"));
        let parenthetical = rest.split(')').next().unwrap_or(rest);
        assert!(
            parenthetical.contains("equals the census the tree carries"),
            "the {name} probe9 parenthetical states the split census pin, got {parenthetical:?}"
        );
        for phrase in superseded {
            assert!(
                !flat.contains(phrase),
                "the {name} 0.16.0 section no longer says {phrase:?}"
            );
        }
    }
}

/// Latin abbreviations are prose, dotted paths and calls are identifiers,
/// and a hyphenated phrase is neither.
#[test]
fn code_identifier_rule_excludes_eg_and_ie_and_keeps_dotted_paths() {
    assert!(!is_code_identifier("e.g."));
    assert!(!is_code_identifier("i.e."));
    assert!(!is_code_identifier("e.g"));
    assert!(is_code_identifier("state.lock"));
    assert!(is_code_identifier("dedupe.lock()"));
    assert!(!is_code_identifier("guard-across-await"));
}
