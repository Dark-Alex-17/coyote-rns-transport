// Crate README section pins: the divergence register cites Python lines,
// each prose section describes its API the way the code behaves, and every
// `rust` fence is pinned verbatim by a public-API test file.

/// The crate README's `## Known divergences from Python RNS` register is the
/// consumer's list of where this crate is known to depart from RNS 1.5.2.
/// Every entry must point at Python with an `RNS/<file>.py:<line>` citation,
/// and the first entry is the request-timeout divergence the release notes
/// lead with, cited at `RNS/Link.py:1398` (`request_timed_out`) and `:1408`
/// (`response_rejected`).
#[test]
fn crate_readme_known_divergences_cite_python_lines() {
    let readme = read_repo_file("crates/libs/rns-transport/README.md");
    let section = crate_readme_section(&readme, "Known divergences from Python RNS");

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

    let uncited: Vec<&str> =
        entries.iter().map(String::as_str).filter(|entry| !cites_python_line(entry)).collect();
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

/// The crate README's `## Attaching to a shared instance` section carries
/// exactly two `rust` fences, the attach, handler, announce and detach
/// snippet and the state-stream snippet, and no fence of any other kind;
/// every line of both snippets is run verbatim by
/// `crates/libs/rns-transport/tests/shared_instance_api.rs` against an
/// in-process shared instance, under the same squash and skip rules as
/// `crate_readme_rust_snippets_are_pinned_verbatim_by_the_api_tests`.
#[test]
fn crate_readme_shared_instance_section_fence_is_pinned_by_the_api_test() {
    let readme = read_repo_file("crates/libs/rns-transport/README.md");
    let section = crate_readme_section(&readme, "Attaching to a shared instance");
    let fences: Vec<&str> =
        section.lines().map(str::trim_start).filter(|line| line.starts_with("```")).collect();
    assert_eq!(
        fences,
        vec!["```rust", "```", "```rust", "```"],
        "the shared-instance section should carry exactly two rust fences and no other"
    );
    let snippets = rust_snippets(section);
    assert!(
        snippets.iter().all(|snippet| !snippet.is_empty()),
        "no shared-instance snippet should be empty"
    );

    let pin = squash(&read_repo_file("crates/libs/rns-transport/tests/shared_instance_api.rs"));
    let unpinned: Vec<&str> = snippets
        .iter()
        .flatten()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty() && !line.starts_with("use ") && !line.starts_with("//"))
        .filter(|line| {
            let needle = match line.split_once(" => ") {
                Some((pattern, _)) => squash(pattern),
                None => squash(line),
            };
            needle != "_" && !pin.contains(&needle)
        })
        .collect();
    assert!(
        unpinned.is_empty(),
        "every line of the shared-instance snippet should appear verbatim in tests/shared_instance_api.rs:\n{}",
        unpinned.join("\n")
    );
}

/// The shared-instance section's parity claims each point at the Python
/// they mirror: the delayed registration announce, the reappearance
/// re-announce and its delay, the disappearance flush and the guard that
/// runs it only while attached, the drop teardown's LINKCLOSE, the redial
/// wait, and the hashlist the owner keeps. Neither it nor the divergence
/// register may describe any of it as still pending: everything the section
/// claims is mirrored.
#[test]
fn crate_readme_shared_instance_section_cites_the_parity_lines() {
    let readme = read_repo_file("crates/libs/rns-transport/README.md");
    let section = crate_readme_section(&readme, "Attaching to a shared instance");
    for cite in [
        "`RNS/Transport.py:2911-2916`",
        "`RNS/Transport.py:3705-3709`",
        "`RNS/Transport.py:3690-3702`",
        "`RNS/Transport.py:1618-1627`",
        "`RNS/Interfaces/LocalInterface.py:178-181`",
        "`RNS/Link.py:657-668`",
        "`RNS/Interfaces/LocalInterface.py:159-160`",
        "`RNS/Interfaces/LocalInterface.py:255-261`",
        "`RNS/Interfaces/LocalInterface.py:109`",
        "`RNS/Transport.py:1938`",
    ] {
        assert!(
            section.contains(cite),
            "the shared-instance section should cite {cite}:\n{section}"
        );
    }
    let divergences = crate_readme_section(&readme, "Known divergences from Python RNS");
    for (name, text) in [("shared-instance", section), ("divergence", divergences)] {
        for pending in ["not yet mirrored", "does not yet"] {
            assert!(
                !text.contains(pending),
                "the {name} section should not call anything pending ({pending:?}):\n{text}"
            );
        }
    }
}

