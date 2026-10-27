/// `(major, minor, patch)` of a release label; `None` for `Unreleased` or
/// anything else that is not a bare `X.Y.Z`.
fn release_version(label: &str) -> Option<(u64, u64, u64)> {
    let mut parts = label.split('.').map(|part| part.parse::<u64>().ok());
    match (parts.next(), parts.next(), parts.next(), parts.next()) {
        (Some(Some(major)), Some(Some(minor)), Some(Some(patch)), None) => {
            Some((major, minor, patch))
        }
        _ => None,
    }
}

/// The labels of a markdown file's `## ` headings in file order: the text
/// after `## ` up to the first space, so `## 0.15.1 (upstream 0.14.0)` and
/// `## 0.15.1 - 2026-10-21` both give `0.15.1`.
fn section_labels(markdown: &str) -> Vec<&str> {
    markdown
        .lines()
        .filter_map(|line| line.strip_prefix("## "))
        .map(|heading| heading.split(' ').next().unwrap_or(heading))
        .collect()
}

/// Sections released before this version keep their text as published: the
/// 0.15.0 legend names `73f177b4` in prose while that commit heads no patch
/// of its own. The all-legend-hash check and the header-to-legend check
/// therefore start at 0.15.1.
const FULL_HASH_CONTRACT_FROM: (u64, u64, u64) = (0, 15, 1);

/// The ledger sections held to the full hash contract, as `(label, section)`:
/// `## Unreleased` when present, then every `## X.Y.Z (upstream <base>)`
/// section from `FULL_HASH_CONTRACT_FROM` upward.
fn ledger_sections_under_full_hash_contract(ledger: &str) -> Vec<(String, &str)> {
    section_labels(ledger)
        .into_iter()
        .filter(|label| {
            *label == "Unreleased"
                || release_version(label).is_some_and(|version| version >= FULL_HASH_CONTRACT_FROM)
        })
        .map(|label| {
            let section = ledger_section(ledger, label)
                .unwrap_or_else(|| panic!("the `## {label}` heading should open a ledger section"));
            (label.to_string(), section)
        })
        .collect()
}

/// Pins which sections the gate admits at this tree: the releases from 0.15.1
/// up, never 0.15.0 or 0.14.1. Extend the expected labels when a section is
/// added or released.
#[test]
fn full_hash_contract_covers_the_newest_section_and_the_releases_from_0_15_1() {
    let ledger = read_repo_file("docs/fork/PATCHSET.md");
    let labels: Vec<String> = ledger_sections_under_full_hash_contract(&ledger)
        .into_iter()
        .map(|(label, _)| label)
        .collect();
    assert_eq!(
        labels,
        ["0.17.0", "0.16.0", "0.15.1"],
        "the full hash contract should cover the newest section and the releases from 0.15.1 \
         up; extend this pin when a section is added or released"
    );
}

/// Every backticked short hash a legend names, under any patch, must head a
/// patch in its section, since each patch header lists every commit a rebase
/// must re-apply. The release-patch-only check keeps running for the current
/// release; this one holds the whole legend of every gated section to it.
#[test]
fn patch_set_ledger_legend_hashes_each_have_a_patch_header() {
    let ledger = read_repo_file("docs/fork/PATCHSET.md");
    for (label, section) in ledger_sections_under_full_hash_contract(&ledger) {
        let legend = ledger_legend(&label, section);
        let legend_hashes: Vec<&str> = legend.split(' ').filter_map(commit_hash).collect();
        assert!(
            !legend_hashes.is_empty(),
            "the {label} legend should name the fork commits of each patch"
        );

        let missing = legend_hashes_without_a_header(section, &legend_hashes);
        assert!(
            missing.is_empty(),
            "every fork commit the {label} legend names should head a patch in the section, \
             since each patch header lists every commit a rebase must re-apply; missing: {}",
            missing.join(", ")
        );
    }
}

