use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().expect("repo root")
}

fn read_repo_file(relative: &str) -> String {
    let path = repo_root().join(relative);
    fs::read_to_string(&path).unwrap_or_else(|err| panic!("read {}: {err}", path.display()))
}

fn crate_version() -> String {
    read_repo_file("crates/libs/rns-transport/Cargo.toml")
        .lines()
        .find_map(|line| line.strip_prefix("version = \""))
        .and_then(|rest| rest.strip_suffix('"'))
        .expect("coyote-rns-transport manifest should declare a package version")
        .to_string()
}

/// Patch ids in the ledger legend are one uppercase letter and one digit,
/// such as `B3` or `F1`.
fn is_patch_id(token: &str) -> bool {
    let mut chars = token.chars();
    matches!(
        (chars.next(), chars.next(), chars.next()),
        (Some(letter), Some(digit), None) if letter.is_ascii_uppercase() && digit.is_ascii_digit()
    )
}

#[test]
fn upstream_publish_workflow_is_disabled() {
    let workflow = read_repo_file(".github/workflows/crates-io-publish.yml");

    for required in [
        "if: false",
        "Disabled in this fork: publishing happens only through publish-coyote-rns-transport.yml.",
    ] {
        assert!(
            workflow.contains(required),
            "upstream crates.io publish workflow should include required token {required:?}"
        );
    }
}

#[test]
fn fork_publish_workflow_pins_its_contract() {
    let workflow = read_repo_file(".github/workflows/publish-coyote-rns-transport.yml");

    for required in [
        "workflow_dispatch:",
        "push:",
        "tags:",
        "- 'coyote-rns-transport-v*'",
        "      ref:\n        description: Git ref, tag, or SHA to publish from",
        "dry_run",
        "type: boolean",
        "libdbus-1-dev",
        "dtolnay/rust-toolchain@stable",
        "CARGO_REGISTRY_TOKEN: ${{ secrets.CARGO_REGISTRY_TOKEN }}",
        "cargo publish -p coyote-rns-transport --locked",
        "--dry-run",
        "PUBLISH_REF: ${{ inputs.ref || github.ref_name }}",
        "run: tools/scripts/check-publish-tag-version.sh",
        "concurrency:",
        "default: main",
    ] {
        assert!(
            workflow.contains(required),
            "fork publish workflow should include required token {required:?}"
        );
    }
}

#[test]
fn publish_tag_version_guard_pins_its_contract() {
    let script = read_repo_file("tools/scripts/check-publish-tag-version.sh");

    for required in [
        "cargo pkgid --locked -p coyote-rns-transport",
        "[ \"$crate_version\" != \"$tag_version\" ]",
        "exit 1",
    ] {
        assert!(
            script.contains(required),
            "publish tag version guard should include required token {required:?}"
        );
    }
}

#[cfg(unix)]
#[test]
fn publish_tag_version_guard_rejects_a_mismatched_tag() {
    use std::process::Command;

    let script = repo_root().join("tools/scripts/check-publish-tag-version.sh");
    let run = |publish_ref: Option<&str>| {
        let mut command = Command::new("bash");
        command.arg(&script).current_dir(repo_root());
        match publish_ref {
            Some(publish_ref) => command.env("PUBLISH_REF", publish_ref),
            None => command.env_remove("PUBLISH_REF"),
        };
        let output = command.output().expect("run check-publish-tag-version.sh");
        (output.status.code(), String::from_utf8_lossy(&output.stdout).into_owned())
    };

    let (code, stdout) = run(Some("coyote-rns-transport-v9.9.9"));
    assert_ne!(code, Some(0), "a mismatched release tag must fail the guard");
    assert!(
        stdout.contains("::error::Tag coyote-rns-transport-v9.9.9 names version 9.9.9"),
        "mismatch output should name the tag version, got {stdout:?}"
    );

    let matching_tag = format!("coyote-rns-transport-v{}", crate_version());
    let (code, stdout) = run(Some(&matching_tag));
    assert_eq!(code, Some(0), "tag {matching_tag:?} should pass the guard, got {stdout:?}");

    let (code, stdout) = run(Some("main"));
    assert_eq!(code, Some(0), "a non-tag ref should pass the guard, got {stdout:?}");
    assert!(
        stdout.contains("is not a coyote-rns-transport-v* release tag"),
        "non-tag output should say the check was skipped, got {stdout:?}"
    );

    let (code, _) = run(None);
    assert_eq!(code, Some(2), "an unset PUBLISH_REF should exit with the usage code");
}