/// The link-opening section describes `Transport::destination_desc` the way
/// the accessor behaves: what it returns, the identity accessor it agrees
/// with, the truncated name hash it stores and the comparison that suits it,
/// the path expiry it survives, the deregistration that removes it, and the
/// link call that takes the desc.
#[test]
fn crate_readme_link_section_describes_destination_desc() {
    let readme = read_repo_file("crates/libs/rns-transport/README.md");
    let section = crate_readme_section(&readme, "Opening links and admitting resources");
    for claim in [
        "`Transport::destination_desc`",
        "`Option<DestinationDesc>`",
        "`Transport::destination_identity`",
        "`DestinationName::new_from_hash_slice`",
        "`DestinationName::new`",
        "`as_name_hash_slice()`",
        "`Transport::expire_path`",
        "`Transport::deregister_destination`",
        "`Transport::link`",
    ] {
        assert!(
            section.contains(claim),
            "the link-opening section should mention {claim}:\n{section}"
        );
    }
}

/// The link-opening section describes `Transport::resolve_destination` the
/// way the call behaves: the timeout it takes, the two accessors it joins,
/// the single path request an unknown destination costs, the two ways the
/// desc and the path table part, and the Python calls an application
/// strings together by hand instead.
#[test]
fn crate_readme_link_section_describes_resolve_destination() {
    let readme = read_repo_file("crates/libs/rns-transport/README.md");
    let section = crate_readme_section(&readme, "Opening links and admitting resources");
    for claim in [
        "`Transport::resolve_destination`",
        "`Duration`",
        "timeout",
        "one path request",
        "`Transport::await_path`",
        "`Transport::request_path`",
        "`Transport::deregister_destination`",
        "path table entry stands",
        "answers `None` at once",
        "`Transport::expire_path`",
        "answers `Some` at once",
        "`RNS/Transport.py:3279`",
        "`RNS/Transport.py:3126`",
    ] {
        assert!(
            section.contains(claim),
            "the link-opening section should mention {claim}:\n{section}"
        );
    }
}

/// The link-opening section carries exactly one `rust` fence, the
/// `destination_desc` snippet, and no fence of any other kind. The block pin
/// below reads only the first fence, so a second one would slip past it;
/// the resolve prose is meant to stay prose.
#[test]
fn crate_readme_link_section_carries_one_rust_fence() {
    let readme = read_repo_file("crates/libs/rns-transport/README.md");
    let section = crate_readme_section(&readme, "Opening links and admitting resources");
    let fences: Vec<&str> =
        section.lines().map(str::trim_start).filter(|line| line.starts_with("```")).collect();
    assert_eq!(
        fences,
        vec!["```rust", "```"],
        "the link-opening section should carry exactly one rust fence and no other"
    );
}

