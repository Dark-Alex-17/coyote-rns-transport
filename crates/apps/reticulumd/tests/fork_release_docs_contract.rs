//! Release-documentation contracts of the fork: every public item added over
//! the upstream baseline cites its Python counterpart in rustdoc or says it is
//! fork-only, the README divergence register cites Python line numbers, the
//! release notes carry `### Breaking` with migrations on a minor bump, the
//! README's Rust snippets are pinned verbatim by the public-API tests, and
//! the enums the docs call `#[non_exhaustive]` carry the attribute.

use std::fs;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().expect("repo root")
}

fn read_repo_file(relative: &str) -> String {
    let path = repo_root().join(relative);
    fs::read_to_string(&path).unwrap_or_else(|err| panic!("read {}: {err}", path.display()))
}

/// The source files the fork added over the upstream baseline: every public
/// item in them is fork-new.
const FORK_NEW_FILES: [&str; 12] = [
    "crates/libs/rns-transport/src/destination/request/allow.rs",
    "crates/libs/rns-transport/src/destination/request/frame.rs",
    "crates/libs/rns-transport/src/destination/request/handler.rs",
    "crates/libs/rns-transport/src/destination/request/id.rs",
    "crates/libs/rns-transport/src/destination/request/inbound.rs",
    "crates/libs/rns-transport/src/destination/request/outcome.rs",
    "crates/libs/rns-transport/src/destination/request/receipt.rs",
    "crates/libs/rns-transport/src/resource/outcome.rs",
    "crates/libs/rns-transport/src/transport/links_parts/transport_sections/shared_instance.rs",
    "crates/libs/rns-transport/src/transport/links_parts/transport_sections/shared_instance_announce.rs",
    "crates/libs/rns-transport/src/transport/links_parts/transport_sections/shared_instance_reconnect.rs",
    "crates/libs/rns-transport/src/transport/shared_instance.rs",
];

fn crate_version() -> String {
    read_repo_file("crates/libs/rns-transport/Cargo.toml")
        .lines()
        .find_map(|line| line.strip_prefix("version = \""))
        .and_then(|rest| rest.strip_suffix('"'))
        .expect("coyote-rns-transport manifest should declare a package version")
        .to_string()
}

/// The crate's doc comments immediately above `line` (attributes between the
/// two are skipped), joined into one string; empty when there is no doc.
fn doc_block_above(lines: &[&str], line: usize) -> String {
    let mut docs = Vec::new();
    let mut at = line;
    while at > 0 {
        at -= 1;
        let text = lines[at].trim_start();
        if text.starts_with("#[") || text.starts_with("#![") {
            continue;
        }
        match text.strip_prefix("///") {
            Some(doc) => docs.push(doc.trim()),
            None => break,
        }
    }
    docs.reverse();
    docs.join(" ")
}

/// A backticked `RNS/<path>.py:<line>` citation somewhere in `text`
/// (`RNS/Link.py:808`, `RNS/vendor/umsgpack.py:320-324,989`).
fn cites_python_line(text: &str) -> bool {
    text.match_indices("`RNS/").any(|(at, _)| {
        let rest = &text[at + "`RNS/".len()..];
        let Some(dot) = rest.find(".py:") else { return false };
        rest[..dot].chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '/')
            && rest[dot + ".py:".len()..].starts_with(|c: char| c.is_ascii_digit())
    })
}

/// A Python citation in the doc, or an explicit fork-only statement.
fn doc_cites_python_or_fork_only(doc: &str) -> bool {
    let lowered = doc.to_ascii_lowercase();
    lowered.contains("fork-only") || lowered.contains("fork only") || cites_python_line(doc)
}

/// `pub <ident>:`, the field of a struct body.
fn is_pub_field(text: &str) -> bool {
    text.strip_prefix("pub ").is_some_and(|rest| {
        let name_len = rest.chars().take_while(|c| c.is_alphanumeric() || *c == '_').count();
        name_len > 0 && rest[name_len..].starts_with(':')
    })
}

/// `text` starts with `prefix` and then `name` as a whole identifier.
fn declares(text: &str, prefix: &str, name: &str) -> bool {
    text.strip_prefix(prefix)
        .and_then(|rest| rest.strip_prefix(name))
        .is_some_and(|rest| !rest.starts_with(|c: char| c.is_alphanumeric() || c == '_'))
}