#[test]
fn crate_readme_pins_the_consumer_contract() {
    let readme = read_repo_file("crates/libs/rns-transport/README.md");
    let manifest = read_repo_file("crates/libs/rns-transport/Cargo.toml");
    let workspace_manifest = read_repo_file("Cargo.toml");

    assert_eq!(
        crate_version(),
        "0.15.0",
        "coyote-rns-transport should declare the published version; bump this pin with the crate"
    );

    let consumer_line = format!("coyote-rns-transport = \"{}\"", crate_version());
    assert!(
        readme.contains(&consumer_line),
        "coyote-rns-transport README should pin the consumer line {consumer_line:?}"
    );

    for required in [
        "coyote-rns-transport-v",
        "CARGO_REGISTRY_TOKEN",
        "EPL-2.0 OR GPL-2.0-or-later",
        "rns_transport",
    ] {
        assert!(
            readme.contains(required),
            "coyote-rns-transport README should include required token {required:?}"
        );
    }

    for required in [
        "name = \"coyote-rns-transport\"",
        "version = \"0.15.0\"",
        "description = \"Coyote's published fork of reticulum-rs-transport",
        "license.workspace = true",
        "rns-core.workspace = true",
        "[lib]",
        "name = \"rns_transport\"",
        "readme = \"README.md\"",
        "repository = \"https://github.com/Dark-Alex-17/coyote-rns-transport\"",
        "documentation = \"https://docs.rs/coyote-rns-transport\"",
        "keywords = [\"reticulum\", \"transport\", \"network\", \"links\"]",
        "categories = [\"network-programming\"]",
    ] {
        assert!(
            manifest.contains(required),
            "coyote-rns-transport manifest should include required token {required:?}"
        );
    }

    let core_dependency =
        "rns-core = { package = \"reticulum-rs-core\", version = \"0.14.0\", path = \"crates/libs/rns-core\" }";
    assert!(
        workspace_manifest.contains(core_dependency),
        "workspace manifest should pin the core dependency the published crate resolves through: {core_dependency:?}"
    );
}

/// Every site that names the published crate carries the same version, and
/// both release ledgers have a section for it. On a patch release Cargo
/// cannot catch a stale dependent manifest on its own: a path dependency
/// declared as `version = "0.14.0"` is a caret requirement that 0.14.1
/// satisfies, so the manifests, the consumer snippets and the lock file are
/// pinned here.
#[test]
fn release_sites_and_ledgers_track_the_crate_version() {
    let version = crate_version();
    let version_token = format!("version = \"{version}\"");

    for manifest in
        ["Cargo.toml", "crates/apps/rns-tools/Cargo.toml", "crates/apps/reticulumd/Cargo.toml"]
    {
        let text = read_repo_file(manifest);
        let sites: Vec<&str> = text
            .lines()
            .filter(|line| line.contains("package = \"coyote-rns-transport\""))
            .collect();
        assert!(
            !sites.is_empty(),
            "{manifest} should depend on coyote-rns-transport by package name"
        );
        for site in sites {
            assert!(
                site.contains(&version_token),
                "{manifest} should require {version_token:?} on its coyote-rns-transport dependency, got {site:?}"
            );
        }
    }

    let consumer_line = format!("coyote-rns-transport = \"{version}\"");
    assert!(
        read_repo_file("docs/getting-started.md").contains(&consumer_line),
        "docs/getting-started.md should pin the consumer line {consumer_line:?}"
    );

    let lock_entry = format!("name = \"coyote-rns-transport\"\n{version_token}\n");
    assert!(
        read_repo_file("Cargo.lock").contains(&lock_entry),
        "Cargo.lock should carry the published crate at {version:?}; regenerate it with `cargo update -p coyote-rns-transport --offline`"
    );

    let changelog = read_repo_file("crates/libs/rns-transport/CHANGELOG.md");
    let release_heading = format!("\n## {version} - ");
    assert!(
        changelog.contains(&release_heading),
        "crate CHANGELOG.md should have a dated `## {version} - YYYY-MM-DD` release heading"
    );
    let heading_offset = changelog.find(&release_heading).expect("release heading located");
    let dated = &changelog[heading_offset + release_heading.len()..];
    let date: String = dated.chars().take_while(|c| !c.is_whitespace()).collect();
    assert!(
        date.len() == 10
            && date.chars().enumerate().all(|(i, c)| if i == 4 || i == 7 {
                c == '-'
            } else {
                c.is_ascii_digit()
            }),
        "release heading date should be ISO YYYY-MM-DD, got {date:?}"
    );

    let readme = read_repo_file("crates/libs/rns-transport/README.md");
    for required in ["follows cargo semver", "docs/fork/PATCHSET.md", "CHANGELOG.md"] {
        assert!(
            readme.contains(required),
            "crate README should state the versioning policy and name both ledgers; missing {required:?}"
        );
    }
}