/// The three places that describe what an unknown destination costs
/// `Transport::resolve_destination` and how the call waits say the same
/// thing: exactly one path request is attempted, a `Broadcast` on every
/// outgoing interface, the call waits until a path is learned polling every
/// 50 ms or the time left, and a timeout the clock cannot hold waits
/// indefinitely. The README, the changelog section that names the call and
/// the method's rustdoc each carry every phrase, and none spells the
/// message type bare, so the capitalised word reads as the `TxMessageType`
/// variant it names.
#[test]
fn crate_readme_resolve_destination_prose_matches_the_rustdoc_and_the_changelog() {
    let readme = read_repo_file("crates/libs/rns-transport/README.md");
    let section = crate_readme_section(&readme, "Opening links and admitting resources");
    let changelog = read_repo_file("crates/libs/rns-transport/CHANGELOG.md");
    let changelog_section =
        changelog_section_naming(&changelog, "`Transport::resolve_destination`");
    let rustdoc = read_repo_file(
        "crates/libs/rns-transport/src/transport/links_parts/transport_sections/resolve_destination.rs",
    );
    let rustdoc = doc_comment_text(&rustdoc);

    for (name, text) in [
        ("the README link-opening section", section),
        ("the changelog section naming resolve_destination", changelog_section),
        ("the resolve_destination rustdoc", rustdoc.as_str()),
    ] {
        // Line wraps differ between the three, so the claims are matched with
        // whitespace removed; the bare spelling is looked for as a whole word
        // on a single line, so punctuation around it does not hide it.
        let flattened = squash(text);
        for claim in [
            "exactly one path request is attempted",
            "a `Broadcast` on every outgoing interface",
            "until a path is learned",
            "polling every 50 ms or the time left, whichever is shorter",
            "`Duration::MAX` included, waits indefinitely instead of panicking",
        ] {
            assert!(flattened.contains(&squash(claim)), "{name} should say {claim:?}:\n{text}");
        }
        let bare = text.lines().find(|line| {
            line.split(|c: char| !c.is_alphanumeric() && c != '_' && c != '`')
                .any(|word| word == "Broadcast")
        });
        assert!(
            bare.is_none(),
            "{name} should backtick the message type as the rustdoc does, found {bare:?}"
        );
    }
}

/// The three rustdocs that say how the responder's cap measures a
/// `RequestResponse::Resource`, `ResponseOutcome::TooLarge`,
/// `Destination::set_max_response_size` and the dispatcher's
/// `send_response`, describe the advertised size in the same words as the
/// README's request-handler section and the changelog section that names
/// `set_max_response_size`, so none of the five can drift apart on its own.
#[test]
fn response_cap_rustdoc_twins_describe_the_resource_measure_alike() {
    let measure = "together with the packed metadata and its length prefix";
    let readme = read_repo_file("crates/libs/rns-transport/README.md");
    let section = squash(crate_readme_section(&readme, "Request handlers"));
    assert!(
        section.contains(&squash(measure)),
        "the README request-handler section should say {measure:?}"
    );
    let changelog = read_repo_file("crates/libs/rns-transport/CHANGELOG.md");
    let changelog_section = squash(changelog_section_naming(&changelog, "`set_max_response_size`"));
    assert!(
        changelog_section.contains(&squash(measure)),
        "the changelog section naming set_max_response_size should say {measure:?}"
    );
    for file in [
        "crates/libs/rns-transport/src/destination/request/outcome.rs",
        "crates/libs/rns-transport/src/destination_parts/destination.rs",
        "crates/libs/rns-transport/src/transport/request_dispatch.rs",
    ] {
        let rustdoc = squash(&doc_comment_text(&read_repo_file(file)));
        assert!(rustdoc.contains(&squash(measure)), "{file} rustdoc should say {measure:?}");
    }
}

/// The text of the first `## ` changelog section, whatever its heading
/// says, that contains `anchor`. Each pin anchors on the API name its
/// bullet is about: anchoring on the content rather than on the
/// `Unreleased` label keeps the pin green across the release cut that
/// promotes the stub to a version heading, and anchoring on the content
/// rather than on position keeps it green when the next `## Unreleased`
/// stub is reopened above the section that carries the bullet.
fn changelog_section_naming<'a>(changelog: &'a str, anchor: &str) -> &'a str {
    changelog
        .split("\n## ")
        .skip(1)
        .find(|section| section.contains(anchor))
        .unwrap_or_else(|| panic!("a changelog section names {anchor}"))
}

