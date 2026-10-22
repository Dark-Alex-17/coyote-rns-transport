// Behavioural usage pins for the fork release scripts that need git state
// the developer's checkout must not be mutated for: each test clones this
// repository locally (object store shared, about a second), edits tags or
// files in the clone only, and runs the clone's copy of the script. The
// working-tree scripts and ledger are copied over the clone's so an edit
// under test is what runs. The clone is removed when the test ends,
// panicking or not.

/// A throwaway local clone of this repository at the same commit as HEAD,
/// removed on drop.
#[cfg(unix)]
struct ProbeClone {
    root: PathBuf,
}

#[cfg(unix)]
impl ProbeClone {
    fn new(label: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "fork-release-probe-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let status = Command::new("git")
            .args(["clone", "-q", "--local"])
            .arg(repo_root())
            .arg(&root)
            .status()
            .expect("run git clone");
        assert!(status.success(), "git clone --local of the repository should succeed");
        let clone = Self { root };
        // The probes create commits (`commit-tree`), and a CI runner has no
        // git identity to auto-detect; the clone carries its own so the
        // probes do not depend on the host's.
        clone.git(&["config", "user.name", "probe"]);
        clone.git(&["config", "user.email", PROBE_CLONE_EMAIL]);
        // Detach at the source HEAD so a detached source or a branch-less
        // clone cannot leave the working tree somewhere else.
        let head = git_stdout(&repo_root(), &["rev-parse", "HEAD"]);
        clone.git(&["checkout", "-q", "--detach", &head]);
        for relative in [
            "tools/scripts/patchset-ledger-check.sh",
            "tools/scripts/old-name-tracked-scan.sh",
            "docs/fork/PATCHSET.md",
        ] {
            fs::copy(repo_root().join(relative), clone.path(relative)).unwrap_or_else(|err| {
                panic!("copy the working-tree {relative} into the clone: {err}")
            });
        }
        clone
    }

    fn git(&self, args: &[&str]) -> String {
        git_stdout(&self.root, args)
    }

    /// `git <args>` in the clone with `env` added to the process environment.
    fn git_with_env(&self, args: &[&str], env: &[(&str, &str)]) -> String {
        git_stdout_with_env(&self.root, args, env)
    }