/// The patch-set ledger's section for the current release opens with a table
/// of every file changed since the previous tag. Every path in it must exist,
/// and the files that change on every release must be listed.
#[test]
fn patch_set_ledger_lists_existing_files_for_the_current_release() {
    let version = crate_version();
    let ledger = read_repo_file("docs/fork/PATCHSET.md");
    let section_heading = format!("\n## {version} (upstream ");
    let start = ledger.find(&section_heading).unwrap_or_else(|| {
        panic!("docs/fork/PATCHSET.md should have a `## {version} (upstream <base>)` section")
    });
    let section = &ledger[start + 1..];
    let section = match section[section_heading.len() - 1..].find("\n## ") {
        Some(end) => &section[..section_heading.len() - 1 + end],
        None => section,
    };

    let root = repo_root();
    let table_paths: Vec<&str> = section
        .lines()
        .filter(|line| line.starts_with("| `"))
        .filter_map(|line| {
            let rest = line.strip_prefix("| `")?;
            rest.split('`').next()
        })
        .collect();
    assert!(
        table_paths.len() >= 10,
        "the {version} section should open with a file table, found {} rows",
        table_paths.len()
    );

    let missing: Vec<&str> =
        table_paths.iter().copied().filter(|path| !root.join(path).exists()).collect();
    assert!(
        missing.is_empty(),
        "every file in the {version} table should exist in the tree:\n{}",
        missing.join("\n")
    );

    for required in [
        "Cargo.lock",
        "crates/libs/rns-transport/Cargo.toml",
        "crates/libs/rns-transport/CHANGELOG.md",
        "crates/apps/reticulumd/tests/fork_publish_contract.rs",
        "docs/fork/PATCHSET.md",
    ] {
        assert!(
            table_paths.contains(&required),
            "the {version} table should list {required:?}, which changes on every release"
        );
    }
}

/// Root `VERSION` is the workspace number: the upstream baseline that
/// `lxmf --version` prints and that HIL evidence records. The fork publishes
/// only `coyote-rns-transport`, so a fork release must not move it; it
/// stays equal to the unpublished workspace crates until the next upstream
/// rebase.
#[test]
fn root_version_stays_on_the_upstream_baseline() {
    let root_version = read_repo_file("VERSION");
    let mut tokens = root_version.split_whitespace();
    let root_version = tokens.next().expect("VERSION should contain a version token");
    assert!(tokens.next().is_none(), "VERSION should contain exactly one token");

    fn manifest_version(manifest: &str) -> String {
        read_repo_file(manifest)
            .lines()
            .find_map(|line| line.strip_prefix("version = \""))
            .and_then(|rest| rest.strip_suffix('"'))
            .unwrap_or_else(|| panic!("{manifest} should declare a package version"))
            .to_string()
    }

    for baseline in ["crates/apps/lxmf-cli/Cargo.toml", "crates/libs/rns-core/Cargo.toml"] {
        assert_eq!(
            manifest_version(baseline),
            root_version,
            "{baseline} is an upstream-baseline crate and should carry the root VERSION; \
             only coyote-rns-transport moves on a fork release"
        );
    }

    let readme = read_repo_file("crates/libs/rns-transport/README.md");
    assert!(
        readme.contains("root `VERSION`"),
        "crate README should explain why root VERSION does not follow the crate version"
    );
}

