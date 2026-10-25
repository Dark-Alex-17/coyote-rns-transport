// Usage pin on the two-guard acquisition rule in the function-call form:
// the sibling probe refuses `join!`/`try_join!` over lock futures, but
// `futures` is a dependency of the transport and daemon crates, so the
// same hazard is writable as `futures::future::join(a.lock(), b.lock())`,
// `join3`, `join_all` or their `try_` twins. Those park the first-won guard
// inside the combinator while the other lock pends and rotate the poll
// order exactly like the macro does, so no production source may do it
// either.

/// Free-function combinators that poll several futures concurrently and
/// keep the finished ones' output (a guard) parked until all are done.
const JOIN_FUNCTIONS: [&str; 10] = [
    "join(",
    "join3(",
    "join4(",
    "join5(",
    "try_join(",
    "try_join3(",
    "try_join4(",
    "try_join5(",
    "join_all(",
    "try_join_all(",
];

/// Whether `line` calls one of `JOIN_FUNCTIONS` as a free function (bare or
/// path-qualified), not as a method such as `Path::join` / `JoinHandle::join`
/// and not as a definition.
fn calls_join_function(line: &str) -> bool {
    let trimmed = line.trim_start();
    if trimmed.starts_with("fn ") || trimmed.starts_with("pub fn ") || trimmed.starts_with("//") {
        return false;
    }
    JOIN_FUNCTIONS.iter().any(|name| {
        line.match_indices(name).any(|(at, _)| {
            let before = line[..at].chars().next_back();
            !matches!(before, Some(c) if c == '.' || c == '_' || c.is_ascii_alphanumeric())
        })
    })
}

/// No production source joins lock futures through a combinator function
/// any more than through the macro: the rule is about concurrent
/// acquisition of two guards, whatever spelling reaches it.
#[test]
fn no_production_source_in_any_crate_joins_lock_futures_by_function_either() {
    let lock_calls = [".lock()", ".read()", ".write()", ".lock_owned()"];
    let mut offenders: Vec<String> = Vec::new();
    for path in tracked_rust_sources() {
        if is_test_or_example_path(&path) || path.ends_with("lint_discipline_probe11.rs") {
            continue;
        }
        let text = read_repo_file(&path);
        let lines: Vec<&str> = text.lines().collect();
        for (index, line) in lines.iter().enumerate() {
            if !calls_join_function(line) {
                continue;
            }
            let window = lines[index..(index + 4).min(lines.len())].join("\n");
            if lock_calls.iter().any(|call| window.contains(call)) {
                offenders.push(format!("{path}:{}", index + 1));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "production code joins lock futures through a combinator function at {offenders:?}"
    );
}

/// The classifier itself: method joins and definitions are not calls, free
/// and path-qualified calls are.
#[test]
fn join_function_classifier_separates_free_calls_from_methods_and_definitions() {
    for call in [
        "let (a, b) = join(state.lock(), dedupe.lock()).await;",
        "futures::future::join3(x.lock(), y.lock(), z.lock()).await",
        "    try_join_all(locks.iter().map(|m| m.lock())).await",
    ] {
        assert!(calls_join_function(call), "a free join call: {call:?}");
    }
    for not_call in [
        "let path = dir.join(name);",
        "thread.join().expect(\"worker\");",
        "fn join(&self) -> Result<(), Error> {",
        "// join(a.lock(), b.lock()) would park a guard",
        "let rejoin(x) = y;",
    ] {
        assert!(!calls_join_function(not_call), "not a free join call: {not_call:?}");
    }
}