/// The lines one brace deep inside the body that opens on line `open`: an
/// enum's variants or a struct's fields, not the fields of a struct variant.
fn direct_members(lines: &[&str], open: usize) -> Vec<usize> {
    let mut depth = 0i32;
    let mut members = Vec::new();
    for (index, line) in lines.iter().enumerate().skip(open) {
        if index > open && depth == 1 {
            members.push(index);
        }
        depth += line.matches('{').count() as i32 - line.matches('}').count() as i32;
        if index > open && depth <= 0 {
            break;
        }
    }
    members
}

#[derive(Debug)]
enum ItemKind {
    Method,
    TypeAlias,
    Variant {
        enum_name: &'static str,
    },
    /// A field of a `pub struct` or of a struct variant; the latter's fields
    /// carry no `pub` of their own.
    Field {
        owner: &'static str,
    },
}

fn locate_named_item(lines: &[&str], kind: &ItemKind, name: &str) -> Option<usize> {
    let position =
        |found: &dyn Fn(&str) -> bool| lines.iter().position(|line| found(line.trim_start()));
    match kind {
        ItemKind::Method => position(&|text| {
            declares(text, "pub fn ", name) || declares(text, "pub async fn ", name)
        }),
        ItemKind::TypeAlias => position(&|text| declares(text, "pub type ", name)),
        ItemKind::Variant { enum_name } => {
            let open = position(&|text| declares(text, "pub enum ", enum_name))?;
            direct_members(lines, open)
                .into_iter()
                .find(|&index| declares(lines[index].trim_start(), "", name))
        }
        ItemKind::Field { owner } => {
            let open = position(&|text| {
                (declares(text, "pub struct ", owner) || declares(text, "", owner))
                    && text.contains('{')
            })?;
            direct_members(lines, open).into_iter().find(|&index| {
                let text = lines[index].trim_start();
                declares(text, "pub ", name) || declares(text, "", name)
            })
        }
    }
}

/// The attribute lines above the item definition starting at byte
/// `definition`: the text since the previous blank line.
fn attribute_block_above(source: &str, definition: usize) -> &str {
    let above = &source[..definition];
    &above[above.rfind("\n\n").map_or(0, |at| at + 2)..]
}

#[derive(Clone, Copy)]
enum Body {
    Enum,
    Struct,
    Trait,
}

