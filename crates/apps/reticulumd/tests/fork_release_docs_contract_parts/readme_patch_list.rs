// Crate README patch-list pins: the 0.18.3, 0.18.2, 0.18.1, 0.18.0, 0.17.0
// and 0.16.0 blocks and their order above the 0.15.1 history, and the Codex
// bullet whose removal pointer the 0.16.0 release notes resolve, plus the
// shared-instance paragraph, register bullet and changelog bullet for the
// AutoInterface peer count and the same three for the rnsd RPC client.

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

/// What the README's 0.17.0 patch-list block says, one needle per item.
const README_0_17_0_BLOCK_NEEDLES: [&str; 9] = [
    "Transport::destination_desc",
    "InboundRequest::track_outcome",
    "ResponseOutcome",
    "Resource(OutboundResult)",
    "SharedInstanceOptions::with_state_events",
    "SharedInstanceEvent",
    "with_connect_events",
    "AddressHash::new_from_hex_string",
    "since 0.14.1",
];

/// The demoted 0.17.0 block sits above the 0.16.0 and 0.15.1 blocks as
/// history: it still names every item of the release and tells consumers
/// `new_from_hex_string` has validated its input since 0.14.1; the demoted
/// 0.16.0 block's content is pinned by the test above.
#[test]
fn readme_patch_list_keeps_the_0_17_0_block_above_the_0_16_0_paragraph() {
    let readme = read_repo_file("crates/libs/rns-transport/README.md");
    let patch_list = crate_readme_section(&readme, "Patch list")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let marker_0_17_0 = "What `0.17.0` carries on top of `0.16.0`:";
    let marker_0_16_0 = "What `0.16.0` carries on top of `0.15.1`:";
    let marker_0_15_1 = "What `0.15.1` carries on top of `0.15.0`:";
    let at_0_17_0 =
        patch_list.find(marker_0_17_0).expect("the README patch list should keep the 0.17.0 block");
    let at_0_16_0 =
        patch_list.find(marker_0_16_0).expect("the README patch list should keep the 0.16.0 block");
    let at_0_15_1 =
        patch_list.find(marker_0_15_1).expect("the README patch list should keep the 0.15.1 block");
    assert!(
        at_0_17_0 < at_0_16_0 && at_0_16_0 < at_0_15_1,
        "the README patch list should run 0.17.0, then 0.16.0, then 0.15.1"
    );

    let block_0_17_0 = &patch_list[at_0_17_0..at_0_16_0];
    for needle in README_0_17_0_BLOCK_NEEDLES {
        assert!(
            block_0_17_0.contains(needle),
            "the README 0.17.0 block should still say {needle:?}:\n{block_0_17_0}"
        );
    }
}

/// What the README's 0.18.0 patch-list block says, one needle per item.
const README_0_18_0_BLOCK_NEEDLES: [&str; 18] = [
    "InboundRequest::into_parts",
    "ResponseOutcomeTracker",
    "set_max_response_size",
    "ResponseOutcome::TooLarge",
    "Transport::resolve_destination",
    "ProofStrategy",
    "Transport::prove_link_packet",
    "with_dial_failure_events",
    "SharedInstanceEvent::DialFailed",
    "with_first_connect_timeout",
    "with_disconnect_reason_events",
    "DisconnectedWithReason",
    "Transport::interface_events",
    "InterfaceEvent",
    "as isize",
    "await_path",
    "await_link_activation",
    "Duration::MAX",
];