/// Every row of the patch-set table names the patch it belongs to, and every
/// letter-digit patch id used in a row is defined by the legend above the table.
#[test]
fn patch_set_ledger_rows_name_a_patch_from_the_legend() {
    let version = crate_version();
    let ledger = read_repo_file("docs/fork/PATCHSET.md");
    let section_heading = format!("\n## {version} (upstream ");
    let start = ledger
        .find(&section_heading)
        .unwrap_or_else(|| panic!("docs/fork/PATCHSET.md should have a `## {version}` section"));
    let section = &ledger[start + 1..];
    let section = match section[section_heading.len() - 1..].find("\n## ") {
        Some(end) => &section[..section_heading.len() - 1 + end],
        None => section,
    };

    let table_start = section.find("\n| File | Patch |").unwrap_or_else(|| {
        panic!("the {version} section should open with a `| File | Patch |` table")
    });
    // The legend wraps at 80 columns, so phrases are matched on one line.
    let legend = section[..table_start].split_whitespace().collect::<Vec<_>>().join(" ");
    let legend = legend.as_str();
    let defines_a_patch_id = legend
        .split(' ')
        .any(|token| is_patch_id(token) && legend.contains(&format!("{token} is ")));
    assert!(
        defines_a_patch_id,
        "the {version} legend should define at least one patch id before the table"
    );

    let rows: Vec<(&str, &str)> = section
        .lines()
        .filter(|line| line.starts_with("| `"))
        .map(|line| {
            let cells: Vec<&str> = line.trim_matches('|').split('|').map(str::trim).collect();
            assert_eq!(cells.len(), 2, "table row should have a file and a patch cell: {line:?}");
            (cells[0], cells[1])
        })
        .collect();
    assert!(!rows.is_empty(), "the {version} table should have rows");

    for (file, patch) in &rows {
        assert!(!patch.is_empty(), "{file} row should name the patch it belongs to");
        for phrase in patch.split(", ") {
            assert!(
                legend.contains(phrase),
                "{file} row names patch {phrase:?} which the legend does not define"
            );
            for id in phrase
                .split(|c: char| !c.is_ascii_alphanumeric())
                .filter(|token| is_patch_id(token))
            {
                assert!(
                    legend.contains(&format!("{id} is ")),
                    "{file} row uses patch id {id} which the legend does not define"
                );
            }
        }
    }

    // Every release carries a Python-in-the-loop proof: at least one interop
    // fixture row is attributed to a patch named for it.
    let interop_rows: Vec<&(&str, &str)> =
        rows.iter().filter(|(file, _)| file.contains("/python_channel_interop_parts/")).collect();
    assert!(
        interop_rows.iter().any(|(_, patch)| patch.contains("Python-in-the-loop")),
        "the {version} table should attribute an interop fixture to a Python-in-the-loop patch, got {interop_rows:?}"
    );
}

/// The README rebase recipe runs the same `cargo publish` the workflow runs.
/// Neither carries a `--registry` override: the publish workflow runs on a
/// GitHub runner with the default crates.io source, and a developer whose
/// local cargo config replaces that source adds the flag on the command line
/// rather than in the repository.
#[test]
fn publish_recipe_mirrors_the_workflow_without_a_registry_override() {
    let workflow = read_repo_file(".github/workflows/publish-coyote-rns-transport.yml");
    let readme = read_repo_file("crates/libs/rns-transport/README.md");

    let dry_run = "cargo publish -p coyote-rns-transport --locked --dry-run";
    assert!(
        workflow.contains(dry_run),
        "fork publish workflow should carry the dry-run publish command {dry_run:?}"
    );
    let recipe_step = readme
        .lines()
        .find(|line| line.contains("cargo publish"))
        .expect("crate README rebase recipe should carry a cargo publish step");
    assert!(
        recipe_step.contains(dry_run),
        "README recipe should run the workflow's dry-run publish command {dry_run:?}, got {recipe_step:?}"
    );

    for (name, text) in [("publish workflow", workflow.as_str()), ("crate README", readme.as_str())]
    {
        assert!(
            !text.contains("--registry"),
            "{name} should not carry a --registry override; local source replacement is a \
             per-machine workaround, not a repository contract"
        );
    }
}