/// The section selector survives both shapes the release cycle gives the
/// changelog: the stub promoted to a version heading, and a fresh empty
/// `## Unreleased` stub reopened above it. With the anchor named in two
/// sections, the newest one wins, so a pin reads the release that carried
/// the bullet and not an older restatement; the heading and the preamble
/// above the first `## ` are never a candidate.
#[test]
fn changelog_section_naming_skips_a_reopened_stub_and_prefers_the_newest_match() {
    let changelog = "# Changelog\n\nPreamble that names `Transport::resolve_destination` too.\n\
        \n## Unreleased\n\nNothing yet.\n\
        \n## 0.18.0 - 2026-11-01\n\n- Added `Transport::resolve_destination`, the newest bullet.\n\
        \n## 0.17.0 - 2026-10-27\n\n- Mentions `Transport::resolve_destination` again, older.\n";
    let section = changelog_section_naming(changelog, "`Transport::resolve_destination`");
    assert!(
        section.starts_with("0.18.0 - 2026-11-01\n"),
        "the first section that names the anchor is the promoted one past the stub, got:\n{section}"
    );
    assert!(section.contains("the newest bullet"), "the section body is returned whole");
    assert!(
        !section.contains("older") && !section.contains("Preamble"),
        "neither the older restatement nor the preamble is read, got:\n{section}"
    );

    let stubless = "# Changelog\n\n## 0.18.0 - 2026-11-01\n\n- `set_max_response_size` lands.\n";
    assert_eq!(
        changelog_section_naming(stubless, "`set_max_response_size`"),
        "0.18.0 - 2026-11-01\n\n- `set_max_response_size` lands.\n",
        "with no stub the promoted section is the first candidate"
    );
}

/// A changelog that names the anchor nowhere fails the pin loudly, naming
/// the anchor, rather than reading some other section.
#[test]
#[should_panic(expected = "a changelog section names `Transport::resolve_destination`")]
fn changelog_section_naming_panics_when_no_section_names_the_anchor() {
    let changelog =
        "# Changelog\n\n## Unreleased\n\nNothing yet.\n\n## 0.17.0 - 2026-10-27\n\n- Other.\n";
    changelog_section_naming(changelog, "`Transport::resolve_destination`");
}

/// The crate changelog groups each release's bullets under `### Added`,
/// `### Changed` and `### Fixed` sub-headings, so a section runs from its
/// `## ` heading to the next `## ` heading and a `### ` line inside it is
/// body, not a boundary. A pin that anchors on a bullet under `### Changed`
/// therefore reads the whole release, `### Added` included, and a twin
/// sentence may sit under either sub-heading.
#[test]
fn changelog_section_naming_keeps_level_three_sub_headings_inside_one_section() {
    let changelog = "# Changelog\n\
        \n## Unreleased\n\
        \n### Added\n\n- Added `set_max_response_size`, measured at the advertised size.\n\
        \n### Changed\n\n- `Transport::resolve_destination` polls every 50 ms.\n\
        \n## 0.17.0 - 2026-10-27\n\
        \n### Added\n\n- Older `Transport::resolve_destination` restatement.\n";
    let section = changelog_section_naming(changelog, "`Transport::resolve_destination`");
    assert!(
        section.starts_with("Unreleased\n"),
        "a `### ` sub-heading does not end the section that carries the anchor, got:\n{section}"
    );
    assert!(
        section.contains("### Added") && section.contains("### Changed"),
        "both sub-headings of the release are returned as one section, got:\n{section}"
    );
    assert!(
        section.contains("`set_max_response_size`") && !section.contains("Older"),
        "the sibling sub-heading's bullet is in reach while the older release is not, got:\n{section}"
    );
    let under_added = changelog_section_naming(changelog, "`set_max_response_size`");
    assert_eq!(under_added, section, "anchoring under either sub-heading reads the same release");
}