/// The demoted 0.18.0 block sits above the 0.17.0 and 0.16.0 blocks as
/// history: it still names every item of the release (the owned request
/// split, the responder cap, the dial failures and first-connect timeout, the
/// disconnect reasons, the interface event stream, the destination resolve,
/// the link proof strategy, the one break and the two deadline fixes); the
/// demoted 0.17.0 block's content is pinned by the test above.
#[test]
fn readme_patch_list_keeps_the_0_18_0_block_above_the_0_17_0_paragraph() {
    let readme = read_repo_file("crates/libs/rns-transport/README.md");
    let patch_list = crate_readme_section(&readme, "Patch list")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let marker_0_18_0 = "What `0.18.0` carries on top of `0.17.0`:";
    let marker_0_17_0 = "What `0.17.0` carries on top of `0.16.0`:";
    let marker_0_16_0 = "What `0.16.0` carries on top of `0.15.1`:";
    let at_0_18_0 =
        patch_list.find(marker_0_18_0).expect("the README patch list should keep the 0.18.0 block");
    let at_0_17_0 =
        patch_list.find(marker_0_17_0).expect("the README patch list should keep the 0.17.0 block");
    let at_0_16_0 =
        patch_list.find(marker_0_16_0).expect("the README patch list should keep the 0.16.0 block");
    assert!(
        at_0_18_0 < at_0_17_0 && at_0_17_0 < at_0_16_0,
        "the README patch list should run 0.18.0, then 0.17.0, then 0.16.0"
    );

    let block_0_18_0 = &patch_list[at_0_18_0..at_0_17_0];
    for needle in README_0_18_0_BLOCK_NEEDLES {
        assert!(
            block_0_18_0.contains(needle),
            "the README 0.18.0 block should still say {needle:?}:\n{block_0_18_0}"
        );
    }
}

/// What the README's 0.18.1 patch-list block says, one needle per item.
const README_0_18_1_BLOCK_NEEDLES: [&str; 15] = [
    "DIAL_WARN_STRIDE",
    "every 30th attempt",
    "(attempt N)",
    "after N failed attempt(s)",
    "Transport::interface_connect_count",
    "InterfaceManager::connected_streams",
    "TcpRuntimeStatusHandle::connected_streams()",
    "connects=N",
    "RecvError::Lagged",
    "max TCP reconnect attempts reached for <endpoint>",
    "python_shared_instance_connect_count.rs",
    "tests/support/",
    "CLICOLOR_FORCE",
    "python-lxmd-rust-lxmd-smoke.sh",
    "free loopback ports",
];

/// The demoted 0.18.1 block sits above the 0.18.0 and 0.17.0 blocks as
/// history: it still names every item of the release (the dial-warn cadence,
/// the connect counter and its status key, the give-up line, the
/// Python-in-the-loop proof and the test-only hygiene); the demoted 0.18.0
/// block's content is pinned by the test above.
#[test]
fn readme_patch_list_keeps_the_0_18_1_block_above_the_0_18_0_paragraph() {
    let readme = read_repo_file("crates/libs/rns-transport/README.md");
    let patch_list = crate_readme_section(&readme, "Patch list")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let marker_0_18_1 = "What `0.18.1` carries on top of `0.18.0`:";
    let marker_0_18_0 = "What `0.18.0` carries on top of `0.17.0`:";
    let marker_0_17_0 = "What `0.17.0` carries on top of `0.16.0`:";
    let at_0_18_1 =
        patch_list.find(marker_0_18_1).expect("the README patch list should keep the 0.18.1 block");
    let at_0_18_0 =
        patch_list.find(marker_0_18_0).expect("the README patch list should keep the 0.18.0 block");
    let at_0_17_0 =
        patch_list.find(marker_0_17_0).expect("the README patch list should keep the 0.17.0 block");
    assert!(
        at_0_18_1 < at_0_18_0 && at_0_18_0 < at_0_17_0,
        "the README patch list should run 0.18.1, then 0.18.0, then 0.17.0"
    );

    let block_0_18_1 = &patch_list[at_0_18_1..at_0_18_0];
    for needle in README_0_18_1_BLOCK_NEEDLES {
        assert!(
            block_0_18_1.contains(needle),
            "the README 0.18.1 block should still say {needle:?}:\n{block_0_18_1}"
        );
    }
}