/// Every public item the request layer and the outbound-outcome types added
/// over the upstream baseline (types, free and associated functions, enum
/// variants and `pub` fields) carries a doc comment that either cites its
/// Python counterpart as `RNS/<file>.py:<line>` or states that the item is
/// fork-only. Two passes: the request-layer and outcome files are new in the
/// fork, so every public item in them is scanned; the files that mix upstream
/// and fork items are checked at the named fork items alone, each located by
/// its declaration line.
#[test]
fn request_layer_public_items_cite_python_or_say_fork_only() {
    let item_keywords =
        ["fn ", "async fn ", "const fn ", "struct ", "enum ", "trait ", "type ", "const "];
    let named_items: &[(&str, ItemKind, &str)] = &[
        (
            "crates/libs/rns-transport/src/destination/link_parts/linkhandleresult.rs",
            ItemKind::Variant { enum_name: "LinkEvent" },
            "PacketProven",
        ),
        (
            "crates/libs/rns-transport/src/error.rs",
            ItemKind::Variant { enum_name: "RnsError" },
            "NoPath",
        ),
        (
            "crates/libs/rns-transport/src/transport/mod.rs",
            ItemKind::Field { owner: "AnnounceEvent" },
            "is_path_response",
        ),
        ("crates/libs/rns-transport/src/transport/mod.rs", ItemKind::Method, "link_id"),
        ("crates/libs/rns-transport/src/transport/mod.rs", ItemKind::Method, "is_sent"),
        (
            "crates/libs/rns-transport/src/resource.rs",
            ItemKind::Field { owner: "OutboundRejected" },
            "reason",
        ),
        (
            "crates/libs/rns-transport/src/transport/resource_accept_hook.rs",
            ItemKind::TypeAlias,
            "ResourceAcceptHook",
        ),
        (
            "crates/libs/rns-transport/src/transport/resource_accept_hook.rs",
            ItemKind::Method,
            "set_resource_accept_hook",
        ),
        (
            "crates/libs/rns-transport/src/destination_parts/destination.rs",
            ItemKind::Method,
            "register_request_handler",
        ),
        (
            "crates/libs/rns-transport/src/destination_parts/destination.rs",
            ItemKind::Method,
            "deregister_request_handler",
        ),
        (
            "crates/libs/rns-transport/src/transport/links_parts/transport_sections/request.rs",
            ItemKind::Method,
            "request",
        ),
        (
            "crates/libs/rns-transport/src/transport/links_parts/transport_sections/send_outcome.rs",
            ItemKind::Method,
            "send_resource_with_outcome",
        ),
        (
            "crates/libs/rns-transport/src/transport/links_parts/transport_sections/send_outcome.rs",
            ItemKind::Method,
            "send_request_resource_with_outcome",
        ),
        (
            "crates/libs/rns-transport/src/transport/links_parts/transport_sections/send_outcome.rs",
            ItemKind::Method,
            "send_response_resource_with_outcome",
        ),
        (
            "crates/libs/rns-transport/src/destination/link_parts/link_sections/response_packet.rs",
            ItemKind::Method,
            "expect_response",
        ),
        (
            "crates/libs/rns-transport/src/destination/link_parts/link_sections/response_packet.rs",
            ItemKind::Method,
            "response_size_allowed",
        ),
        (
            "crates/libs/rns-transport/src/resource/outcome.rs",
            ItemKind::Method,
            "track_prepared_with_outcome",
        ),
    ];

    let mut checked = 0usize;
    let mut undocumented = Vec::new();
    let mut uncited = Vec::new();
    let mut check = |lines: &[&str], index: usize, site: String| {
        checked += 1;
        let doc = doc_block_above(lines, index);
        if doc.is_empty() {
            undocumented.push(site);
        } else if !doc_cites_python_or_fork_only(&doc) {
            uncited.push(site);
        }
    };

    for file in FORK_NEW_FILES {
        let source = read_repo_file(file);
        let lines: Vec<&str> = source.lines().collect();
        // The public enum, struct or trait whose braces the walker is inside,
        // and how deep the current line sits in it. Trait methods carry no
        // `pub`, so they are recognised by the `fn` keyword alone.
        let mut body: Option<Body> = None;
        let mut depth = 0i32;
        for (index, raw) in lines.iter().enumerate() {
            let text = raw.trim_start();
            let site = format!("{file}:{}: {}", index + 1, text.trim_end());

            if let Some(kind) = body {
                let is_member = match (kind, depth) {
                    (Body::Enum, 1) => text.starts_with(|c: char| c.is_ascii_uppercase()),
                    (Body::Struct, 1) => is_pub_field(text),
                    (Body::Trait, 1) => text.starts_with("fn ") || text.starts_with("async fn "),
                    _ => false,
                };
                if is_member {
                    check(&lines, index, site.clone());
                }
                depth += text.matches('{').count() as i32;
                depth -= text.matches('}').count() as i32;
                if depth <= 0 && text.contains('}') {
                    body = None;
                }
            }

            let Some(after_pub) = text.strip_prefix("pub ") else { continue };
            if !item_keywords.iter().any(|keyword| after_pub.starts_with(keyword)) {
                continue; // `pub use` re-exports, `pub mod`, and struct fields in `pub(crate)` scope
            }
            check(&lines, index, site);
            let opened = if after_pub.starts_with("enum ") {
                Body::Enum
            } else if after_pub.starts_with("struct ") {
                Body::Struct
            } else if after_pub.starts_with("trait ") {
                Body::Trait
            } else {
                continue;
            };
            depth = text.matches('{').count() as i32 - text.matches('}').count() as i32;
            if depth > 0 {
                body = Some(opened);
            }
        }
    }

    for (file, kind, name) in named_items {
        let source = read_repo_file(file);
        let lines: Vec<&str> = source.lines().collect();
        let index = locate_named_item(&lines, kind, name)
            .unwrap_or_else(|| panic!("{file} should declare `{name}` as a {kind:?}"));
        check(&lines, index, format!("{file}:{}: {}", index + 1, lines[index].trim()));
    }

    assert!(
        checked >= 142,
        "the request layer should expose the receipt, frame, handler and outcome surface; found only {checked} public items"
    );
    assert!(
        undocumented.is_empty(),
        "every public request-layer item, enum variant and public field should carry a doc comment:\n{}",
        undocumented.join("\n")
    );
    assert!(
        uncited.is_empty(),
        "every public request-layer item should cite `RNS/<file>.py:<line>` or state it is fork-only:\n{}",
        uncited.join("\n")
    );
}

