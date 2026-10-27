// The `### Breaking` rule of a minor release: the verdict helper over the
// lines of a release section, the release-notes test that applies it to the
// current version, and the fixtures that pin each shape it accepts or
// rejects.

/// The rule for the `### Breaking` body of a minor release, applied to the
/// lines of its release section. With top-level `- ` bullets, every bullet
/// (joined with its indented continuation lines) carries a `Migration:`
/// clause. Without bullets the release is an additive minor and the prose,
/// joined on whitespace, opens with the exact `None. This minor release is
/// additive;` sentence followed by a non-empty reason clause.
fn breaking_subsection_verdict(section: &[&str]) -> Result<(), String> {
    let start = section
        .iter()
        .position(|line| *line == "### Breaking")
        .ok_or_else(|| "the release section has no `### Breaking` subsection".to_string())?
        + 1;
    let end = section[start..]
        .iter()
        .position(|line| line.starts_with("### "))
        .map_or(section.len(), |p| start + p);
    let breaking = &section[start..end];
    let mut bullets: Vec<String> = Vec::new();
    for line in breaking {
        if line.starts_with("- ") {
            bullets.push(line.to_string());
        } else if let Some(bullet) = bullets.last_mut() {
            if line.starts_with(' ') && !line.trim().is_empty() {
                bullet.push(' ');
                bullet.push_str(line.trim());
            }
        }
    }
    if bullets.is_empty() {
        const ADDITIVE: &str = "None. This minor release is additive;";
        let prose = breaking.join(" ").split_whitespace().collect::<Vec<_>>().join(" ");
        let reason = prose.strip_prefix(ADDITIVE).ok_or_else(|| {
            format!("`### Breaking` without bullets should open with {ADDITIVE:?}, got {prose:?}")
        })?;
        if reason.trim().is_empty() {
            return Err(format!("`### Breaking` should give the reason after {ADDITIVE:?}"));
        }
        return Ok(());
    }
    for bullet in &bullets {
        if !bullet.contains("Migration:") {
            return Err(format!(
                "every breaking bullet should carry a `Migration:` clause, got {bullet:?}"
            ));
        }
    }
    Ok(())
}

/// A release whose minor digit moved against the previous release heading
/// opens its release notes with `### Breaking`, and the README sends
/// consumers there: the subsection lists the breaking changes with a
/// migration each, or says the release is additive and why. A release that
/// only moved the patch digit must not claim breaking changes.
#[test]
fn release_notes_list_breaking_changes_on_a_minor_bump() {
    let version = crate_version();
    let changelog = read_repo_file("crates/libs/rns-transport/CHANGELOG.md");

    fn minor(version: &str) -> (u64, u64) {
        let mut parts = version.split('.').map(|part| part.parse::<u64>().expect("numeric"));
        (parts.next().expect("major"), parts.next().expect("minor"))
    }

    // Release headings only: an `## Unreleased` stub above them is allowed.
    let headings: Vec<(usize, &str)> = changelog
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            line.strip_prefix("## ").map(|rest| (index, rest.split(' ').next().unwrap_or(rest)))
        })
        .filter(|(_, heading)| heading.starts_with(|c: char| c.is_ascii_digit()))
        .collect();
    let current = headings
        .iter()
        .position(|(_, heading)| *heading == version)
        .unwrap_or_else(|| panic!("CHANGELOG should have a `## {version}` heading"));
    assert_eq!(current, 0, "the current release should be the first release heading");
    let previous = headings
        .get(1)
        .map(|(_, heading)| *heading)
        .expect("a previous release heading follows the current one");

    let lines: Vec<&str> = changelog.lines().collect();
    let section = &lines[headings[0].0 + 1..headings[1].0];
    let subsections: Vec<&str> =
        section.iter().copied().filter(|line| line.starts_with("### ")).collect();

    if minor(&version) > minor(previous) {
        assert_eq!(
            subsections.first().copied(),
            Some("### Breaking"),
            "{version} moved the minor digit against {previous}, so its release notes should open with `### Breaking`, got {subsections:?}"
        );
        if let Err(reason) = breaking_subsection_verdict(section) {
            panic!("{version} release notes: {reason}");
        }
        let readme = read_repo_file("crates/libs/rns-transport/README.md");
        assert!(
            readme.contains("`### Breaking` in `CHANGELOG.md`"),
            "the README versioning policy should send consumers to `### Breaking` in `CHANGELOG.md`"
        );
    } else {
        assert!(
            !subsections.contains(&"### Breaking"),
            "{version} only moved the patch digit against {previous}, so it must not list breaking changes"
        );
    }
}

#[test]
fn breaking_verdict_rejects_a_bare_none() {
    let section = ["### Breaking", "", "None.", "", "### Added"];
    assert!(breaking_subsection_verdict(&section).is_err(), "a bare `None.` gives no reason");
}

#[test]
fn breaking_verdict_accepts_the_additive_sentence_with_a_reason() {
    let section =
        ["### Breaking", "", "None. This minor release is additive; it", "adds surface.", ""];
    assert_eq!(breaking_subsection_verdict(&section), Ok(()));
}

#[test]
fn breaking_verdict_rejects_a_bullet_without_a_migration() {
    let section = ["### Breaking", "", "- `Foo` is gone.", "  Use `Bar`.", "", "### Added"];
    assert!(breaking_subsection_verdict(&section).is_err(), "a bullet needs `Migration:`");
}

#[test]
fn breaking_verdict_accepts_a_bullet_with_a_migration() {
    let section =
        ["### Breaking", "", "- `Foo` is gone.", "  Migration: use `Bar`.", "", "### Added"];
    assert_eq!(breaking_subsection_verdict(&section), Ok(()));
}

#[test]
fn breaking_verdict_rejects_an_empty_subsection() {
    let section = ["### Breaking", "", "### Added", "", "- x"];
    assert!(breaking_subsection_verdict(&section).is_err(), "an empty subsection says nothing");
}