/// What the README's 0.18.2 patch-list block says, one needle per item.
const README_0_18_2_BLOCK_NEEDLES: [&str; 19] = [
    "SharedInstanceOptions::with_tunnel_synthesis",
    "RNS/Interfaces/LocalInterface.py",
    "RNS/Transport.py:2829",
    "Transport::synthesize_tunnel_on_interface",
    "LinkCloseReason::Timeout",
    "INTERVAL_OUTPUT_LINK_REPEAT",
    "delivery::await_link_activation",
    "io::ErrorKind::ConnectionAborted",
    "link closed before activation:",
    "admit_announce_before_queue",
    "IC_DEQUE_MIN_SAMPLE",
    "burst_freq_new",
    "python_shared_instance_two_clients.rs",
    "Tunnel endpoint",
    "Restored path to",
    "tests/support/",
    "link_activation_closed_repend_then_close_probe.rs",
    "coyote-rns-transport = \"0.18.2\"",
    "nothing to opt into",
];

/// The demoted 0.18.2 block sits above the 0.18.1 and 0.18.0 blocks as
/// history: it still names every item of the release (the tunnel opt-in, the
/// one link request, the fail-fast wait, the announce admission, the
/// Python-in-the-loop proof, the test-only hygiene and the consumer pin); the
/// demoted 0.18.1 block's content is pinned by the test above.
#[test]
fn readme_patch_list_keeps_the_0_18_2_block_above_the_0_18_1_paragraph() {
    let readme = read_repo_file("crates/libs/rns-transport/README.md");
    let patch_list = crate_readme_section(&readme, "Patch list")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let marker_0_18_2 = "What `0.18.2` carries on top of `0.18.1`:";
    let marker_0_18_1 = "What `0.18.1` carries on top of `0.18.0`:";
    let marker_0_18_0 = "What `0.18.0` carries on top of `0.17.0`:";
    let at_0_18_2 =
        patch_list.find(marker_0_18_2).expect("the README patch list should keep the 0.18.2 block");
    let at_0_18_1 =
        patch_list.find(marker_0_18_1).expect("the README patch list should keep the 0.18.1 block");
    let at_0_18_0 =
        patch_list.find(marker_0_18_0).expect("the README patch list should keep the 0.18.0 block");
    assert!(
        at_0_18_2 < at_0_18_1 && at_0_18_1 < at_0_18_0,
        "the README patch list should run 0.18.2, then 0.18.1, then 0.18.0"
    );

    let block_0_18_2 = &patch_list[at_0_18_2..at_0_18_1];
    for needle in README_0_18_2_BLOCK_NEEDLES {
        assert!(
            block_0_18_2.contains(needle),
            "the README 0.18.2 block should still say {needle:?}:\n{block_0_18_2}"
        );
    }
}

/// The README patch list opens with the 0.18.3 block, which names every item
/// of the release (the auto peer count, the rnsd RPC client, announce
/// emission, the Python-in-the-loop proofs, the test-only hygiene and the
/// consumer pin), and runs 0.18.2, then 0.18.1 below it; the demoted blocks'
/// content is pinned by the tests above.
#[test]
fn readme_patch_list_opens_with_the_0_18_3_block_and_keeps_the_0_18_2_paragraph() {
    let readme = read_repo_file("crates/libs/rns-transport/README.md");
    let patch_list = crate_readme_section(&readme, "Patch list")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let marker_0_18_3 = "What `0.18.3` carries on top of `0.18.2`:";
    let marker_0_18_2 = "What `0.18.2` carries on top of `0.18.1`:";
    let marker_0_18_1 = "What `0.18.1` carries on top of `0.18.0`:";
    let at_0_18_3 = patch_list
        .find(marker_0_18_3)
        .expect("the README patch list should open with the 0.18.3 block");
    assert_eq!(
        patch_list.find("What `"),
        Some(at_0_18_3),
        "the README patch list should open with the 0.18.3 block, not another release's"
    );
    let at_0_18_2 =
        patch_list.find(marker_0_18_2).expect("the README patch list should keep the 0.18.2 block");
    let at_0_18_1 =
        patch_list.find(marker_0_18_1).expect("the README patch list should keep the 0.18.1 block");
    assert!(
        at_0_18_3 < at_0_18_2 && at_0_18_2 < at_0_18_1,
        "the README patch list should run 0.18.3, then 0.18.2, then 0.18.1"
    );

    let block_0_18_3 = &patch_list[at_0_18_3..at_0_18_2];
    for needle in [
        "Transport::interface_peer_count",
        "RNS/Reticulum.py:1501-1503",
        "InterfaceManager::set_peer_counter",
        "AutoRuntimeStatusHandle::peer_count()",
        "peers=N",
        "transport::shared_instance_rpc",
        "multiprocessing.connection",
        "RpcKey::from_transport_identity_file",
        "md-5",
        "AnnounceEvent::emitted",
        "AnnounceEvent::random_blob",
        "Transport::latest_announce_emitted",
        "RNS/Transport.py:3459-3473",
        "python_shared_instance_rpc.rs",
        "python_stdlib_listener_rpc_probe.rs",
        "python_shared_instance_announce_emitted.rs",
        "shared_instance_rpc_departed_server_python_probe.rs",
        "auto-peer-count-timeout",
        "hil_core_case_names_probe.rs",
        "announce_emitted_probe.rs",
        "coyote-rns-transport = \"0.18.3\"",
        "RpcEndpoint::for_instance(\"default\")",
        "kind == \"AutoInterface\"",
        "nothing to opt into",
    ] {
        assert!(
            block_0_18_3.contains(needle),
            "the README 0.18.3 block should say {needle:?}:\n{block_0_18_3}"
        );
    }
}