/// Every `///` line of a source file joined with single spaces, so a phrase
/// can be looked for across line wraps.
fn doc_comment_text(source: &str) -> String {
    source
        .lines()
        .map(str::trim_start)
        .filter(|line| line.starts_with("///"))
        .map(|line| line.trim_start_matches('/').trim())
        .collect::<Vec<_>>()
        .join(" ")
}

/// The shared-instance section describes the state stream the way the
/// attach and its consumer task behave: the option that installs it, the
/// `non_exhaustive` enum and its three variants, the non-blocking send and
/// what a full channel does, the posture flag the events leave alone, the
/// client builders and getters underneath, and the Python reconnect loop
/// that has no such stream.
#[test]
fn crate_readme_shared_instance_section_describes_state_events() {
    let readme = read_repo_file("crates/libs/rns-transport/README.md");
    let section = crate_readme_section(&readme, "Attaching to a shared instance");
    for claim in [
        "`SharedInstanceOptions::with_state_events`",
        "`SharedInstanceEvent`",
        "`Connecting`",
        "`Connected`",
        "`Disconnected`",
        "`try_send`",
        "a drop that was already queued",
        "a connect already queued is read as `Connected`",
        "already set when `Connecting` arrives",
        "`connected_to_shared_instance()`",
        "`SharedInstanceOptions` clone",
        "interleave",
        "`with_connect_events`",
        "`connect_events_enabled`",
        "`RNS/Interfaces/LocalInterface.py:155-181`",
    ] {
        assert!(
            section.contains(claim),
            "the shared-instance section should mention {claim}:\n{section}"
        );
    }
}

/// The request-handler section describes `track_outcome` the way the
/// dispatcher behaves: the receiver it returns, when to call it, each
/// outcome and what it proves, the proof section a packet outcome
/// defers to, the stop and the panicking handler that resolve `NotSent`, the
/// dropped-sender rule they are instances of, and the Python it diverges
/// from.
#[test]
fn crate_readme_request_handler_section_describes_track_outcome() {
    let readme = read_repo_file("crates/libs/rns-transport/README.md");
    let section = crate_readme_section(&readme, "Request handlers");
    for claim in [
        "`request.track_outcome()`",
        "`ResponseOutcomeReceiver`",
        "`ResponseOutcome`",
        "`Packet`",
        "`Resource`",
        "`NotSent`",
        "see Delivery proofs on a link",
        "`into_data`",
        "`Transport::stop`",
        "panicked after tracking",
        "drops the sender",
        "`RNS/Link.py:846-852`",
    ] {
        assert!(
            section.contains(claim),
            "the request-handler section should mention {claim}:\n{section}"
        );
    }
}

/// The request-handler section describes `into_parts` and the responder's
/// response cap the way the dispatcher behaves: the tracker `into_parts`
/// returns and its arm-before-return rule, the three destination accessors,
/// the `TooLarge` outcome a refused response resolves, and one sentence that
/// sets the responder's cap beside the requester's
/// `RequestOptions::with_max_response_size` so a reader does not take one
/// for the other.
#[test]
fn crate_readme_request_handler_section_describes_into_parts_and_the_response_cap() {
    let readme = read_repo_file("crates/libs/rns-transport/README.md");
    let section = crate_readme_section(&readme, "Request handlers");
    for claim in [
        "`into_parts`",
        "`ResponseOutcomeTracker`",
        "`set_max_response_size`",
        "`clear_max_response_size`",
        "`max_response_size`",
        "`TooLarge`",
        "`RequestOptions::with_max_response_size`",
        "at the cap goes out",
        "logs one warn",
        "logs nothing",
        "compose at one number",
        "measures as its data alone",
        "together with the packed metadata and its length prefix",
        "leaves the slot as it was",
    ] {
        assert!(
            section.contains(claim),
            "the request-handler section should mention {claim}:\n{section}"
        );
    }

    let requester_cap = "`RequestOptions::with_max_response_size`";
    let flattened = section.split_whitespace().collect::<Vec<_>>().join(" ");
    let cross_reference = flattened
        .split(". ")
        .find(|sentence| sentence.contains(requester_cap))
        .expect("the request-handler section names the requester's cap");
    for claim in ["`set_max_response_size`", "requester", "responder"] {
        assert!(
            cross_reference.contains(claim),
            "the sentence naming {requester_cap} should also carry {claim}, got {cross_reference:?}"
        );
    }
    assert_eq!(
        flattened.matches(requester_cap).count(),
        1,
        "the request-handler section should name the requester's cap once, in the cross-reference"
    );
}

