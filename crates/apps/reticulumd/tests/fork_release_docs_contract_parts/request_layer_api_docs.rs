// Rustdoc and visibility pins over the public items the fork added: every
// request-layer item cites Python or says fork-only, fork-new structs keep
// their fields private, and the enums the docs call `#[non_exhaustive]`
// carry the attribute.

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
            "crates/libs/rns-transport/src/destination_parts/destination.rs",
            ItemKind::Method,
            "set_max_response_size",
        ),
        (
            "crates/libs/rns-transport/src/destination_parts/destination.rs",
            ItemKind::Method,
            "clear_max_response_size",
        ),
        (
            "crates/libs/rns-transport/src/destination_parts/destination.rs",
            ItemKind::Method,
            "max_response_size",
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
        checked >= 154,
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
        "ResponseOutcomeTracker",
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
        ("SharedInstanceEvent", "crates/libs/rns-transport/src/transport/shared_instance.rs"),
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