/// Every `pub struct` the fork introduced keeps its fields private, so a
/// consumer builds and reads it through methods alone: a brace body has no
/// `pub <ident>:` member and a one-line tuple body has no `pub ` between its
/// parentheses. The set of structs found is pinned to the expected names so a
/// new or renamed public struct shows up here. Two fork-new fields in upstream
/// structs are checked by name: `DeliveryReceipt::link_id` must be private.
/// `AnnounceEvent` predates the fork and exposes every field, so its
/// `is_path_response` follows that shape and is asserted public, together
/// with the `#[non_exhaustive]` attribute that keeps the public field from
/// breaking out-of-crate struct literals.
#[test]
fn fork_new_public_structs_keep_their_fields_private() {
    let expected = [
        "InboundRequest",
        "OutboundOutcome",
        "PathHash",
        "RequestFrame",
        "RequestId",
        "RequestOptions",
        "RequestReceipt",
        "RequestResponseData",
        "ResponseFrame",
        "ResponseOutcomeReceiver",
        "SharedInstanceOptions",
    ];
    let mut found = Vec::new();
    let mut exposed = Vec::new();
    for file in FORK_NEW_FILES {
        let source = read_repo_file(file);
        let lines: Vec<&str> = source.lines().collect();
        for (open, raw) in lines.iter().enumerate() {
            let Some(rest) = raw.trim_start().strip_prefix("pub struct ") else { continue };
            let name: String =
                rest.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
            let offenders: Vec<usize> = if rest.contains('{') {
                direct_members(&lines, open)
                    .into_iter()
                    .filter(|&index| is_pub_field(lines[index].trim_start()))
                    .collect()
            } else {
                let body = rest
                    .split_once('(')
                    .and_then(|(_, after)| after.rsplit_once(')'))
                    .map(|(body, _)| body)
                    .unwrap_or_else(|| {
                        panic!(
                            "{file}:{}: `{name}` should be a brace or one-line tuple struct",
                            open + 1
                        )
                    });
                if body.contains("pub ") {
                    vec![open]
                } else {
                    Vec::new()
                }
            };
            for index in offenders {
                exposed.push(format!(
                    "{file}:{}: `{name}` exposes `{}`",
                    index + 1,
                    lines[index].trim()
                ));
            }
            found.push(name);
        }
    }
    found.sort_unstable();
    assert_eq!(
        found, expected,
        "the fork-new files should declare exactly the expected public structs"
    );
    assert!(
        exposed.is_empty(),
        "every public struct the fork introduced should keep its fields private:\n{}",
        exposed.join("\n")
    );

    let file = "crates/libs/rns-transport/src/transport/mod.rs";
    let source = read_repo_file(file);
    let lines: Vec<&str> = source.lines().collect();
    let field_line = |owner: &'static str, name: &str| {
        locate_named_item(&lines, &ItemKind::Field { owner }, name)
            .unwrap_or_else(|| panic!("{file} should declare the field `{owner}::{name}`"))
    };
    let link_id = field_line("DeliveryReceipt", "link_id");
    assert!(
        !is_pub_field(lines[link_id].trim_start()),
        "{file}:{}: `DeliveryReceipt::link_id` should be private, got `{}`",
        link_id + 1,
        lines[link_id].trim()
    );
    let is_path_response = field_line("AnnounceEvent", "is_path_response");
    assert!(
        is_pub_field(lines[is_path_response].trim_start()),
        "{file}:{}: `AnnounceEvent::is_path_response` should be public like the struct's upstream fields, got `{}`",
        is_path_response + 1,
        lines[is_path_response].trim()
    );
    let definition = source
        .find("pub struct AnnounceEvent ")
        .unwrap_or_else(|| panic!("{file} should define `AnnounceEvent`"));
    let attributes = attribute_block_above(&source, definition);
    assert!(
        attributes.lines().any(|line| line.trim() == "#[non_exhaustive]"),
        "{file}: `AnnounceEvent` exposes a public fork field, so it must be `#[non_exhaustive]`:\n{attributes}"
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
/// exactly one `rust` fence, the attach, handler, announce and detach
/// snippet, and no fence of any other kind; every line of that snippet is
/// run verbatim by `crates/libs/rns-transport/tests/shared_instance_api.rs`
/// against an in-process shared instance, under the same squash and skip
/// rules as `crate_readme_rust_snippets_are_pinned_verbatim_by_the_api_tests`.
#[test]
fn crate_readme_shared_instance_section_fence_is_pinned_by_the_api_test() {
    let readme = read_repo_file("crates/libs/rns-transport/README.md");
    let section = crate_readme_section(&readme, "Attaching to a shared instance");
    let fences: Vec<&str> =
        section.lines().map(str::trim_start).filter(|line| line.starts_with("```")).collect();
    assert_eq!(
        fences,
        vec!["```rust", "```"],
        "the shared-instance section should carry exactly one rust fence and no other"
    );
    let snippet: Vec<&str> = section
        .lines()
        .skip_while(|line| line.trim_start() != "```rust")
        .skip(1)
        .take_while(|line| line.trim_start() != "```")
        .collect();
    assert!(!snippet.is_empty(), "the shared-instance snippet should not be empty");

    let pin = squash(&read_repo_file("crates/libs/rns-transport/tests/shared_instance_api.rs"));
    let unpinned: Vec<&str> = snippet
        .iter()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty() && !line.starts_with("use ") && !line.starts_with("//"))
        .filter(|line| {
            let needle = match line.split_once(" => ") {
                Some((pattern, _)) => squash(pattern),
                None => squash(line),
            };
            !pin.contains(&needle)
        })
        .collect();
    assert!(
        unpinned.is_empty(),
        "every line of the shared-instance snippet should appear verbatim in tests/shared_instance_api.rs:\n{}",
        unpinned.join("\n")
    );
}

