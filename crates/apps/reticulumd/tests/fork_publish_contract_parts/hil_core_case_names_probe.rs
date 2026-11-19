// The hil core cases run `cargo test -p coyote-rns-transport --lib <name>`,
// and `cargo test` exits 0 when its filter matches no test, so a case whose
// name has drifted from the test it points at passes with nothing run; the
// repository has met that once already, when the link-close case had to be
// pointed back at the real test name. Every such case must name a function
// that exists under the crate's sources, and a case that runs an ignored
// test must be named by that test's ignore reason, so the lane is visible at
// the site.

/// The `(case id, test name, runs ignored)` triple of every hil core case
/// whose command is `cargo test -p coyote-rns-transport --lib <name> ...`.
fn hil_core_lib_cases() -> Vec<(String, String, bool)> {
    let matrix: toml::Value =
        toml::from_str(&read_repo_file("tests/hil/cases/core.toml")).expect("parse core.toml");
    let cases = matrix["cases"].as_array().expect("core.toml carries [[cases]]");
    cases
        .iter()
        .filter_map(|case| {
            let id = case["id"].as_str().expect("case id").to_string();
            let command = &case["command"];
            if command["program"].as_str() != Some("cargo") {
                return None;
            }
            let args: Vec<&str> = command["args"]
                .as_array()
                .expect("case args")
                .iter()
                .map(|arg| arg.as_str().expect("string arg"))
                .collect();
            if args.len() < 3 || args[..3] != ["test", "-p", "coyote-rns-transport"] {
                return None;
            }
            let lib = args.iter().position(|arg| *arg == "--lib")?;
            let name = args.get(lib + 1).filter(|name| !name.starts_with("--"))?;
            Some((id, name.to_string(), args.contains(&"--ignored")))
        })
        .collect()
}

/// The tracked Rust source under `crates/libs/rns-transport/src` that
/// defines `fn <name>(`, with its text.
fn rns_transport_source_defining(name: &str) -> Option<(String, String)> {
    let needle = format!("fn {name}(");
    git_tracked_files()
        .into_iter()
        .filter(|path| path.starts_with("crates/libs/rns-transport/src/") && path.ends_with(".rs"))
        .map(|path| {
            let text = read_repo_file(&path);
            (path, text)
        })
        .find(|(_, text)| text.contains(&needle))
}

/// The `#[ignore ...]` attribute line in the attribute and doc block right
/// above `fn <name>(`, if any.
fn ignore_attribute_on(text: &str, name: &str) -> Option<String> {
    let needle = format!("fn {name}(");
    let lines: Vec<&str> = text.lines().collect();
    let fn_line = lines.iter().position(|line| line.contains(&needle))?;
    lines[..fn_line]
        .iter()
        .rev()
        .take_while(|line| {
            let trimmed = line.trim_start();
            trimmed.starts_with("#[") || trimmed.starts_with("//")
        })
        .find(|line| line.trim_start().starts_with("#[ignore"))
        .map(|line| line.trim().to_string())
}

#[test]
fn hil_core_lib_cases_name_tests_that_exist_and_ignored_ones_name_their_case() {
    let cases = hil_core_lib_cases();
    assert!(
        cases.iter().any(|(id, _, _)| id == "auto-peer-count-timeout"),
        "the core matrix carries the auto-peer-count-timeout case: {cases:?}"
    );
    for (id, name, runs_ignored) in cases {
        let (path, text) = rns_transport_source_defining(&name).unwrap_or_else(|| {
            panic!("hil core case {id} names `{name}`, which no rns-transport source defines")
        });
        match (runs_ignored, ignore_attribute_on(&text, &name)) {
            (true, Some(attribute)) => assert!(
                attribute.contains(&id),
                "{path}: the ignore reason on `{name}` names its hil case {id}: {attribute}"
            ),
            (true, None) => {
                panic!("{path}: hil case {id} runs `{name}` with --ignored but it is not ignored")
            }
            (false, Some(attribute)) => {
                panic!("{path}: `{name}` is ignored ({attribute}) but hil case {id} runs it plainly")
            }
            (false, None) => {}
        }
    }
}