/// Release-note bullets under the current `## <version>` heading of the crate
/// CHANGELOG end in a test clause that names the file pinning the behaviour.
/// Every backticked `.rs` / `.py` path cited in that section must resolve to
/// a file in the crate tree, the reticulumd interop tests or the rns-tools
/// sources, so a renamed or deleted test file cannot leave the release notes
/// pointing at nothing.
#[test]
fn release_notes_cite_files_that_exist_in_the_tree() {
    let version = crate_version();
    let changelog = read_repo_file("crates/libs/rns-transport/CHANGELOG.md");
    let heading = format!("\n## {version} - ");
    let start = changelog.find(&heading).unwrap_or_else(|| {
        panic!("crate CHANGELOG should have a dated `## {version} - <date>` heading")
    });
    let section = &changelog[start + 1..];
    let section = match section[heading.len() - 1..].find("\n## ") {
        Some(end) => &section[..heading.len() - 1 + end],
        None => section,
    };

    let mut cited: Vec<&str> = Vec::new();
    let mut rest = section;
    while let Some(open) = rest.find('`') {
        let after = &rest[open + 1..];
        let Some(close) = after.find('`') else { break };
        let token = &after[..close];
        let is_path = (token.ends_with(".rs") || token.ends_with(".py"))
            && token.chars().all(|c| c.is_ascii_alphanumeric() || "/_.-".contains(c));
        if is_path && !cited.contains(&token) {
            cited.push(token);
        }
        rest = &after[close + 1..];
    }
    assert!(
        cited.len() >= 5,
        "the {version} release notes should carry per-bullet test clauses, found {} file cites",
        cited.len()
    );

    let root = repo_root();
    let mut tree: Vec<String> = Vec::new();
    for subtree in
        ["crates/libs/rns-transport", "crates/apps/reticulumd/tests", "crates/apps/rns-tools/src"]
    {
        visit_text_files(&root, &root.join(subtree), &mut |path| {
            let relative = path.strip_prefix(&root).unwrap_or(path);
            tree.push(relative.to_string_lossy().replace('\\', "/"));
        });
    }

    let unresolved: Vec<&str> = cited
        .iter()
        .copied()
        .filter(|token| {
            !tree.iter().any(|path| path == token || path.ends_with(&format!("/{token}")))
        })
        .collect();
    assert!(
        unresolved.is_empty(),
        "every file cited in the {version} release notes should exist under the crate, the \
         reticulumd tests or the rns-tools sources:\n{}",
        unresolved.join("\n")
    );
}

#[test]
fn repo_readme_points_at_the_crate_readme() {
    let pointer = "](crates/libs/rns-transport/README.md)";
    assert!(
        read_repo_file("README.md").contains(pointer),
        "repository README should link to the crate README with {pointer:?}"
    );
    assert!(
        repo_root().join("crates/libs/rns-transport/README.md").is_file(),
        "the crate README the repository README points at should exist"
    );
}

/// The lxmd smoke script waits for the warn's own message, `close /<32 hex>/`,
/// in the Rust lxmd log, so the needle is independent of how the tracing
/// subscriber renders the level and target. Re-applying the patch must keep
/// the message starting with `close ` followed by the link id.
#[test]
fn link_close_needle_is_derivable_from_the_surviving_warn() {
    let needle = "close /[0-9a-f]{32}/";

    let smoke = read_repo_file("tools/scripts/python-lxmd-rust-lxmd-smoke.sh");
    let waits = smoke.matches(&format!("\"{needle}\"")).count();
    assert!(
        waits >= 2,
        "lxmd smoke script should wait for the {needle:?} needle at both teardown sites, found {waits}"
    );

    let source = read_repo_file(
        "crates/libs/rns-transport/src/destination/link_parts/link_sections/next_watchdog_deadline.rs",
    );
    let warn_call = "log::warn!(\"close {}\", self.id)";
    assert!(source.contains(warn_call), "link close must keep the warn {warn_call:?}");
    assert!(!source.contains("println!"), "link close must not print to stdout");

    let literal = &warn_call[warn_call.find('"').expect("warn call opens a literal") + 1..];
    let prefix = &literal[..literal.find("{}").expect("warn literal interpolates the id")];
    assert!(
        needle.starts_with(prefix),
        "needle {needle:?} should start with the warn message prefix {prefix:?}"
    );
    assert_eq!(
        &needle[prefix.len()..],
        "/[0-9a-f]{32}/",
        "the rest of the needle should match the link id's Display: a slash, 32 lowercase hex digits, a slash"
    );
}

const OLD_PACKAGE_NAME_FORMS: [&str; 9] = [
    "-p reticulum-rs-transport",
    "--package reticulum-rs-transport",
    "\"-p\", \"reticulum-rs-transport\"",
    "\"reticulum-rs-transport\"",
    "package = \"reticulum-rs-transport\"",
    "package: \"reticulum-rs-transport\"",
    "name = \"reticulum-rs-transport\"",
    "reticulum-rs-transport = \"",
    "crates.io/crates/reticulum-rs-transport",
];

const SKIPPED_DIRECTORIES: [&str; 5] = ["target", ".git", "vendor", "artifacts", "generated"];

