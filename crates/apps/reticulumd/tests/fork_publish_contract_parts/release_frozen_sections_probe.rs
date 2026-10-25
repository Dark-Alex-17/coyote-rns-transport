// Usage pins for the release rule the ledgers state in prose: a released
// section is frozen. Once a `coyote-rns-transport-v*` tag is cut, every
// `## <version>` section of the crate CHANGELOG and of the patch-set ledger
// that existed at that tag reads byte for byte as it did there, and the README
// patch-list block of that release keeps its bullets verbatim; a later release
// only adds a section above them (the shape of the 0.15.1 and 0.16.0 promotions).
// The pins read the files at the newest fork tag reachable from HEAD with
// `git show`, read-only, and compare them with the working tree, so a
// promotion that edits history instead of adding to it turns red here while
// the structural ledger contracts (which only see the current tree) stay green.
// They skip, in so many words, on a checkout with no reachable fork tag, the
// same precondition the zero-argument ledger script has.

/// The newest `coyote-rns-transport-v<major>.<minor>.<patch>` tag that is an
/// ancestor of HEAD, with its parsed version; `None` when there is none.
#[cfg(unix)]
fn newest_reachable_fork_tag() -> Option<(String, String)> {
    let output = Command::new("git")
        .args(["tag", "--list", "coyote-rns-transport-v*", "--merged", "HEAD"])
        .current_dir(repo_root())
        .output()
        .expect("run git tag --list");
    assert!(output.status.success(), "git tag --list should succeed");
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|tag| {
            let version = tag.strip_prefix("coyote-rns-transport-v")?;
            let mut parts = version.split('.').map(|part| part.parse::<u64>().ok());
            let key = (parts.next()??, parts.next()??, parts.next()??);
            parts.next().is_none().then(|| (key, tag.to_string(), version.to_string()))
        })
        .max_by_key(|(key, _, _)| *key)
        .map(|(_, tag, version)| (tag, version))
}

/// `git show <tag>:<relative>` from the repository root.
#[cfg(unix)]
fn file_at_tag(tag: &str, relative: &str) -> String {
    let output = Command::new("git")
        .arg("show")
        .arg(format!("{tag}:{relative}"))
        .current_dir(repo_root())
        .output()
        .expect("run git show");
    assert!(
        output.status.success(),
        "git show {tag}:{relative} should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("ledger files are UTF-8")
}

/// Every release section of a ledger: each `## <digit...>` heading (an
/// `## Unreleased` heading is not a release) with the text that follows it up
/// to the next `## ` heading or the end of the file, in file order.
#[cfg(unix)]
fn release_sections(text: &str) -> Vec<(&str, &str)> {
    let mut starts: Vec<(usize, &str)> = Vec::new();
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        if let Some(rest) = line.strip_prefix("## ") {
            let label = rest[..rest.find(' ').unwrap_or(rest.len())].trim_end();
            if label.starts_with(|c: char| c.is_ascii_digit()) {
                starts.push((offset, label));
            } else if !starts.is_empty() {
                // A non-release heading below a release heading closes it.
                starts.push((offset, ""));
            }
        }
        offset += line.len();
    }
    let mut sections = Vec::new();
    for (index, (start, label)) in starts.iter().enumerate() {
        if label.is_empty() {
            continue;
        }
        let end = starts.get(index + 1).map_or(text.len(), |(next, _)| *next);
        sections.push((*label, &text[*start..end]));
    }
    sections
}

/// The bullets of the README patch-list block headed `What \`<version>\`
/// carries on top of ...`: from the first `- ` line after that marker to the
/// next `What \`` marker or `## ` heading. The marker sentence itself is not
/// compared, because a promotion may unwrap it from the intro sentence into a
/// standalone paragraph.
#[cfg(unix)]
fn readme_patch_block_bullets(readme: &str, version: &str) -> String {
    let marker = format!("What `{version}` carries on top");
    let at = readme
        .find(&marker)
        .unwrap_or_else(|| panic!("the README patch list should have a {marker:?} block"));
    let rest = &readme[at..];
    let first_bullet = rest.find("\n- ").expect("the block has bullets") + 1;
    let body = &rest[first_bullet..];
    let end =
        ["\nWhat `", "\n## "].iter().filter_map(|stop| body.find(stop)).min().unwrap_or(body.len());
    body[..end].trim_end().to_string()
}

/// Every release section of both ledgers that existed at the newest reachable
/// fork tag is still present in the working tree, byte for byte, in the same
/// order and contiguous: a later release adds its section above them, never
/// between them.
#[cfg(unix)]
#[test]
fn released_ledger_sections_read_as_they_did_at_the_newest_reachable_fork_tag() {
    let Some((tag, _)) = newest_reachable_fork_tag() else {
        eprintln!("skipping: no coyote-rns-transport-v* tag reachable from HEAD in this checkout");
        return;
    };
    for relative in ["crates/libs/rns-transport/CHANGELOG.md", "docs/fork/PATCHSET.md"] {
        let tagged = file_at_tag(&tag, relative);
        let head = read_repo_file(relative);
        let sections = release_sections(&tagged);
        assert!(!sections.is_empty(), "{relative} at {tag} should have release sections");
        let mut cursor = 0;
        for (index, (label, section)) in sections.into_iter().enumerate() {
            let at = head[cursor..].find(section).unwrap_or_else(|| {
                panic!(
                    "the `## {label}` section of {relative} was released in {tag} and is frozen; \
                     it no longer reads as it did at that tag (or has moved above a section \
                     that precedes it there)"
                )
            });
            assert!(
                index == 0 || at == 0,
                "the `## {label}` section of {relative} should follow the section above it \
                 exactly as it did at {tag}; a later release adds its section above the \
                 released ones, never between them (found {at} bytes inserted)"
            );
            cursor += at + section.len();
        }
    }
}

/// The README patch-list block of the release the newest reachable fork tag
/// names keeps its bullets verbatim; a later release demotes the sentence
/// that heads it, never the bullets.
#[cfg(unix)]
#[test]
fn readme_patch_list_block_of_the_tagged_release_keeps_its_bullets() {
    let Some((tag, version)) = newest_reachable_fork_tag() else {
        eprintln!("skipping: no coyote-rns-transport-v* tag reachable from HEAD in this checkout");
        return;
    };
    let relative = "crates/libs/rns-transport/README.md";
    let tagged = readme_patch_block_bullets(&file_at_tag(&tag, relative), &version);
    let head = readme_patch_block_bullets(&read_repo_file(relative), &version);
    assert_eq!(
        head, tagged,
        "the README `What `{version}` carries on top of ...` bullets were released in {tag} \
         and are frozen"
    );
}