/// The shared-instance section describes the peer count the way the
/// AutoInterface keeps it: the transport getter, the manager read and the
/// status handle underneath, the always-present JSON key, the Python lines
/// it mirrors and the public-API test files that pin it; the divergence
/// register names the count as fork-only under `Not divergences:` (API
/// shape, not behaviour), and the changelog section that carries the
/// bullet makes the same claims, each document in its own spelling of the
/// signatures.
#[test]
fn crate_readme_shared_instance_section_describes_interface_peer_count() {
    let readme = read_repo_file("crates/libs/rns-transport/README.md");
    let section = squash(crate_readme_section(&readme, "Attaching to a shared instance"));
    let shared_claims = [
        "`InterfaceManager::set_peer_counter`",
        "returns `false`",
        "`\"peers\"`",
        "`peers=",
        "keeps the state's own when no handle is given",
        "`RNS/Reticulum.py:1501-1503`",
        "`RNS/Interfaces/AutoInterface.py:126`",
        "`RNS/Utilities/rnstatus.py:555-556`",
        "`tests/interface_peer_count_api.rs`",
        "`tests/interface_peer_count_probe.rs`",
        "`tests/auto_status_handle_peers_json_probe.rs`",
        "`iface/auto_runtime_parts/tests_sections/auto_peer_count_pins.rs`",
        "`iface/auto_runtime_parts/tests_sections/auto_peer_count_probe.rs`",
        "`rnstatus_peers_cli`",
        "`auto_peers_daemon_status_probe`",
    ];
    for claim in shared_claims.into_iter().chain([
        "`Transport::interface_peer_count(&iface)`",
        "`InterfaceManager::peer_count(&address)`",
        "`AutoRuntimeStatusHandle::peer_count()`",
    ]) {
        assert!(
            section.contains(&squash(claim)),
            "the shared-instance section should mention {claim}:\n{section}"
        );
    }
    let divergences = crate_readme_section(&readme, "Known divergences from Python RNS");
    let (real, api_shape) = divergences
        .split_once("\nNot divergences:\n")
        .expect("the register separates API-shape facts under `Not divergences:`");
    assert!(
        squash(api_shape).contains(&squash("`Transport::interface_peer_count()`")),
        "the peer count is API shape, listed below the marker:\n{api_shape}"
    );
    assert!(
        !squash(real).contains(&squash("`Transport::interface_peer_count()`")),
        "the peer count is not a behaviour divergence:\n{real}"
    );
    let changelog = read_repo_file("crates/libs/rns-transport/CHANGELOG.md");
    let changelog_section = squash(changelog_section_naming(
        &changelog,
        "`Transport::interface_peer_count(&AddressHash) -> Option<u64>`",
    ));
    for claim in shared_claims.into_iter().chain([
        "`InterfaceManager::peer_count` reads",
        "`AutoRuntimeStatusHandle::peer_count`",
        "`bootstrap_parts::module_core`",
        "`src/bin/shared/rnstatus.rs`",
        "`rnstatus_cli` test",
    ]) {
        assert!(changelog_section.contains(&squash(claim)), "the changelog should mention {claim}");
    }
}

