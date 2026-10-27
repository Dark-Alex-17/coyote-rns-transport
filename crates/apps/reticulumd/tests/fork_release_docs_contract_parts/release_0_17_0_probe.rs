// Pins over the 0.17.0 release artefacts beyond what the release contracts
// check. Those pin the additive-minor `### Breaking` rule by its prefix, the
// README policy by two literals and the version by nine sites; these pins
// hold the whole additive sentence (not just its prefix) as the section's body,
// the semver verdict the version bullet records, the ledger wording for the
// shared interop commit and the promoted pin, the digit the README policy
// names for each kind of release, and the one register claim about behaviour
// already shipped (`AddressHash` hex parsing since 0.14.1) against the crate
// as published.

/// The additive sentence the CHANGELOG commits to for the additive-minor
/// `### Breaking` subsection, whitespace-joined as the CHANGELOG wraps it.
const ADDITIVE_BREAKING_SENTENCE: &str = "None. This minor release is additive; it adds \
     consumer-facing surface (responder outcomes, shared-instance state events, a destination \
     lookup) that a consumer adopts deliberately, so the minor digit signals new surface to opt \
     into rather than a patch to absorb.";

/// Joins a text's lines on single spaces so a pin can quote prose the
/// ledgers wrap at 80 columns.
fn joined_prose(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The CHANGELOG section for `version`: from its `## <version> - ` heading up
/// to the next `## ` heading or the end of the file.
fn changelog_release_section(changelog: &str, version: &str) -> String {
    let heading = format!("## {version} - ");
    let start = changelog
        .find(&format!("\n{heading}"))
        .map(|at| at + 1)
        .unwrap_or_else(|| panic!("CHANGELOG should carry a `{heading}<date>` heading"));
    let body = &changelog[start..];
    let end =
        body[heading.len()..].find("\n## ").map(|at| at + heading.len()).unwrap_or(body.len());
    body[..end].to_string()
}

/// The `### <name>` subsections of a release section in file order, each
/// with its body (the text up to the next `### ` or the section's end).
fn subsections(section: &str) -> Vec<(&str, &str)> {
    let mut out = Vec::new();
    let mut rest = section;
    while let Some(at) = rest.find("\n### ") {
        let after = &rest[at + 5..];
        let name_end = after.find('\n').unwrap_or(after.len());
        let name = &after[..name_end];
        let body = &after[name_end..];
        let body_end = body.find("\n### ").unwrap_or(body.len());
        out.push((name, &body[..body_end]));
        rest = &after[name_end + body_end..];
    }
    out
}

/// The 0.17.0 `### Breaking` subsection is the first subsection and its whole
/// body is the additive sentence: the contract pins only the prefix and a
/// non-empty reason, so a reworded reason or a trailing bullet would pass it
/// while the release note no longer says what the CHANGELOG commits to.
#[test]
fn release_0_17_0_breaking_is_exactly_the_additive_sentence() {
    let changelog = read_repo_file("crates/libs/rns-transport/CHANGELOG.md");
    let section = changelog_release_section(&changelog, "0.17.0");
    let subs = subsections(&section);
    let (first_name, first_body) =
        subs.first().expect("the 0.17.0 section should have subsections");
    assert_eq!(
        *first_name, "Breaking",
        "the first subsection of a minor release is `### Breaking`"
    );
    assert_eq!(
        joined_prose(first_body),
        ADDITIVE_BREAKING_SENTENCE,
        "the additive-minor `### Breaking` body is the single additive sentence, nothing more"
    );
    assert!(
        subs.iter().any(|(name, _)| *name == "Added"),
        "an additive minor lists what it adds under `### Added`"
    );
}

/// The bullets of a release section's `### <name>` subsection, as
/// `top_level_bullets` splits them.
fn subsection_bullets(section: &str, name: &str) -> Vec<String> {
    let body = subsections(section)
        .into_iter()
        .find(|(sub, _)| *sub == name)
        .map(|(_, body)| body)
        .unwrap_or_else(|| panic!("the release section should have a `### {name}` subsection"));
    top_level_bullets(body)
}

/// The one bullet of `bullets` whose text starts with `prefix`.
fn single_bullet_starting_with(bullets: &[String], prefix: &str) -> String {
    let matching: Vec<&String> =
        bullets.iter().filter(|bullet| bullet.starts_with(prefix)).collect();
    assert_eq!(matching.len(), 1, "exactly one bullet should start with {prefix:?}");
    matching[0].clone()
}

/// The 0.17.0 version bullet itself, the first under `### Changed`, records
/// the semver verdict the release check made and why an additive minor
/// measures zero: the baseline tag, the two zero counts and the lint-level
/// explanation. The same words elsewhere in the section would not satisfy
/// the pin.
#[test]
fn release_0_17_0_version_bullet_records_the_semver_verdict() {
    let changelog = read_repo_file("crates/libs/rns-transport/CHANGELOG.md");
    let section = changelog_release_section(&changelog, "0.17.0");
    let bullets = subsection_bullets(&section, "Changed");
    let bullet =
        joined_prose(&single_bullet_starting_with(&bullets, "- The crate version is `0.17.0`"));
    for needle in [
        "`cargo semver-checks` against `v0.16.0`",
        "0 major and 0 minor findings",
        "additive changes are below cargo-semver-checks' default lint level",
    ] {
        assert!(bullet.contains(needle), "the 0.17.0 version bullet should say {needle:?}");
    }
}

/// The two ledgers agree on the release's two wording fixes: the PATCHSET
/// legend says the interop patch's single commit is also a `response
/// outcomes` commit, and the CHANGELOG's ledger-contracts bullet says the
/// full-hash pin covered `## Unreleased` while the work was in progress and
/// names the `0.17.0` label now.
#[test]
fn release_0_17_0_ledgers_name_the_shared_interop_commit_and_the_promoted_pin() {
    let patchset = read_repo_file("docs/fork/PATCHSET.md");
    let heading = "\n## 0.17.0 (upstream 0.14.0)\n";
    let start = patchset.find(heading).expect("PATCHSET should carry the 0.17.0 heading");
    let body = &patchset[start + heading.len()..];
    let end = body.find("\n## ").unwrap_or(body.len());
    let release_section = joined_prose(&body[..end]);
    assert!(
        release_section.contains("is fork commit `79aae7ed`, also a `response outcomes` commit"),
        "the PATCHSET legend should say the interop commit is also a response outcomes commit"
    );

    let changelog = read_repo_file("crates/libs/rns-transport/CHANGELOG.md");
    let section = changelog_release_section(&changelog, "0.17.0");
    let bullets = subsection_bullets(&section, "Changed");
    let bullet =
        joined_prose(&single_bullet_starting_with(&bullets, "- Ledger contracts, test-only"));
    for needle in ["while the work was in progress", "names the `0.17.0` label"] {
        assert!(bullet.contains(needle), "the ledger-contracts bullet should say {needle:?}");
    }
}

/// The README release policy names the digit each kind of fork release
/// bumps: bug fixes and API a consumer need not opt into the patch digit,
/// breaking changes the minor digit with `0.15.0` as the first, and additive
/// API may take the minor digit with `0.17.0` as the first; the two literals
/// the contracts quote stay in place, and the upstream-rebase sentence names
/// the digit for each outcome of the rebase.
#[test]
fn crate_readme_release_policy_names_the_digit_each_release_kind_bumps() {
    let readme = joined_prose(&read_repo_file("crates/libs/rns-transport/README.md"));
    for needle in [
        "The version follows cargo semver relative to the upstream baseline.",
        "A fork release that only fixes bugs, or adds API a consumer need not opt into, bumps \
         the patch digit;",
        "a fork change that breaks the public API bumps the minor digit, and `0.15.0` is the \
         first such minor bump; its breaking changes are listed under `### Breaking` in \
         `CHANGELOG.md`.",
        "A fork release that adds public API without breaking any may bump the minor digit",
        "`0.17.0` is the first such additive minor, and its `### Breaking` says so in place of a \
         list.",
        "When upstream ships a release the fork is rebased onto it and takes the first version \
         above both the upstream number and the fork's last published number: the next minor \
         digit when the rebase breaks the fork's public API, the next patch digit otherwise.",
    ] {
        assert!(readme.contains(needle), "README release policy should say {needle:?}");
    }
    assert!(
        !readme.contains("(additive API, internal fixes) bump the patch digit"),
        "the sentence that routed additive API to the patch digit is gone"
    );
}

/// The one 0.17.0 register entry about behaviour that shipped earlier holds
/// against the crate: `AddressHash::new_from_hex_string` rejects every
/// malformed string with `RnsError::IncorrectHash` and round-trips a valid
/// one, the register names the method, the error and the release that fixed
/// it, and that release's `### Fixed` carries the bullet the register points
/// at.
#[test]
fn release_0_17_0_register_hex_parsing_claim_holds_against_the_crate() {
    use rns_transport::error::RnsError;
    use rns_transport::hash::AddressHash;

    let valid = "0123456789abcdef0123456789abcdef";
    let parsed = AddressHash::new_from_hex_string(valid).expect("32 hex chars parse");
    assert_eq!(parsed.to_hex_string(), valid, "a valid hash round-trips through hex");
    for malformed in [
        "",
        "0123456789abcdef0123456789abcde",      // 31 chars
        "0123456789abcdef0123456789abcdef0",    // 33 chars
        "0123456789abcdef0123456789abcdeg",     // non-hex ASCII
        "+123456789abcdef0123456789abcdef",     // sign-prefixed pair
        "0123456789abcdef0123456789abcd\u{e9}", // multi-byte UTF-8, 32 bytes
    ] {
        assert!(
            matches!(AddressHash::new_from_hex_string(malformed), Err(RnsError::IncorrectHash)),
            "{malformed:?} should be rejected with RnsError::IncorrectHash"
        );
    }

    let readme = read_repo_file("crates/libs/rns-transport/README.md");
    let block_start = readme
        .find("What `0.17.0` carries on top")
        .expect("README should open its patch list with the 0.17.0 register");
    let block_end = readme[block_start..]
        .find("What `0.16.0` carries on top")
        .map(|at| block_start + at)
        .expect("the 0.16.0 register follows the 0.17.0 one");
    let block = joined_prose(&readme[block_start..block_end]);
    for needle in
        ["`AddressHash::new_from_hex_string`", "`RnsError::IncorrectHash`", "since 0.14.1"]
    {
        assert!(block.contains(needle), "the 0.17.0 register should say {needle:?}");
    }

    let changelog = read_repo_file("crates/libs/rns-transport/CHANGELOG.md");
    let fixed_0_14_1 = subsections(&changelog_release_section(&changelog, "0.14.1"))
        .into_iter()
        .find(|(name, _)| *name == "Fixed")
        .map(|(_, body)| joined_prose(body))
        .expect("the 0.14.1 section should have a `### Fixed` subsection");
    assert!(
        fixed_0_14_1.contains("`AddressHash::new_from_hex_string`"),
        "the 0.14.1 `### Fixed` the register points at should name the method"
    );
}