/// Genuinely historical subtrees under docs/: goal records, migration notes,
/// decision records, and per-release notes describe the tree as it was when
/// they were written, so the old package name is correct there. Everything
/// else under docs/ (including docs/status/, the current roadmap and parity
/// records, and docs/sdk/) is scanned so runnable commands stay current.
const SKIPPED_DOC_PREFIXES: [&str; 4] =
    ["docs/goals/", "docs/migrations/", "docs/adr/", "docs/release-notes-"];

const SCANNED_EXTENSIONS: [&str; 8] = ["rs", "toml", "yml", "yaml", "sh", "py", "md", "txt"];

fn is_skipped_doc(root: &Path, path: &Path) -> bool {
    let Ok(relative) = path.strip_prefix(root) else {
        return false;
    };
    let relative = relative.to_string_lossy();
    SKIPPED_DOC_PREFIXES.iter().any(|prefix| relative.starts_with(prefix))
}

fn visit_text_files(root: &Path, dir: &Path, f: &mut impl FnMut(&Path)) {
    let entries =
        fs::read_dir(dir).unwrap_or_else(|err| panic!("read directory {}: {err}", dir.display()));
    for entry in entries {
        let path = entry.expect("read directory entry").path();
        let name = path.file_name().and_then(|name| name.to_str()).unwrap_or_default();
        if is_skipped_doc(root, &path) {
            continue;
        }
        if path.is_dir() {
            // Dot-dirs like .coyote and .venv are local tool state; .github holds the publish workflow.
            let local_tool_dir = name.starts_with('.') && name != ".github";
            if !local_tool_dir && !SKIPPED_DIRECTORIES.contains(&name) {
                visit_text_files(root, &path, f);
            }
            continue;
        }
        if name == "Cargo.lock" {
            continue;
        }
        let scanned = path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| SCANNED_EXTENSIONS.contains(&ext));
        if scanned && path.is_file() {
            f(&path);
        }
    }
}

#[test]
fn old_package_name_survives_only_as_prose() {
    let root = repo_root();
    let this_file = root.join("crates/apps/reticulumd/tests/fork_publish_contract.rs");
    let mut offenders = Vec::new();

    visit_text_files(&root, &root, &mut |path| {
        // This file spells out the forbidden forms as needles.
        if path == this_file {
            return;
        }
        let Ok(text) = fs::read_to_string(path) else {
            return;
        };
        for (index, line) in text.lines().enumerate() {
            if OLD_PACKAGE_NAME_FORMS.iter().any(|form| line.contains(form)) {
                let relative = path.strip_prefix(&root).unwrap_or(path);
                offenders.push(format!("{}:{}: {}", relative.display(), index + 1, line.trim()));
            }
        }
    });

    assert!(
        offenders.is_empty(),
        "the upstream package name must not survive as a cargo target or manifest name:\n{}",
        offenders.join("\n")
    );
}