/// The patch-header hashes of a section that its legend does not name.
fn header_hashes_without_a_legend_mention<'a>(section: &'a str, legend: &str) -> Vec<&'a str> {
    let legend_hashes: Vec<&str> = legend.split(' ').filter_map(commit_hash).collect();
    ledger_header_hashes(section).into_iter().filter(|hash| !legend_hashes.contains(hash)).collect()
}

/// The other direction of the legend-to-header check: a rebase reads the
/// commits to re-apply from the patch headers and which patch each belongs to
/// from the legend, so a header hash the legend never names is a commit no
/// patch claims.
#[test]
fn patch_set_ledger_patch_header_hashes_are_named_by_the_legend() {
    let ledger = read_repo_file("docs/fork/PATCHSET.md");
    for (label, section) in ledger_sections_under_full_hash_contract(&ledger) {
        let legend = ledger_legend(&label, section);
        let unnamed = header_hashes_without_a_legend_mention(section, &legend);
        assert!(
            unnamed.is_empty(),
            "every fork commit a {label} patch header names should appear in the section's \
             legend, which is where a rebase reads the patch a commit belongs to; unnamed: {}",
            unnamed.join(", ")
        );
    }
}

/// Released CHANGELOG sections from this version up carry a pinning cite on
/// every bullet. Earlier sections keep their text as published and are
/// exempt: 0.15.0 has ten uncited bullets (its seven Breaking migration
/// notes, two Changed bullets and one Added bullet) and 0.14.1 has two, and
/// released notes are not edited.
const BULLET_CITE_CONTRACT_FROM: (u64, u64, u64) = (0, 15, 1);

/// The released `## X.Y.Z - <date>` sections of the crate CHANGELOG from
/// `BULLET_CITE_CONTRACT_FROM` upward, as `(version, body)`; `## Unreleased`
/// has its own check.
fn changelog_sections_under_bullet_cite_contract(changelog: &str) -> Vec<(String, &str)> {
    section_labels(changelog)
        .into_iter()
        .filter(|label| {
            release_version(label).is_some_and(|version| version >= BULLET_CITE_CONTRACT_FROM)
        })
        .map(|label| {
            let heading = format!("\n## {label} - ");
            let start = changelog.find(&heading).unwrap_or_else(|| {
                panic!("the crate CHANGELOG heading for {label} should read `## {label} - <date>`")
            });
            let body = &changelog[start + heading.len()..];
            let body = body.find("\n## ").map_or(body, |end| &body[..end]);
            (label.to_string(), body)
        })
        .collect()
}

/// Pins which released CHANGELOG sections the bullet-cite gate admits at this
/// tree: 0.17.0, 0.16.0 and 0.15.1, nothing earlier. Extend the expected
/// labels when a section is released.
#[test]
fn bullet_cite_contract_covers_the_releases_from_0_15_1() {
    let changelog = read_repo_file("crates/libs/rns-transport/CHANGELOG.md");
    let labels: Vec<String> = changelog_sections_under_bullet_cite_contract(&changelog)
        .into_iter()
        .map(|(label, _)| label)
        .collect();
    assert_eq!(
        labels,
        ["0.17.0", "0.16.0", "0.15.1"],
        "the bullet cite contract should cover the released sections from 0.15.1 up; extend \
         this pin when a section is released"
    );
}

/// The top-level `- ` bullets of a CHANGELOG section that cite no `.rs` or
/// `.py` file, each with its two-space-indented continuation lines.
fn changelog_bullets_without_a_cite(label: &str, section: &str) -> Vec<String> {
    let mut bullets: Vec<Vec<&str>> = Vec::new();
    for line in section.lines() {
        if line.starts_with("- ") {
            bullets.push(vec![line]);
        } else if line.starts_with("  ") {
            bullets
                .last_mut()
                .unwrap_or_else(|| {
                    panic!("the {label} release notes indent a line before their first bullet")
                })
                .push(line);
        }
    }
    assert!(!bullets.is_empty(), "the {label} release notes should carry at least one bullet");
    bullets
        .iter()
        .map(|bullet| bullet.join("\n"))
        .filter(|text| cited_source_files(text).is_empty())
        .collect()
}

