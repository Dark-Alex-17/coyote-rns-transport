#[cfg(test)]
mod release_version_tests {
    use super::{crate_manifest_version, resolve_release_version, Xtask, XtaskCommand};
    use clap::Parser;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static MANIFEST_COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn version_of_manifest(contents: &str) -> anyhow::Result<String> {
        let path = std::env::temp_dir().join(format!(
            "xtask-release-version-{}-{}.toml",
            std::process::id(),
            MANIFEST_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&path, contents).expect("write manifest fixture");
        let result = crate_manifest_version(path.to_str().expect("utf-8 temp path"));
        std::fs::remove_file(&path).expect("remove manifest fixture");
        result
    }

    #[test]
    fn crate_manifest_version_reads_the_literal_package_version() {
        assert_eq!(
            version_of_manifest("[package]\nname = \"demo\"\nversion = \"1.2.3\"\n")
                .expect("literal version"),
            "1.2.3"
        );
    }

    #[test]
    fn crate_manifest_version_rejects_a_package_without_version() {
        let error = version_of_manifest("[package]\nname = \"demo\"\n")
            .expect_err("missing version must fail");
        assert!(error.to_string().contains("no [package] version string"), "{error}");
    }

    #[test]
    fn crate_manifest_version_rejects_unparsable_toml() {
        let error = version_of_manifest("[package\nversion = \"1.2.3\"\n")
            .expect_err("broken toml must fail");
        // The toml crate's own message also says "parse"; the fixture path
        // is what the xtask context adds.
        let message = error.to_string();
        assert!(message.contains("parse "), "{message}");
        assert!(message.contains("xtask-release-version-"), "{message}");
    }

    #[test]
    fn crate_manifest_version_rejects_a_missing_manifest() {
        let path = std::env::temp_dir().join(format!(
            "xtask-release-version-missing-{}-{}.toml",
            std::process::id(),
            MANIFEST_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let error = crate_manifest_version(path.to_str().expect("utf-8 temp path"))
            .expect_err("missing manifest must fail");
        assert!(error.to_string().contains("read "), "{error}");
    }

    #[test]
    fn explicit_release_version_wins() {
        assert_eq!(
            resolve_release_version(Some(" custom/1 "), Some("v0.2.3"), "0.2.3", "0.9.9")
                .expect("explicit version"),
            "custom-1"
        );
    }

    #[test]
    fn matching_exact_tag_is_used() {
        assert_eq!(
            resolve_release_version(None, Some("v0.2.3"), "0.2.3", "0.9.9").expect("matching tag"),
            "v0.2.3"
        );
    }

    #[test]
    fn mismatched_exact_tag_is_rejected() {
        let error = resolve_release_version(None, Some("v0.2.4"), "0.2.3", "0.9.9")
            .expect_err("mismatched tag must fail");
        assert!(error.to_string().contains("does not match project VERSION"));
    }

    #[test]
    fn project_version_is_used_without_exact_tag() {
        assert_eq!(
            resolve_release_version(None, None, "0.2.3\n", "0.9.9").expect("VERSION fallback"),
            "0.2.3"
        );
    }

    #[test]
    fn fork_tag_matching_the_crate_version_is_returned_verbatim() {
        assert_eq!(
            resolve_release_version(
                None,
                Some("coyote-rns-transport-v0.15.1"),
                "0.14.0",
                "0.15.1\n"
            )
            .expect("fork tag matching the crate version"),
            "coyote-rns-transport-v0.15.1"
        );
    }

    #[test]
    fn fork_tag_with_another_version_is_rejected_naming_the_crate_version() {
        let error =
            resolve_release_version(None, Some("coyote-rns-transport-v0.15.2"), "0.14.0", "0.15.1")
                .expect_err("mismatched fork tag must fail");
        let message = error.to_string();
        assert!(message.contains("coyote-rns-transport crate version '0.15.1'"), "{message}");
        assert!(!message.contains("project VERSION"), "{message}");
    }

    #[test]
    fn plain_tag_is_compared_against_version_not_the_crate_version() {
        assert_eq!(
            resolve_release_version(None, Some("v0.14.0"), "0.14.0", "0.15.1").expect("daemon tag"),
            "v0.14.0"
        );
        let error = resolve_release_version(None, Some("v0.15.1"), "0.14.0", "0.15.1")
            .expect_err("a plain tag at the crate version must still fail against VERSION");
        assert!(error.to_string().contains("does not match project VERSION"));
    }

    #[test]
    fn fork_tag_never_compares_against_version_even_when_it_matches() {
        let error =
            resolve_release_version(None, Some("coyote-rns-transport-v0.14.0"), "0.14.0", "0.15.1")
                .expect_err("fork tag must be checked against the crate version only");
        assert!(error.to_string().contains("coyote-rns-transport crate version '0.15.1'"));
        assert_eq!(
            resolve_release_version(None, Some("coyote-rns-transport-v0.14.0"), "0.9.9", "0.14.0")
                .expect("fork tag matching the crate version with a different VERSION"),
            "coyote-rns-transport-v0.14.0"
        );
    }

    /// The rustdoc promises that root `VERSION` must still be one non-empty
    /// token on a flag-less run even when a fork tag matches the crate
    /// version, because the token check precedes the tag arms. An empty or a
    /// two-token `VERSION` therefore fails with the token error, not with a
    /// fork-tag verdict and not with an accepted label.
    #[test]
    fn fork_tag_does_not_excuse_a_malformed_version_token() {
        for project_version in ["", "   \n", "0.14.0 extra\n", "0.14.0\n0.14.1\n"] {
            let error = match resolve_release_version(
                None,
                Some("coyote-rns-transport-v0.15.1"),
                project_version,
                "0.15.1",
            ) {
                Ok(label) => panic!(
                    "VERSION {project_version:?} must fail the token check, got label {label:?}"
                ),
                Err(error) => error,
            };
            let message = error.to_string();
            assert!(
                message.contains("VERSION must contain exactly one non-empty version token"),
                "VERSION {project_version:?}: {message}"
            );
            assert!(
                !message.contains("coyote-rns-transport crate version"),
                "the token check should fire before the fork-tag arm, got {message}"
            );
        }
    }

    /// The rustdoc's escape hatch for a commit carrying tags from both
    /// families: an explicit version wins over whichever tag `git describe`
    /// picked, and over a malformed `VERSION`, since it is read first.
    #[test]
    fn explicit_version_wins_over_a_fork_tag_and_a_malformed_version_file() {
        assert_eq!(
            resolve_release_version(
                Some("v0.14.0"),
                Some("coyote-rns-transport-v0.15.1"),
                "",
                "0.15.1"
            )
            .expect("the explicit version is resolved before VERSION is inspected"),
            "v0.14.0"
        );
        assert_eq!(
            resolve_release_version(
                Some(" coyote-rns-transport-v0.15.1 "),
                Some("v0.14.0"),
                "0.14.0",
                "0.15.1"
            )
            .expect("the explicit fork label is returned as written"),
            "coyote-rns-transport-v0.15.1"
        );
    }

    /// The flag the rustdoc tells a releaser to pass is spelled `--version`
    /// on `package-daemon-bundle`; a rename of the clap field would silently
    /// strand that instruction.
    #[test]
    fn package_daemon_bundle_takes_the_release_label_as_dash_dash_version() {
        let parsed =
            Xtask::try_parse_from(["xtask", "package-daemon-bundle", "--version", "custom/1"])
                .expect("--version is the release-label flag");
        match parsed.command {
            XtaskCommand::PackageDaemonBundle { version, target } => {
                assert_eq!(version.as_deref(), Some("custom/1"));
                assert_eq!(target, None);
            }
            _ => panic!("package-daemon-bundle should parse to PackageDaemonBundle"),
        }
    }
}