/// The rnsd RPC section describes the client the way the code presents it:
/// the one call, the endpoint and key constructors a consumer reaches for,
/// the authentication error, the default TCP listener and the abstract
/// socket name, the Python lines the listener, the peer count and the
/// stdlib handshake are read from, the four test files that pin it, the
/// in-crate test that pins the failed `#FAILURE#` write and the
/// Python-in-the-loop parts that read a live rnsd and CPython's own
/// listener; the divergence register
/// names the module as fork-only below the `Not divergences:` marker (API
/// shape) and carries the key refusal above it as a real divergence, and
/// the changelog section that carries the bullet names the same
/// constructors, the handshake lines and the same pin files.
#[test]
fn crate_readme_rnsd_rpc_section_describes_the_client() {
    let readme = read_repo_file("crates/libs/rns-transport/README.md");
    let section = squash(crate_readme_section(&readme, "Reading rnsd's interface statistics"));
    for claim in [
        "`interface_stats`",
        "`RpcEndpoint::for_instance(\"default\")`",
        "`RpcKey::from_transport_identity_file(",
        "`RpcError::Authentication`",
        "`127.0.0.1:37429`",
        "`rns/<instance_name>/rpc`",
        "`#[non_exhaustive]`",
        "`RNS/Reticulum.py:1274-1334`",
        "`RNS/Reticulum.py:1501-1503`",
        "`connection.py:917-1006`",
        "`src/transport/shared_instance_rpc/tests/mod.rs`",
        "`failure_notice_on_a_broken_pipe_is_still_authentication`",
        "`tests/shared_instance_rpc_api.rs`",
        "`tests/shared_instance_rpc_departed_server.rs`",
        "`tests/shared_instance_rpc_departed_server_python_probe.rs`",
        "`crates/apps/reticulumd/tests/python_channel_interop_parts/python_shared_instance_rpc.rs`",
        "`python_shared_instance_rpc_probe.rs`",
        "`python_stdlib_listener_rpc_probe.rs`",
    ] {
        assert!(
            section.contains(&squash(claim)),
            "the rnsd RPC section should mention {claim}:\n{section}"
        );
    }
    let divergences = crate_readme_section(&readme, "Known divergences from Python RNS");
    let (real, api_shape) = divergences
        .split_once("\nNot divergences:\n")
        .expect("the register separates API-shape facts under `Not divergences:`");
    assert!(
        squash(api_shape)
            .contains(&squash("`transport::shared_instance_rpc` is a fork-only typed client")),
        "the typed client is API shape, listed below the marker:\n{api_shape}"
    );
    assert!(
        squash(real).contains(&squash("`RpcKey::from_hex` refuses")),
        "the key refusal is a real divergence, listed above the marker:\n{real}"
    );
    assert!(
        !squash(api_shape).contains(&squash("`RpcKey::from_hex` refuses")),
        "the key refusal does not repeat below the marker:\n{api_shape}"
    );
    let changelog = read_repo_file("crates/libs/rns-transport/CHANGELOG.md");
    let changelog_section = squash(changelog_section_naming(
        &changelog,
        "`transport::shared_instance_rpc` (`transport/shared_instance_rpc.rs`), a",
    ));
    for claim in [
        "`interface_stats(&endpoint, &key, timeout)`",
        "`for_instance(name)`",
        "`from_transport_identity_file`",
        "`Authentication`",
        "`127.0.0.1:37429`",
        "`RNS/Reticulum.py:1274-1334`",
        "`connection.py:917-1006`",
        "`src/transport/shared_instance_rpc/tests/mod.rs`",
        "`failure_notice_on_a_broken_pipe_is_still_authentication`",
        "`tests/shared_instance_rpc_api.rs`",
        "`tests/shared_instance_rpc_departed_server.rs`",
        "`tests/shared_instance_rpc_departed_server_python_probe.rs`",
        "`python_shared_instance_rpc.rs`",
        "`python_shared_instance_rpc_probe.rs`",
        "`python_stdlib_listener_rpc_probe.rs`",
    ] {
        assert!(changelog_section.contains(&squash(claim)), "the changelog should mention {claim}");
    }
}

