// Usage probes written from the acceptance text before the sites
// were read: "every marker names its guard + await" and the
// public `run_kiss_stream_with_ifac` is "restored to per-pass (now
// per-receive) locking of its shared receivers". probe.rs pins the owner tag
// and a non-empty reason; these two pins add the two halves the text
// promises: the reason names a guard and the await it spans, and the kiss
// restore moved the only kiss allowance off the public wrapper onto the
// per-receive `recv` (a wrapper-level allowance is what a guard held for the
// life of the stream needs). Needles are assembled so this file never
// contains the attribute text it scans for.

/// Words a reason uses to name the guard it holds.
const GUARD_NOUNS: [&str; 3] = ["guard", "Guard", "lock"];

/// Words a reason uses to name the await the guard spans.
const AWAIT_VERBS: [&str; 5] = ["await", "recv", "spans", "spanning", "across"];

/// The two sanctioned categories whose reasons name the guard by its
/// category rather than by a code identifier.
const CATEGORY_PHRASES: [&str; 2] =
    ["public-API-pinned shared receiver", "documented lock-order nest"];

/// Whether a token reads as a code identifier: a path, call, field access,
/// snake_case name or CamelCase type. Prose such as "guard-across-await
/// site" or "public signature" has none of these.
fn is_code_identifier(token: &str) -> bool {
    if matches!(token.trim_end_matches('.'), "e.g" | "i.e") {
        return false;
    }
    if token.contains("::") || token.contains("()") || token.contains('_') {
        return true;
    }
    if token.split('.').filter(|part| !part.is_empty()).count() > 1
        && token.chars().all(|c| c.is_ascii_alphanumeric() || c == '.')
    {
        return true;
    }
    let mut saw_lower = false;
    for c in token.chars() {
        if c.is_ascii_lowercase() {
            saw_lower = true;
        } else if c.is_ascii_uppercase() && saw_lower {
            return true;
        }
    }
    false
}

/// Whether a reason names something concrete: a code identifier or one of the
/// sanctioned category phrases.
fn names_a_concrete_site(reason: &str) -> bool {
    CATEGORY_PHRASES.iter().any(|phrase| reason.contains(phrase))
        || reason
            .split(|c: char| c.is_whitespace() || matches!(c, ',' | ';' | '(' | ')'))
            .any(|token| !token.is_empty() && token != "()" && is_code_identifier(token))
}

/// Every production allowance's reason names both the guard and the await it
/// spans and points at a concrete site, not the generic "restructure this
/// guard-across-await site" placeholder.
#[test]
fn every_production_allowance_reason_names_a_guard_and_the_await_it_spans() {
    let mut sites = 0_usize;
    let mut vague: Vec<String> = Vec::new();
    for path in tracked_rust_sources() {
        if is_test_or_example_path(&path) {
            continue;
        }
        let text = read_repo_file(&path);
        for (index, line) in text.lines().enumerate() {
            if !is_site_allowance(line) {
                continue;
            }
            sites += 1;
            let reason = line.split_once(OWNER_TAG).map(|(_, r)| r.trim()).unwrap_or_default();
            let names_guard = GUARD_NOUNS.iter().any(|noun| reason.contains(noun));
            let names_await = AWAIT_VERBS.iter().any(|verb| reason.contains(verb));
            if !(names_guard && names_await && names_a_concrete_site(reason)) {
                vague.push(format!("{path}:{} {reason:?}", index + 1));
            }
        }
    }
    // The floor tracks the measured census (52 at the time of writing) and
    // moves down as each owner-tagged site is removed.
    assert!(sites >= 50, "expected the measured production allowances, found {sites}");
    assert!(
        vague.is_empty(),
        "every allowance reason names the guard it holds and the await it spans, these do not: {vague:#?}"
    );
}

/// The kiss restore: the public shared-receiver wrapper carries no
/// allowance of its own (nothing above it holds a guard across the loop), the
/// one allowance under `kiss_parts/` sits on the per-receive `recv`, and it
/// claims the sanctioned public-API-pinned category.
#[test]
fn kiss_shared_receiver_wrapper_holds_no_allowance_and_the_lease_recv_holds_the_only_one() {
    let path = "crates/libs/rns-transport/src/iface/kiss_parts/kisstcpclientinterface.rs";
    let text = read_repo_file(path);
    let lines: Vec<&str> = text.lines().collect();
    let wrapper = lines
        .iter()
        .position(|l| l.starts_with("pub async fn run_kiss_stream_with_ifac"))
        .expect("the public shared-receiver wrapper is still exported");
    let attributes_above: Vec<&str> = lines[..wrapper]
        .iter()
        .rev()
        .take_while(|l| l.trim_start().starts_with("#[") || l.trim_start().starts_with("///"))
        .copied()
        .collect();
    assert!(
        !attributes_above.iter().any(|l| is_site_allowance(l)),
        "run_kiss_stream_with_ifac carries no allowance of its own, found {attributes_above:?}"
    );

    let sites: Vec<(usize, &str)> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| is_site_allowance(l))
        .map(|(i, l)| (i, *l))
        .collect();
    assert_eq!(sites.len(), 1, "exactly one allowance under kiss_parts, found {sites:?}");
    let (index, marker) = sites[0];
    let item = lines[index + 1..]
        .iter()
        .find(|l| !l.trim_start().starts_with("#["))
        .copied()
        .unwrap_or_default();
    assert!(
        item.contains("fn recv("),
        "the kiss allowance sits on the lease's per-receive recv, not on `{}`",
        item.trim()
    );
    assert!(
        marker.contains("public-API-pinned shared receiver"),
        "the kiss allowance claims the sanctioned public-API-pinned category, got {marker:?}"
    );
    let shared_arm = lines[index..(index + 8).min(lines.len())].join("\n");
    assert!(
        shared_arm.contains(".lock().await.recv().await"),
        "the shared arm takes the guard as a temporary spanning only the recv: {shared_arm}"
    );
}