/// Released bullets are held to the same per-bullet cite as `## Unreleased`:
/// each names a file that pins the behaviour, and every cite in the section
/// resolves under the same roots.
#[test]
fn released_notes_bullets_each_cite_a_pinning_file() {
    let changelog = read_repo_file("crates/libs/rns-transport/CHANGELOG.md");
    let sections = changelog_sections_under_bullet_cite_contract(&changelog);
    assert!(
        !sections.is_empty(),
        "the crate CHANGELOG should have a released section at or above {BULLET_CITE_CONTRACT_FROM:?}"
    );
    for (version, section) in sections {
        let uncited = changelog_bullets_without_a_cite(&version, section);
        assert!(
            uncited.is_empty(),
            "every {version} bullet should cite at least one file that pins it; these cite none:\n{}",
            uncited.join("\n")
        );
        let unresolved = unresolved_source_cites(&cited_source_files(section));
        assert!(
            unresolved.is_empty(),
            "every file cited in the {version} release notes should exist under the crate, the \
             reticulumd tests, the rns-tools sources or the lxmf-cli tests:\n{}",
            unresolved.join("\n")
        );
    }
}

const FORK_RELEASE_SCRIPTS: [&str; 4] = [
    "patchset-ledger-check.sh",
    "workspace-clippy-classify.sh",
    "old-name-tracked-scan.sh",
    "semver-major-findings-check.sh",
];

/// The release checks the runbook and the README recipe hand to a releaser
/// exist as runnable bash under `tools/scripts/`, are tracked by git (not a
/// local file a release run left behind), and every document that walks a
/// releaser through a fork release names them, so none can be dropped or
/// renamed without the prose following.
#[test]
fn fork_release_scripts_are_committed_and_named_by_the_runbooks() {
    let root = repo_root();
    let tracked = git_tracked_files();
    for script in FORK_RELEASE_SCRIPTS {
        let relative = format!("tools/scripts/{script}");
        assert!(
            tracked.contains(&relative),
            "{relative} should be tracked by git, not a local file the release leaves behind"
        );
        let path = root.join(&relative);
        let text = fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("tools/scripts/{script} should exist: {err}"));
        assert!(
            text.lines().next() == Some("#!/usr/bin/env bash"),
            "tools/scripts/{script} should open with the bash shebang"
        );
        assert!(
            text.contains("set -euo pipefail"),
            "tools/scripts/{script} should run under `set -euo pipefail`"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&path).expect("script metadata").permissions().mode();
            assert!(
                mode & 0o111 != 0,
                "tools/scripts/{script} should be executable, mode {mode:o}"
            );
        }
    }

    let runbook = read_repo_file("docs/runbooks/release-runbook.md");
    let readme = read_repo_file("crates/libs/rns-transport/README.md");
    let recipe_at = readme
        .find("\n## Rebase recipe")
        .expect("crate README should have a `## Rebase recipe` section");
    let recipe = &readme[recipe_at + 1..];
    let recipe = recipe.find("\n## ").map_or(recipe, |end| &recipe[..end]);
    for script in FORK_RELEASE_SCRIPTS {
        assert!(
            runbook.contains(script),
            "docs/runbooks/release-runbook.md should name tools/scripts/{script}"
        );
        assert!(
            recipe.contains(script),
            "the crate README `## Rebase recipe` section should name tools/scripts/{script}"
        );
    }

    let ledger = read_repo_file("docs/fork/PATCHSET.md");
    let preamble = ledger.find("\n## ").map_or(ledger.as_str(), |end| &ledger[..end]);
    assert!(
        preamble.contains("patchset-ledger-check.sh"),
        "the docs/fork/PATCHSET.md preamble should name the script that checks the ledger"
    );
}