/// The link section's `destination_desc` snippet is pinned as one block, not
/// line by line: the per-line pin below matches squashed substrings, so a
/// snippet that swapped `destination_desc` for `destination_identity` would
/// still find every line somewhere in the API test that calls both. Here the
/// whole block (its `use` line aside) must appear contiguously in
/// `tests/destination_desc_api.rs`, which is what "run verbatim" promises.
#[test]
fn crate_readme_destination_desc_snippet_is_pinned_as_one_block() {
    let readme = read_repo_file("crates/libs/rns-transport/README.md");
    let section = crate_readme_section(&readme, "Opening links and admitting resources");
    let block = section
        .split("```rust\n")
        .nth(1)
        .and_then(|rest| rest.split("```").next())
        .expect("the link-opening section carries a rust snippet");
    assert!(
        block.contains(".destination_desc("),
        "the link-opening snippet should call destination_desc:\n{block}"
    );
    let body: String = block
        .lines()
        .filter(|line| !line.trim_start().starts_with("use "))
        .collect::<Vec<_>>()
        .join("\n");
    let body = squash(&body);
    assert!(!body.is_empty(), "the snippet has a body beyond its use lines");
    let pin = squash(&read_repo_file("crates/libs/rns-transport/tests/destination_desc_api.rs"));
    assert!(
        pin.contains(&body),
        "tests/destination_desc_api.rs should carry the README's destination_desc snippet as \
         one contiguous block, whitespace aside:\n{block}"
    );
}

/// Every `rust` snippet in the crate README is pinned verbatim by one of the
/// public-API test files, so a README edit that drifts from what compiles and
/// runs turns this test red. `use` lines are left to the compiler of those
/// tests; every other line of a snippet must appear, whitespace aside, in a
/// pin file, with a `match` arm compared up to its `=>` because the pins
/// reduce the README's log calls to labels (and may bind an unused pattern
/// variable with a leading underscore); a bare `_` arm is skipped rather than
/// matched against any underscore in the pins.
#[test]
fn crate_readme_rust_snippets_are_pinned_verbatim_by_the_api_tests() {
    let readme = read_repo_file("crates/libs/rns-transport/README.md");
    let pins: String = [
        "crates/libs/rns-transport/tests/request_receipt_readme.rs",
        "crates/libs/rns-transport/tests/request_handler_api.rs",
        "crates/libs/rns-transport/tests/outbound_outcome_api.rs",
        "crates/libs/rns-transport/tests/request_receipt_api.rs",
        "crates/libs/rns-transport/tests/shared_instance_api.rs",
        "crates/libs/rns-transport/tests/destination_desc_api.rs",
    ]
    .iter()
    .map(|path| read_repo_file(path))
    .collect::<Vec<_>>()
    .join("\n");
    let pins = squash(&pins);

    let snippets = rust_snippets(&readme);
    assert!(
        snippets.len() >= 7,
        "the README should carry the consumer, handler, request, outcome, link-opening, shared-instance and shared-instance state snippets, found {}",
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
            if needle != "_" && !pins.contains(&needle) {
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
