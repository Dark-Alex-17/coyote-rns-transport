// Usage-pattern pins over the 0.17.0 release cut beyond `release_0_17_0_probe.rs`:
// the additive-minor rule's two ordering edges (the sentence must OPEN the
// prose; the exclusive-shapes rule holds whichever side the bullets sit on),
// the release headings a consumer reads newest-first with ISO dates, and the
// split of this contract into parts (every part file included exactly once
// and named by the CHANGELOG bullet that announces the split).

/// The rule says a bullet-less `### Breaking` MUST open with the additive
/// sentence: prose placed before it is not an opening, however the sentence
/// then continues. A verdict that merely finds the sentence somewhere in the
/// prose would accept this shape.
#[test]
fn breaking_verdict_rejects_prose_placed_before_the_additive_sentence() {
    let section = [
        "### Breaking",
        "",
        "Nothing a consumer must change.",
        "None. This minor release is additive; it adds surface.",
        "",
        "### Added",
    ];
    assert!(
        breaking_subsection_verdict(&section).is_err(),
        "the additive sentence must open the `### Breaking` prose, not follow other prose"
    );
}

/// The exclusive-shapes rule does not depend on which side of the bullets the
/// additive sentence sits: bullets first, then `None. ...`, is the same
/// contradiction as the committed sentence-first fixture. A verdict that
/// inspects only the first non-empty line of the subsection would accept it.
#[test]
fn breaking_verdict_rejects_bullets_followed_by_the_additive_sentence() {
    let section = [
        "### Breaking",
        "",
        "- `Foo` is gone.",
        "  Migration: use `Bar`.",
        "",
        "None. This minor release is additive; it adds surface.",
        "",
        "### Added",
    ];
    assert!(
        breaking_subsection_verdict(&section).is_err(),
        "bullets followed by the additive sentence contradict each other as much as the reverse"
    );
}

/// A `## X.Y.Z - YYYY-MM-DD` CHANGELOG heading split into its version triple
/// and ISO date, or `None` for any other `## ` heading.
fn release_heading(line: &str) -> Option<((u64, u64, u64), String)> {
    let rest = line.strip_prefix("## ")?;
    let (version, date) = rest.split_once(" - ")?;
    let mut parts = version.split('.').map(|part| part.parse::<u64>().ok());
    let triple = (parts.next()??, parts.next()??, parts.next()??);
    if parts.next().is_some() {
        return None;
    }
    let date_ok = date.len() == 10
        && date.bytes().enumerate().all(|(index, byte)| match index {
            4 | 7 => byte == b'-',
            _ => byte.is_ascii_digit(),
        });
    date_ok.then(|| (triple, date.to_string()))
}

/// A consumer reads the CHANGELOG top to bottom expecting the newest release
/// first: every `## ` heading below the preamble is a release heading with
/// an ISO `YYYY-MM-DD` date (no `## Unreleased` stub survives a release
/// tree), versions strictly descend, dates never ascend, the first heading is
/// the crate's manifest version, and its date is not earlier than the day the
/// previous release's tag was cut.
#[test]
fn changelog_release_headings_read_newest_first_with_iso_dates() {
    let changelog = read_repo_file("crates/libs/rns-transport/CHANGELOG.md");
    let headings: Vec<(String, (u64, u64, u64), String)> = changelog
        .lines()
        .filter(|line| line.starts_with("## "))
        .map(|line| {
            let parsed = release_heading(line).unwrap_or_else(|| {
                panic!("every `## ` heading should read `## X.Y.Z - YYYY-MM-DD`, got {line:?}")
            });
            (line.to_string(), parsed.0, parsed.1)
        })
        .collect();
    assert!(headings.len() >= 2, "the CHANGELOG should carry at least two releases");

    let newest = format!("## {} - ", crate_version());
    assert!(
        headings[0].0.starts_with(&newest),
        "the first release heading should be the manifest version, got {:?}",
        headings[0].0
    );
    for pair in headings.windows(2) {
        let (above, below) = (&pair[0], &pair[1]);
        assert!(
            above.1 > below.1,
            "release headings should descend by version: {:?} sits above {:?}",
            above.0,
            below.0
        );
        assert!(
            above.2 >= below.2,
            "a newer release cannot be dated before an older one: {:?} sits above {:?}",
            above.0,
            below.0
        );
    }

    // The previous release's tag date (committer date of the tagged commit)
    // bounds the new release's date from below; `ls-remote`-free, local only.
    let previous = format!(
        "coyote-rns-transport-v{}.{}.{}",
        headings[1].1 .0, headings[1].1 .1, headings[1].1 .2
    );
    let tag_date = std::process::Command::new("git")
        .args(["log", "-1", "--format=%cs", &previous])
        .current_dir(repo_root())
        .output()
        .expect("git log should run");
    assert!(tag_date.status.success(), "tag {previous} should exist locally");
    let tag_date = String::from_utf8_lossy(&tag_date.stdout).trim().to_string();
    assert!(
        headings[0].2 >= tag_date,
        "{:?} should not be dated before the {previous} tag ({tag_date})",
        headings[0].0
    );
}

/// The docs contract keeps its helpers in the root and its tests in
/// `fork_release_docs_contract_parts/`: every `.rs` file in that directory is
/// `include!`d exactly once by the root (a part left out of the include list
/// would drop its tests without a failure), nothing else is included from
/// there, and the CHANGELOG bullet announcing the split names every part the
/// release shipped.
#[test]
fn every_release_docs_contract_part_is_included_once_and_named_by_the_changelog() {
    let root = read_repo_file("crates/apps/reticulumd/tests/fork_release_docs_contract.rs");
    let mut included: Vec<String> = root
        .lines()
        .filter_map(|line| {
            line.strip_prefix("include!(\"fork_release_docs_contract_parts/")
                .and_then(|rest| rest.strip_suffix("\");"))
                .map(str::to_string)
        })
        .collect();
    let mut unique = included.clone();
    unique.sort();
    unique.dedup();
    included.sort();
    assert_eq!(included, unique, "no part should be included twice");

    let mut on_disk: Vec<String> = fs::read_dir(
        repo_root().join("crates/apps/reticulumd/tests/fork_release_docs_contract_parts"),
    )
    .expect("the parts directory should exist")
    .map(|entry| entry.expect("directory entry").file_name().to_string_lossy().into_owned())
    .filter(|name| name.ends_with(".rs"))
    .collect();
    on_disk.sort();
    assert_eq!(
        included, on_disk,
        "every `.rs` file under fork_release_docs_contract_parts/ should be included exactly once"
    );

    let changelog = read_repo_file("crates/libs/rns-transport/CHANGELOG.md");
    let section = changelog_release_section(&changelog, "0.17.0");
    let bullet = joined_prose(&single_bullet_starting_with(
        &subsection_bullets(&section, "Changed"),
        "- Release docs contract, test-only",
    ));
    for part in &on_disk {
        assert!(
            bullet.contains(&format!("`{part}`")),
            "the release-docs-contract bullet should name the part `{part}`"
        );
    }
}