/// Snippet text with every whitespace character removed and a pin's leading
/// underscore on an unused pattern binding dropped, the form the README
/// snippet lines are matched against the public-API pin files in.
fn squash(text: &str) -> String {
    text.chars().filter(|c| !c.is_whitespace()).collect::<String>().replace("(_", "(")
}

/// The body of the crate README section under `## <heading>`, up to the next
/// `## ` heading.
fn crate_readme_section<'a>(readme: &'a str, heading: &str) -> &'a str {
    let marker = format!("\n## {heading}\n");
    let start = readme
        .find(&marker)
        .unwrap_or_else(|| panic!("crate README should carry a `## {heading}` section"));
    let section = &readme[start + marker.len()..];
    match section.find("\n## ") {
        Some(end) => &section[..end],
        None => section,
    }
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

/// The request-handler section describes `track_outcome` the way the
/// dispatcher behaves: the receiver it returns, when to call it, the three
/// outcomes and what each one proves, the proof section a packet outcome
/// defers to, the stop that resolves `NotSent`, and the Python it diverges
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
        "`RNS/Link.py:846-852`",
    ] {
        assert!(
            section.contains(claim),
            "the request-handler section should mention {claim}:\n{section}"
        );
    }
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
        let breaking_start =
            section.iter().position(|line| *line == "### Breaking").expect("located above") + 1;
        let breaking_end = section[breaking_start..]
            .iter()
            .position(|line| line.starts_with("### "))
            .map_or(section.len(), |p| breaking_start + p);
        let breaking = &section[breaking_start..breaking_end];
        // A bullet starts at a top-level `- ` line and runs through the
        // indented lines that follow it.
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

/// The top-level `- ` bullets of a markdown text, each joined with its
/// two-space-indented continuation lines.
fn top_level_bullets(markdown: &str) -> Vec<String> {
    let mut bullets: Vec<Vec<&str>> = Vec::new();
    for line in markdown.lines() {
        if line.starts_with("- ") {
            bullets.push(vec![line]);
        } else if line.starts_with("  ") {
            if let Some(bullet) = bullets.last_mut() {
                bullet.push(line);
            }
        }
    }
    bullets.iter().map(|bullet| bullet.join("\n")).collect()
}

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

