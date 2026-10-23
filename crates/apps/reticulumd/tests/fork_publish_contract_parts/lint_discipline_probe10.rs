// Pins on the ledger and changelog prose around the sequential peer-data
// lock order: the CHANGELOG Unreleased entry describes the sequential form
// under its fn-level allowance and no longer vouches for the joined one; the
// PATCHSET narrative says why the joined form was wrong (it counted sites,
// not tasks) and which patch undid it; the PATCHSET header confines the
// joined form to the commits it lived in and says it shipped in no release;
// the README row carries its two patch attributions and every row's
// attribution is a phrase the legend defines; and the code-identifier rule
// the allowance-reason probes share treats "e.g." and "i.e." as prose while
// keeping dotted paths.

/// The `## Unreleased` section of the crate changelog, whitespace-normalised.
fn changelog_unreleased_flat() -> String {
    let changelog = read_repo_file("crates/libs/rns-transport/CHANGELOG.md");
    let (_, rest) = changelog
        .split_once("## Unreleased")
        .expect("the CHANGELOG has an Unreleased section");
    let section = rest.split("\n## ").next().unwrap_or(rest);
    section.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The `## Unreleased` section of the patch-set ledger, whitespace-normalised.
fn patchset_unreleased_flat() -> String {
    let ledger = read_repo_file("docs/fork/PATCHSET.md");
    let section = ledger_section(&ledger, "Unreleased").expect("an Unreleased ledger section");
    section.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The CHANGELOG describes the sequential lock order under its allowance
/// and carries neither of the two claims the joined form once made.
#[test]
fn changelog_unreleased_names_the_sequential_lock_order_under_its_allowance() {
    let flat = changelog_unreleased_flat();
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

/// The ledger narrative names the flaw in the joined form's premise and the
/// patch that undid it, without repeating the claim the premise rested on.
#[test]
fn patchset_thirty_ninth_narrative_no_longer_vouches_for_the_joined_form() {
    let flat = patchset_unreleased_flat();
    assert!(flat.contains("counted sites, not tasks"), "the narrative names the flawed premise");
    assert!(flat.contains("the forty-third undoes it"), "the narrative names the undoing patch");
    assert!(
        !flat.contains("so neither guard spans an await"),
        "the narrative no longer vouches for the joined form"
    );
}

/// The header's behaviour sentence confines the joined form to the commits
/// between the patch that introduced it and the one that undid it.
#[test]
fn patchset_header_confines_the_joined_form_to_the_commits_it_lived_in() {
    let flat = patchset_unreleased_flat();
    let (_, rest) = flat
        .split_once("Everything else keeps its behaviour")
        .expect("the header has the behaviour sentence");
    let sentence = rest.split(';').next().unwrap_or(rest);
    for needle in ["thirty-ninth", "forty-third", "never in a release"] {
        assert!(sentence.contains(needle), "the behaviour sentence says {needle:?}, got {sentence:?}");
    }
}

/// The README row is attributed to both patches that touched it, and every
/// row's attribution is built from phrases the legend defines.
#[test]
fn patchset_readme_row_is_attributed_to_release_tooling_and_guard_discipline() {
    let ledger = read_repo_file("docs/fork/PATCHSET.md");
    let section = ledger_section(&ledger, "Unreleased").expect("an Unreleased ledger section");
    let legend = ledger_legend("Unreleased", section);
    let rows: Vec<(&str, &str)> = section
        .lines()
        .filter(|line| line.starts_with("| `"))
        .map(|line| {
            let mut cells = line.trim_matches('|').split('|').map(str::trim);
            let file = cells.next().unwrap_or_default().trim_matches('`');
            let patch = cells.next().unwrap_or_default();
            (file, patch)
        })
        .collect();
    let readme = rows
        .iter()
        .find(|(file, _)| *file == "crates/libs/rns-transport/README.md")
        .map(|(_, patch)| *patch)
        .expect("the README has a row");
    assert_eq!(readme, "release tooling, guard discipline");
    for (file, patch) in rows {
        for phrase in patch.split(", ") {
            assert!(
                legend.contains(phrase),
                "{file} is attributed to {phrase:?}, which the legend does not define"
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
