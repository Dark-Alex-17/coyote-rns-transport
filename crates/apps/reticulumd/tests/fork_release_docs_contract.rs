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

/// The bodies of the `rust` fences in `text`, in order, each as its lines
/// between the opening and closing fence.
fn rust_snippets(text: &str) -> Vec<Vec<&str>> {
    let mut snippets = Vec::new();
    let mut current: Option<Vec<&str>> = None;
    for line in text.lines() {
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
    snippets
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

include!("fork_release_docs_contract_parts/request_layer_api_docs.rs");

include!("fork_release_docs_contract_parts/crate_readme_sections.rs");

include!("fork_release_docs_contract_parts/release_notes_breaking.rs");

include!("fork_release_docs_contract_parts/readme_patch_list.rs");

include!("fork_release_docs_contract_parts/release_0_17_0_probe.rs");
