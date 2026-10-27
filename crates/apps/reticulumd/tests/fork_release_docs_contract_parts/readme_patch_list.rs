// Crate README patch-list pins: the 0.17.0 and 0.16.0 blocks and their order
// above the 0.15.1 history, and the Codex bullet whose removal pointer the
// 0.16.0 release notes resolve.

/// The Codex review workflow was removed after 0.15.1, so the README register
/// that records 0.15.0 adding it must say so in the same bullet: a reader of
/// the register must not be left believing the `CODEX_REVIEW_ENABLED`
/// variable still gates anything. Every mention of the variable in the README
/// sits in such a bullet; a mention in free prose has no removal note. The
/// bullet names 0.16.0 as the release whose `### Removed` records it, now
/// that the release exists, rather than describing it by position.
#[test]
fn readme_names_the_codex_variable_only_in_a_bullet_that_says_removed() {
    let readme = read_repo_file("crates/libs/rns-transport/README.md");
    let codex_bullets: Vec<String> = top_level_bullets(&readme)
        .into_iter()
        .filter(|bullet| bullet.contains("CODEX_REVIEW_ENABLED"))
        .collect();
    assert!(
        !codex_bullets.is_empty(),
        "the crate README should record the Codex review workflow in a bullet of the 0.15.0 register"
    );
    for bullet in &codex_bullets {
        let unwrapped = bullet.split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(
            unwrapped.contains("workflow was removed"),
            "a README bullet naming CODEX_REVIEW_ENABLED should say the workflow was removed:\n{bullet}"
        );
        assert!(
            unwrapped.contains("records it under `### Removed` in 0.16.0"),
            "a README bullet naming CODEX_REVIEW_ENABLED should name 0.16.0 as the release that records the removal:\n{bullet}"
        );
    }
    assert!(
        !readme.contains("first release after 0.15.1"),
        "the README should name 0.16.0 rather than describe it as the release after 0.15.1"
    );
    let mentions_in_bullets: usize =
        codex_bullets.iter().map(|bullet| bullet.matches("CODEX_REVIEW_ENABLED").count()).sum();
    assert_eq!(
        readme.matches("CODEX_REVIEW_ENABLED").count(),
        mentions_in_bullets,
        "every README mention of CODEX_REVIEW_ENABLED should sit in a bullet that says the workflow was removed"
    );
}

/// The README's Codex bullet sends the reader to `### Removed` in the 0.16.0
/// release notes; that pointer resolves: the CHANGELOG `## 0.16.0` section has
/// a `### Removed` subsection and one of its top-level bullets records the
/// Codex review workflow. Read from the CHANGELOG, not from the README, so a
/// release note that moved or dropped the bullet turns this red while the
/// README-only pin above stays green.
#[test]
fn changelog_0_16_0_removed_records_the_codex_workflow_the_readme_points_at() {
    let changelog = read_repo_file("crates/libs/rns-transport/CHANGELOG.md");
    let lines: Vec<&str> = changelog.lines().collect();
    let start = lines
        .iter()
        .position(|line| line.starts_with("## 0.16.0"))
        .expect("CHANGELOG should have a `## 0.16.0` heading");
    let end = lines[start + 1..]
        .iter()
        .position(|line| line.starts_with("## "))
        .map_or(lines.len(), |p| start + 1 + p);
    let section = &lines[start + 1..end];
    let removed_start = section
        .iter()
        .position(|line| *line == "### Removed")
        .expect("the 0.16.0 release notes should have a `### Removed` subsection")
        + 1;
    let removed_end = section[removed_start..]
        .iter()
        .position(|line| line.starts_with("### "))
        .map_or(section.len(), |p| removed_start + p);
    let removed = section[removed_start..removed_end].join("\n");
    let codex: Vec<String> = top_level_bullets(&removed)
        .into_iter()
        .filter(|bullet| bullet.contains("Codex") && bullet.contains("review workflow"))
        .collect();
    assert_eq!(codex.len(), 1, "exactly one `### Removed` bullet of 0.16.0 records the Codex review workflow, got {codex:?}");
}

/// What the README's 0.16.0 patch-list block says, one needle per item.
const README_0_16_0_BLOCK_NEEDLES: [&str; 14] = [
    "SharedInstanceEndpointNotLoopback",
    "allow_remote_endpoint(true)",
    "replaces the 0.15.1 once-per-attach warning",
    "RejectReason::{Oversize, Unknown}",
    "transport::test_bridge",
    "replacing the 0.15.1 rule that it consulted nothing while attached",
    "LINKCLOSE a `Handshake` teardown",
    "The reticulumd migration",
    "in the family `prefer_ipv6` selects",
    "clippy::await_holding_invalid_type",
    "Codex review workflow",
    "receipt-less records",
    "packet hashlist flush",
    "`TcpClient` connect is cancel-aware",
];