/// The announce section describes the emission second the way the code
/// carries it: the two event fields, the blob slice and the Python lines
/// it is decoded from, the relay replaying the stored bytes under the
/// path-response flag against the destination emitting afresh, the
/// accessor's `None` and `Some(0)` shapes and the path-table calls that
/// drop or keep it, and the four test files that pin it, in a paragraph
/// that follows the `is_path_response` one; the divergence register names
/// the fields as fork-only under `Not divergences:` (API shape, not
/// behaviour), and the changelog section that carries the bullet makes the
/// same claims with the accessor's full signature.
#[test]
fn crate_readme_announce_section_describes_announce_emitted() {
    let readme = read_repo_file("crates/libs/rns-transport/README.md");
    let section = squash(crate_readme_section(&readme, "Opening links and admitting resources"));
    let shared_claims = [
        "`AnnounceEvent::emitted`",
        "`AnnounceEvent::random_blob`",
        "`RNS/Transport.py:2217`",
        "`RNS/Transport.py:3459-3473`",
        "`RNS/Destination.py:308`",
        "`RNS/Transport.py:3729`",
        "`timebase_from_random_blobs`",
        "`None` without a path table entry",
        "`Some(0)` for an entry without blobs",
        "`tests/announce_emitted_api.rs`",
        "`transport/tests_parts/two_transport_loopback_announce_emitted.rs`",
        "`tests/announce_emitted_probe.rs`",
    ];
    for claim in shared_claims.into_iter().chain([
        "`Transport::latest_announce_emitted` returns `Option<u64>`",
        "`Transport::expire_path` removes it",
        "`Transport::deregister_destination` leaves it standing",
        "`python_shared_instance_announce_emitted.rs`",
    ]) {
        assert!(
            section.contains(&squash(claim)),
            "the announce section should mention {claim}:\n{section}"
        );
    }
    let path_response_at = section
        .find(&squash("`AnnounceEvent::is_path_response` is true"))
        .expect("the section opens the flag paragraph before the emission one");
    let emitted_at = section
        .find(&squash("`AnnounceEvent::emitted` is"))
        .expect("the section opens the emission paragraph");
    assert!(
        path_response_at < emitted_at,
        "the emission paragraph follows the `is_path_response` one:\n{section}"
    );
    let divergences = crate_readme_section(&readme, "Known divergences from Python RNS");
    let (real, api_shape) = divergences
        .split_once("\nNot divergences:\n")
        .expect("the register separates API-shape facts under `Not divergences:`");
    assert!(
        squash(api_shape)
            .contains(&squash("`AnnounceEvent::emitted` and `AnnounceEvent::random_blob`")),
        "the emission fields are API shape, listed below the marker:\n{api_shape}"
    );
    assert!(
        !squash(real)
            .contains(&squash("`AnnounceEvent::emitted` and `AnnounceEvent::random_blob`")),
        "the emission fields are not a behaviour divergence:\n{real}"
    );
    let changelog = read_repo_file("crates/libs/rns-transport/CHANGELOG.md");
    let changelog_section = squash(changelog_section_naming(
        &changelog,
        "`Transport::latest_announce_emitted(&AddressHash) -> Option<u64>`",
    ));
    for claim in shared_claims.into_iter().chain([
        "`expire_path` removes it",
        "`deregister_destination` leaves it standing",
        "`python_channel_interop_parts/python_shared_instance_announce_emitted.rs`",
    ]) {
        assert!(changelog_section.contains(&squash(claim)), "the changelog should mention {claim}");
    }
}