    /// Runs `tools/scripts/<script>` from the clone and returns
    /// `(exit code, stdout, stderr)`.
    fn run_script(&self, script: &str, args: &[&str]) -> (Option<i32>, String, String) {
        let output = Command::new("bash")
            .arg(self.root.join("tools/scripts").join(script))
            .args(args)
            .current_dir(&self.root)
            .output()
            .unwrap_or_else(|err| panic!("run tools/scripts/{script} in the clone: {err}"));
        (
            output.status.code(),
            String::from_utf8_lossy(&output.stdout).into_owned(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
        )
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }

    fn fork_tags(&self) -> Vec<String> {
        self.git(&["tag", "--list", "coyote-rns-transport-v*"])
            .lines()
            .map(str::to_string)
            .collect()
    }

    /// The script's own precondition for its zero-argument form: a fork tag
    /// that is an ancestor of HEAD. Tags on other branches do not count.
    fn has_a_reachable_fork_tag(&self) -> bool {
        !self
            .git(&["tag", "--list", "coyote-rns-transport-v*", "--merged", "HEAD"])
            .trim()
            .is_empty()
    }
}

#[cfg(unix)]
const PROBE_CLONE_EMAIL: &str = "probe@example.invalid";

#[cfg(unix)]
impl Drop for ProbeClone {
    fn drop(&mut self) {
        // Best-effort cleanup of a temp dir: a failure here must not mask the
        // assertion that is already unwinding.
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[cfg(unix)]
fn git_stdout(dir: &Path, args: &[&str]) -> String {
    git_stdout_with_env(dir, args, &[])
}

#[cfg(unix)]
fn git_stdout_with_env(dir: &Path, args: &[&str], env: &[(&str, &str)]) -> String {
    let mut command = Command::new("git");
    command.args(args);
    // An identity the test process inherited through the environment would
    // satisfy git before any config is consulted, so a probe commit would
    // pass here on a host that exports one and fail on a runner that does
    // not; the probes commit on the clone-local identity alone.
    for variable in [
        "GIT_AUTHOR_NAME",
        "GIT_AUTHOR_EMAIL",
        "GIT_COMMITTER_NAME",
        "GIT_COMMITTER_EMAIL",
        "EMAIL",
    ] {
        command.env_remove(variable);
    }
    let output = command
        .envs(env.iter().copied())
        .current_dir(dir)
        .output()
        .unwrap_or_else(|err| panic!("run git {args:?}: {err}"));
    assert!(
        output.status.success(),
        "git {args:?} should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// The probe clone's commits are signed by the clone's own identity, not
/// the host's: with the global and system git config out of reach and
/// identity auto-detection disabled (what a CI runner without a configured
/// identity amounts to), `commit-tree` still succeeds and the commit's
/// author and committer are the clone-local ones.
#[cfg(unix)]
#[test]
fn probe_clone_commits_without_a_global_git_identity() {
    let clone = ProbeClone::new("clone-identity");
    // `user.useConfigOnly` closes the auto-detection route a host with a
    // fully qualified hostname would otherwise take; without it a missing
    // clone-local identity would go unnoticed on such a host.
    let no_host_identity = [
        ("GIT_CONFIG_GLOBAL", "/dev/null"),
        ("GIT_CONFIG_NOSYSTEM", "1"),
        ("GIT_CONFIG_COUNT", "1"),
        ("GIT_CONFIG_KEY_0", "user.useConfigOnly"),
        ("GIT_CONFIG_VALUE_0", "true"),
    ];
    let commit =
        clone.git_with_env(&["commit-tree", "-m", "probe", "HEAD^{tree}"], &no_host_identity);
    assert_eq!(commit.len(), 40, "commit-tree prints the new commit's hash, got {commit:?}");
    let identities = clone.git(&["log", "-1", "--format=%ae %ce", &commit]);
    assert_eq!(
        identities,
        format!("{PROBE_CLONE_EMAIL} {PROBE_CLONE_EMAIL}"),
        "author and committer should be the clone-local identity"
    );
}

/// The zero-argument ledger check takes as its base the newest
/// `coyote-rns-transport-v*` tag that is an ancestor of HEAD: a higher
/// release tag on a commit HEAD does not contain is ignored, a higher tag on
/// an ancestor is chosen (and the check then fails, naming it), and with no
/// fork tag reachable at all the check fails with the fetch-depth hint.
#[cfg(unix)]
#[test]
fn zero_argument_ledger_check_bases_on_the_newest_fork_tag_reachable_from_head() {
    let clone = ProbeClone::new("ledger-tags");
    if !clone.has_a_reachable_fork_tag() {
        eprintln!("skipping: no coyote-rns-transport-v* tag reachable from HEAD in this checkout");
        return;
    }
    let (code, stdout, stderr) = clone.run_script("patchset-ledger-check.sh", &[]);
    assert_eq!(code, Some(0), "clean clone should pass the zero-argument check: {stderr}");
    let newest_reachable = stdout
        .split("since ")
        .nth(1)
        .and_then(|rest| rest.split(';').next())
        .expect("the ok line names the base tag")
        .to_string();

    // A release tag on a commit that is not an ancestor of HEAD.
    let dangling = clone.git(&["commit-tree", "-m", "probe", "HEAD^{tree}"]);
    clone.git(&["tag", "coyote-rns-transport-v9.9.9", &dangling]);
    let (code, stdout, stderr) = clone.run_script("patchset-ledger-check.sh", &[]);
    assert_eq!(code, Some(0), "an unmerged release tag must not become the base: {stderr}");
    assert!(
        stdout.contains(&format!("since {newest_reachable};")),
        "the base should stay {newest_reachable}, got {stdout:?}"
    );
    clone.git(&["tag", "-d", "coyote-rns-transport-v9.9.9"]);

    // The same tag on an ancestor is the newest reachable one and is chosen.
    clone.git(&["tag", "coyote-rns-transport-v9.9.9", "HEAD~1"]);
    let (code, _, stderr) = clone.run_script("patchset-ledger-check.sh", &[]);
    assert_eq!(
        code,
        Some(1),
        "a reachable higher tag becomes the base; the ledger has no 9.9.9 heading to end the section, so the check fails naming the range"
    );
    assert!(
        stderr.contains("no '## 9.9.9 (upstream ...)' heading"),
        "the failure should name the missing 9.9.9 heading, got {stderr:?}"
    );
    assert!(
        stderr.contains("coyote-rns-transport-v9.9.9..HEAD"),
        "the failure should name the chosen base, got {stderr:?}"
    );
    clone.git(&["tag", "-d", "coyote-rns-transport-v9.9.9"]);

    // No fork tag at all.
    for tag in clone.fork_tags() {
        clone.git(&["tag", "-d", &tag]);
    }
    let (code, _, stderr) = clone.run_script("patchset-ledger-check.sh", &[]);
    assert_eq!(code, Some(1), "with no reachable fork tag the check must fail, got {stderr:?}");
    assert!(
        stderr.contains("no coyote-rns-transport-v* tag is reachable from HEAD")
            && stderr.contains("fetch-depth: 0"),
        "the failure should carry the fetch-depth hint, got {stderr:?}"
    );
}

/// The commit-cite half of the ledger check reads the named section only: a
/// hash that is cited anywhere else in `docs/fork/PATCHSET.md` but not in
/// the section under check is an uncited commit.
#[cfg(unix)]
#[test]
fn ledger_check_cites_commits_per_section_not_per_file() {
    let clone = ProbeClone::new("ledger-section");
    if !clone.has_a_reachable_fork_tag() {
        eprintln!("skipping: no coyote-rns-transport-v* tag reachable from HEAD in this checkout");
        return;
    }
    let (code, _, stderr) = clone.run_script("patchset-ledger-check.sh", &[]);
    assert_eq!(code, Some(0), "clean clone should pass first: {stderr}");

    // The zero-argument run checks the newest section, whatever the current
    // release state labels it.
    let label = newest_ledger_label();
    let ledger_path = clone.path("docs/fork/PATCHSET.md");
    let ledger = fs::read_to_string(&ledger_path).expect("read the clone's ledger");
    let start = ledger
        .find(&format!("\n## {label} (upstream"))
        .unwrap_or_else(|| panic!("the ledger has a `## {label} (upstream ...)` section"))
        + 1;
    let after_heading = start + ledger[start..].find('\n').expect("heading line ends") + 1;
    let end =
        ledger[after_heading..].find("\n## ").map_or(ledger.len(), |rel| after_heading + rel + 1);
    let section = &ledger[start..end];
    let cites: Vec<&str> = section
        .split('`')
        .filter(|token| token.len() == 8 && token.chars().all(|c| c.is_ascii_hexdigit()))
        .collect();
    if cites.is_empty() {
        eprintln!("skipping: the {label} section cites no commit");
        return;
    }

    // Scrub every cite from the section and append them all, verbatim, to
    // the section that follows, so the file as a whole still names each.
    let mut scrubbed = section.to_string();
    for cite in &cites {
        scrubbed = scrubbed.replace(&format!("`{cite}`"), "`00000000`");
    }
    let moved: Vec<String> = cites.iter().map(|cite| format!("`{cite}`")).collect();
    let mut mutated = String::new();
    mutated.push_str(&ledger[..start]);
    mutated.push_str(&scrubbed);
    let tail = &ledger[end..];
    match tail.find('\n') {
        Some(heading_end) => {
            mutated.push_str(&tail[..=heading_end]);
            mutated.push_str(&format!("\nProbe: {}.\n", moved.join(", ")));
            mutated.push_str(&tail[heading_end + 1..]);
        }
        None => {
            mutated.push_str(tail);
            mutated.push_str(&format!(
                "\n## 0.0.0 (upstream probe)\n\nProbe: {}.\n",
                moved.join(", ")
            ));
        }
    }
    fs::write(&ledger_path, mutated).expect("write the mutated ledger");

    let (code, _, stderr) = clone.run_script("patchset-ledger-check.sh", &[]);
    assert_eq!(code, Some(1), "a hash cited only outside the section must fail: {stderr}");
    assert!(stderr.contains("uncited commit "), "the failure names the uncited commit: {stderr:?}");
    assert!(
        stderr.contains(&format!("the {label} section of docs/fork/PATCHSET.md does not cite")),
        "the failure names the section, got {stderr:?}"
    );
}

/// `old-name-tracked-scan.sh` reports a runnable old-name form in a tracked
/// scanned file as `path:line:text` and exits 1; a form in an untracked
/// file, or in a tracked file under a skipped doc prefix, is not a finding.
#[cfg(unix)]
#[test]
fn old_name_tracked_scan_reports_tracked_hits_only() {
    let clone = ProbeClone::new("old-name-scan");
    // Assembled from parts so this file carries no runnable old-name form.
    let old = ["reticulum", "rs", "transport"].join("-");
    let needle = format!("cargo test -p {old}");

    let (code, stdout, stderr) = clone.run_script("old-name-tracked-scan.sh", &[]);
    assert_eq!(code, Some(0), "clean clone should pass: {stdout}{stderr}");
    assert!(stdout.starts_with("old-name scan ok:"), "{stdout:?}");

    // Untracked file: not a finding.
    let untracked = clone.path("probe-untracked.md");
    fs::write(&untracked, format!("{needle}\n")).expect("write untracked probe");
    let (code, _, _) = clone.run_script("old-name-tracked-scan.sh", &[]);
    assert_eq!(code, Some(0), "an untracked file is outside the tracked-files scan");
    fs::remove_file(&untracked).expect("remove untracked probe");

    // Tracked file under a dot-directory the Rust twin keeps: a finding,
    // reported with its path and line.
    let workflow = clone.path(".github/workflows/ci.yml");
    let original = fs::read_to_string(&workflow).expect("read ci.yml");
    let lines_before = original.lines().count();
    fs::write(&workflow, format!("{original}# {needle}\n")).expect("append to ci.yml");
    let (code, stdout, _) = clone.run_script("old-name-tracked-scan.sh", &[]);
    assert_eq!(code, Some(1), "a tracked hit must fail the scan, got {stdout:?}");
    assert!(
        stdout.contains(&format!(".github/workflows/ci.yml:{}:# {needle}", lines_before + 1)),
        "the hit should be reported as path:line:text, got {stdout:?}"
    );
    assert!(!stdout.contains("old-name scan ok"), "no ok line on a failing scan: {stdout:?}");
    fs::write(&workflow, &original).expect("restore ci.yml");

    // Tracked file under a skipped historical doc prefix: not a finding.
    let skipped = clone.git(&["ls-files", "docs/goals/*.md"]).lines().next().map(str::to_string);
    if let Some(skipped) = skipped {
        let path = clone.path(&skipped);
        let original = fs::read_to_string(&path).expect("read skipped doc");
        fs::write(&path, format!("{original}\n{needle}\n")).expect("append to skipped doc");
        let (code, stdout, _) = clone.run_script("old-name-tracked-scan.sh", &[]);
        assert_eq!(code, Some(0), "{skipped} is under a skipped doc prefix, got {stdout:?}");
    }
}