/// The demoted 0.16.0 block sits above the 0.15.1 and 0.15.0 blocks as
/// history: it still names every item of the release, and the two 0.15.1
/// sentences it supersedes (the no-op duplicate filter and the once-per-attach
/// warning) survive verbatim in the 0.15.1 block rather than being edited
/// away.
#[test]
fn readme_patch_list_keeps_the_0_16_0_block_above_the_0_15_1_paragraph() {
    let readme = read_repo_file("crates/libs/rns-transport/README.md");
    let patch_list = crate_readme_section(&readme, "Patch list")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let marker_0_16_0 = "What `0.16.0` carries on top of `0.15.1`:";
    let marker_0_15_1 = "What `0.15.1` carries on top of `0.15.0`:";
    let marker_0_15_0 = "What `0.15.0` carries on top of `0.14.1`:";
    let at_0_16_0 =
        patch_list.find(marker_0_16_0).expect("the README patch list should keep the 0.16.0 block");
    let at_0_15_1 =
        patch_list.find(marker_0_15_1).expect("the README patch list should keep the 0.15.1 block");
    let at_0_15_0 =
        patch_list.find(marker_0_15_0).expect("the README patch list should keep the 0.15.0 block");
    assert!(
        at_0_16_0 < at_0_15_1 && at_0_15_1 < at_0_15_0,
        "the README patch list should run 0.16.0, then 0.15.1, then 0.15.0"
    );

    let block_0_16_0 = &patch_list[at_0_16_0..at_0_15_1];
    for needle in README_0_16_0_BLOCK_NEEDLES {
        assert!(
            block_0_16_0.contains(needle),
            "the README 0.16.0 block should still say {needle:?}:\n{block_0_16_0}"
        );
    }
    assert!(
        !block_0_16_0.contains("flushes only on a seen disconnect"),
        "the README 0.16.0 block should no longer attribute the reconnect consumer to this release:\n{block_0_16_0}"
    );

    let block_0_15_1 = &patch_list[at_0_15_1..at_0_15_0];
    for superseded in [
        "inserts nothing and consults nothing while attached",
        "A warning, once per attach, when a `Tcp` shared-instance endpoint is not loopback.",
    ] {
        assert!(
            block_0_15_1.contains(superseded),
            "the README 0.15.1 block should keep its superseded sentence {superseded:?} as history:\n{block_0_15_1}"
        );
    }
}

/// The README patch list opens with the 0.17.0 block, which names every item
/// of the release and tells consumers `new_from_hex_string` has validated its
/// input since 0.14.1, and runs 0.16.0, then 0.15.1 below it; the demoted
/// 0.16.0 block's content is pinned by the test above.
#[test]
fn readme_patch_list_opens_with_the_0_17_0_block_and_keeps_the_0_16_0_paragraph() {
    let readme = read_repo_file("crates/libs/rns-transport/README.md");
    let patch_list = crate_readme_section(&readme, "Patch list")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let marker_0_17_0 = "What `0.17.0` carries on top of `0.16.0`:";
    let marker_0_16_0 = "What `0.16.0` carries on top of `0.15.1`:";
    let marker_0_15_1 = "What `0.15.1` carries on top of `0.15.0`:";
    let at_0_17_0 = patch_list
        .find(marker_0_17_0)
        .expect("the README patch list should open with the 0.17.0 block");
    let at_0_16_0 =
        patch_list.find(marker_0_16_0).expect("the README patch list should keep the 0.16.0 block");
    let at_0_15_1 =
        patch_list.find(marker_0_15_1).expect("the README patch list should keep the 0.15.1 block");
    assert!(
        at_0_17_0 < at_0_16_0 && at_0_16_0 < at_0_15_1,
        "the README patch list should run 0.17.0, then 0.16.0, then 0.15.1"
    );

    let block_0_17_0 = &patch_list[at_0_17_0..at_0_16_0];
    for needle in [
        "Transport::destination_desc",
        "InboundRequest::track_outcome",
        "ResponseOutcome",
        "SharedInstanceOptions::with_state_events",
        "SharedInstanceEvent",
        "with_connect_events",
        "AddressHash::new_from_hex_string",
        "since 0.14.1",
    ] {
        assert!(
            block_0_17_0.contains(needle),
            "the README 0.17.0 block should say {needle:?}:\n{block_0_17_0}"
        );
    }
}
