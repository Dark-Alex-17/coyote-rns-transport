#[cfg(test)]
mod release_version_tests {
    use super::resolve_release_version;

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
}