/// The crate README's `## Known divergences from Python RNS` register is the
/// consumer's list of where this crate is known to depart from RNS 1.5.2.
/// Every entry must point at Python with an `RNS/<file>.py:<line>` citation,
/// and the first entry is the request-timeout divergence the release notes
/// lead with, cited at `RNS/Link.py:1398` (`request_timed_out`) and `:1408`
/// (`response_rejected`).
#[test]
fn crate_readme_known_divergences_cite_python_lines() {
    let readme = read_repo_file("crates/libs/rns-transport/README.md");
    let heading = "\n## Known divergences from Python RNS\n";
    let start = readme
        .find(heading)
        .expect("crate README should carry a `## Known divergences from Python RNS` section");
    let section = &readme[start + heading.len()..];
    let section = match section.find("\n## ") {
        Some(end) => &section[..end],
        None => section,
    };

    // Entries are top-level bullets; continuation lines are indented.
    let entries: Vec<String> = section
        .split("\n- ")
        .skip(1)
        .map(|entry| entry.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect();
    assert!(
        entries.len() >= 10,
        "the divergence register should list the request-layer divergences, found {} entries",
        entries.len()
    );

    let rns_modules = ["Link", "Destination", "Transport", "Packet", "Resource", "Identity"];
    let cites_python = |entry: &str| {
        entry.match_indices("RNS/").any(|(at, _)| {
            let rest = &entry[at + "RNS/".len()..];
            rns_modules.iter().any(|module| {
                rest.strip_prefix(module)
                    .and_then(|r| r.strip_prefix(".py:"))
                    .is_some_and(|r| r.starts_with(|c: char| c.is_ascii_digit()))
            })
        })
    };
    let uncited: Vec<&str> =
        entries.iter().map(String::as_str).filter(|entry| !cites_python(entry)).collect();
    assert!(
        uncited.is_empty(),
        "every divergence entry should cite `RNS/<file>.py:<line>`:\n{}",
        uncited.join("\n")
    );

    let timeout_entry = &entries[0];
    for required in ["RequestError::Timeout", "`RNS/Link.py:1398`", "`:1408`", "DELIVERED"] {
        assert!(
            timeout_entry.contains(required),
            "the first divergence should be the request-timeout-in-any-state entry citing {required:?}, got {timeout_entry:?}"
        );
    }

    assert!(
        section.contains("\nNot divergences:\n"),
        "the register should separate API-shape facts under `Not divergences:`"
    );
}

/// A release whose minor digit moved against the previous release heading
/// carries breaking changes, and the README sends consumers to `### Breaking`
/// in the CHANGELOG for them: that subsection must open the release section
/// and every bullet in it must tell the consumer how to migrate. A release
/// that only moved the patch digit must not claim breaking changes.
#[test]
fn release_notes_list_breaking_changes_on_a_minor_bump() {
    let version = crate_version();
    let changelog = read_repo_file("crates/libs/rns-transport/CHANGELOG.md");

    fn minor(version: &str) -> (u64, u64) {
        let mut parts = version.split('.').map(|part| part.parse::<u64>().expect("numeric"));
        (parts.next().expect("major"), parts.next().expect("minor"))
    }

    let headings: Vec<(usize, &str)> = changelog
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            line.strip_prefix("## ").map(|rest| (index, rest.split(' ').next().unwrap_or(rest)))
        })
        .collect();
    let current = headings
        .iter()
        .position(|(_, heading)| *heading == version)
        .unwrap_or_else(|| panic!("CHANGELOG should have a `## {version}` heading"));
    assert_eq!(current, 0, "the current release should be the first `## ` heading");
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
        let breaking_start =
            section.iter().position(|line| *line == "### Breaking").expect("located above") + 1;
        let breaking_end = section[breaking_start..]
            .iter()
            .position(|line| line.starts_with("### "))
            .map_or(section.len(), |p| breaking_start + p);
        let breaking = &section[breaking_start..breaking_end];
        let bullets: Vec<String> = breaking
            .split(|line| line.trim().is_empty())
            .filter(|para| !para.is_empty())
            .map(|para| para.join(" "))
            .filter(|para| para.starts_with("- "))
            .collect();
        assert!(!bullets.is_empty(), "`### Breaking` should list at least one change");
        for bullet in &bullets {
            assert!(
                bullet.contains("Migration:"),
                "every breaking bullet should carry a `Migration:` clause, got {bullet:?}"
            );
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

/// Release hygiene the fork carries in CI: the publish contract runs on the
/// pull-request path, not only on the tag workflow, and the upstream Codex
/// review is opt-in through a repository variable, so a fork without the
/// OpenAI secret does not fail every PR.
#[test]
fn pull_request_ci_runs_the_publish_contract_and_codex_review_is_opt_in() {
    let ci = read_repo_file(".github/workflows/ci.yml");
    assert!(ci.contains("\non:\n  pull_request:"), "ci.yml should run on pull requests");
    assert!(
        ci.contains("run: cargo test -p reticulumd --test fork_publish_contract"),
        "ci.yml should run the fork publish contract as a step"
    );
    // The step belongs to a job with no `if:` guard, so it runs on every PR.
    let job_start = ci[..ci.find("--test fork_publish_contract").expect("step located")]
        .rfind("\n  contracts:\n")
        .expect("the publish contract step should sit in the `contracts` job");
    let job = &ci[job_start..ci.find("--test fork_publish_contract").expect("step located")];
    assert!(
        !job.lines().any(|line| line.trim_start().starts_with("if:")),
        "the `contracts` job must not be conditional, or the publish contract skips on PRs"
    );

    let codex = read_repo_file(".github/workflows/codex-review.yml");
    let guard = codex
        .lines()
        .find(|line| line.trim_start().starts_with("if:"))
        .expect("codex-review.yml should guard its job with an `if:`");
    assert!(
        guard.contains("vars.CODEX_REVIEW_ENABLED == 'true'"),
        "the Codex review job should be opt-in through the CODEX_REVIEW_ENABLED variable, got {guard:?}"
    );
    assert!(
        !guard.contains("secrets."),
        "a job-level `if:` cannot read secrets; the gate must be a repository variable"
    );
}

/// Every `rust` snippet in the crate README is pinned verbatim by one of the
/// public-API test files, so a README edit that drifts from what compiles and
/// runs turns this test red. `use` lines are left to the compiler of those
/// tests; every other line of a snippet must appear, whitespace aside, in a
/// pin file, with a `match` arm compared up to its `=>` because the pins
/// reduce the README's log calls to labels (and may bind an unused pattern
/// variable with a leading underscore).
#[test]
fn crate_readme_rust_snippets_are_pinned_verbatim_by_the_api_tests() {
    let readme = read_repo_file("crates/libs/rns-transport/README.md");
    let pins: String = [
        "crates/libs/rns-transport/tests/request_receipt_readme.rs",
        "crates/libs/rns-transport/tests/request_handler_api.rs",
        "crates/libs/rns-transport/tests/outbound_outcome_api.rs",
        "crates/libs/rns-transport/tests/request_receipt_api.rs",
    ]
    .iter()
    .map(|path| read_repo_file(path))
    .collect::<Vec<_>>()
    .join("\n");
    let squash = |text: &str| {
        text.chars().filter(|c| !c.is_whitespace()).collect::<String>().replace("(_", "(")
    };
    let pins = squash(&pins);

    let mut snippets = Vec::new();
    let mut current: Option<Vec<&str>> = None;
    for line in readme.lines() {
        match (&mut current, line.trim_start()) {
            (None, "```rust") => current = Some(Vec::new()),
            (Some(lines), "```") => snippets.push(lines.clone()),
            (Some(lines), _) => lines.push(line),
            _ => {}
        }
        if line.trim_start() == "```" {
            current = None;
        }
    }
    assert!(
        snippets.len() >= 4,
        "the README should carry the consumer, handler, request and outcome snippets, found {}",
        snippets.len()
    );

    let mut unpinned = Vec::new();
    for snippet in &snippets {
        for line in snippet {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with("use ") || trimmed.starts_with("//") {
                continue;
            }
            let needle = match trimmed.split_once(" => ") {
                Some((pattern, _)) => squash(pattern),
                None => squash(trimmed),
            };
            if !pins.contains(&needle) {
                unpinned.push(trimmed.to_string());
            }
        }
    }
    assert!(
        unpinned.is_empty(),
        "every README snippet line should appear verbatim in a public-API pin test:\n{}",
        unpinned.join("\n")
    );
}

/// Every enum the README or the release notes call `#[non_exhaustive]` carries
/// the attribute on its definition, so the "keep a wildcard arm" guidance the
/// consumer reads is what the compiler enforces.
#[test]
fn enums_the_docs_call_non_exhaustive_carry_the_attribute() {
    let sites = [
        ("RequestError", "crates/libs/rns-transport/src/destination/request/receipt.rs"),
        ("RequestStatus", "crates/libs/rns-transport/src/destination/request/receipt.rs"),
        ("RequestAllow", "crates/libs/rns-transport/src/destination/request/allow.rs"),
        ("RequestResponse", "crates/libs/rns-transport/src/destination/request/handler.rs"),
        ("RnsError", "crates/libs/rns-transport/src/error.rs"),
        ("OutboundResult", "crates/libs/rns-transport/src/resource/outcome.rs"),
        ("OutboundFailure", "crates/libs/rns-transport/src/resource/outcome.rs"),
        ("RejectReason", "crates/libs/rns-transport/src/resource/outcome.rs"),
        ("LinkEvent", "crates/libs/rns-transport/src/destination/link_parts/linkhandleresult.rs"),
        ("ResourceEventKind", "crates/libs/rns-transport/src/resource.rs"),
        ("AnnounceEvent", "crates/libs/rns-transport/src/transport/mod.rs"),
    ];
    for (item, file) in sites {
        let source = read_repo_file(file);
        let definition = source
            .find(&format!("pub enum {item} "))
            .or_else(|| source.find(&format!("pub struct {item} ")))
            .unwrap_or_else(|| panic!("{file} should define `{item}`"));
        // The attribute block sits between the previous blank line and the definition.
        let attributes = &source[..definition];
        let attributes = &attributes[attributes.rfind("\n\n").map_or(0, |at| at + 2)..];
        assert!(
            attributes.lines().any(|line| line.trim() == "#[non_exhaustive]"),
            "{file}: `{item}` is documented as `#[non_exhaustive]` but its definition lacks the attribute:\n{attributes}"
        );
    }
}