/// The README patch list opens with the 0.16.0 block and keeps the 0.15.1 and
/// 0.15.0 blocks below it as history: the 0.16.0 block names every item of
/// the release, and the two 0.15.1 sentences it supersedes (the no-op
/// duplicate filter and the once-per-attach warning) survive verbatim in the
/// 0.15.1 block rather than being edited away.
#[test]
fn readme_patch_list_opens_with_the_0_16_0_block_and_keeps_the_0_15_1_paragraph() {
    let readme = read_repo_file("crates/libs/rns-transport/README.md");
    let patch_list = crate_readme_section(&readme, "Patch list")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let marker_0_16_0 = "What `0.16.0` carries on top of `0.15.1`:";
    let marker_0_15_1 = "What `0.15.1` carries on top of `0.15.0`:";
    let marker_0_15_0 = "What `0.15.0` carries on top of `0.14.1`:";
    let at_0_16_0 = patch_list
        .find(marker_0_16_0)
        .expect("the README patch list should open with the 0.16.0 block");
    let at_0_15_1 =
        patch_list.find(marker_0_15_1).expect("the README patch list should keep the 0.15.1 block");
    let at_0_15_0 =
        patch_list.find(marker_0_15_0).expect("the README patch list should keep the 0.15.0 block");
    assert!(
        at_0_16_0 < at_0_15_1 && at_0_15_1 < at_0_15_0,
        "the README patch list should run 0.16.0, then 0.15.1, then 0.15.0"
    );

    let block_0_16_0 = &patch_list[at_0_16_0..at_0_15_1];
    for needle in [
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
    ] {
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
        "crates/libs/rns-transport/tests/shared_instance_api.rs",
        "crates/libs/rns-transport/tests/destination_desc_api.rs",
    ]
    .iter()
    .map(|path| read_repo_file(path))
    .collect::<Vec<_>>()
    .join("\n");
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
        snippets.len() >= 6,
        "the README should carry the consumer, handler, request, outcome, link-opening and shared-instance snippets, found {}",
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

/// Every public enum the request layer and the outcome types added, and every
/// enum the README or the release notes call `#[non_exhaustive]`, carries the
/// attribute on its definition, so the "keep a wildcard arm" guidance the
/// consumer reads is what the compiler enforces.
#[test]
fn enums_the_docs_call_non_exhaustive_carry_the_attribute() {
    let sites = [
        ("RequestError", "crates/libs/rns-transport/src/destination/request/receipt.rs"),
        ("RequestStatus", "crates/libs/rns-transport/src/destination/request/receipt.rs"),
        ("RequestAllow", "crates/libs/rns-transport/src/destination/request/allow.rs"),
        ("RequestResponse", "crates/libs/rns-transport/src/destination/request/handler.rs"),
        ("FrameDecodeError", "crates/libs/rns-transport/src/destination/request/frame.rs"),
        ("ResponseOutcome", "crates/libs/rns-transport/src/destination/request/outcome.rs"),
        ("RnsError", "crates/libs/rns-transport/src/error.rs"),
        ("OutboundResult", "crates/libs/rns-transport/src/resource/outcome.rs"),
        ("OutboundFailure", "crates/libs/rns-transport/src/resource/outcome.rs"),
        ("RejectReason", "crates/libs/rns-transport/src/resource/outcome.rs"),
        ("LinkEvent", "crates/libs/rns-transport/src/destination/link_parts/linkhandleresult.rs"),
        ("ResourceEventKind", "crates/libs/rns-transport/src/resource.rs"),
        ("AnnounceEvent", "crates/libs/rns-transport/src/transport/mod.rs"),
        ("SharedInstanceEndpoint", "crates/libs/rns-transport/src/transport/shared_instance.rs"),
    ];
    for (item, file) in sites {
        let source = read_repo_file(file);
        let definition = source
            .find(&format!("pub enum {item} "))
            .or_else(|| source.find(&format!("pub struct {item} ")))
            .unwrap_or_else(|| panic!("{file} should define `{item}`"));
        let attributes = attribute_block_above(&source, definition);
        assert!(
            attributes.lines().any(|line| line.trim() == "#[non_exhaustive]"),
            "{file}: `{item}` is documented as `#[non_exhaustive]` but its definition lacks the attribute:\n{attributes}"
        );
    }
}
