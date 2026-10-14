use std::fs;
use std::path::{Path, PathBuf};

const PRINT_MACROS: [&str; 5] = ["print!(", "println!(", "eprint!(", "eprintln!(", "dbg!("];

fn is_test_dir(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name == "tests" || name.ends_with("_tests"))
}

fn is_test_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name == "tests.rs" || name.ends_with("_tests.rs"))
}

fn collect_library_sources(dir: &Path, files: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(dir)
        .unwrap_or_else(|err| panic!("failed to read directory {}: {err}", dir.display()));
    for entry in entries {
        let path = entry
            .unwrap_or_else(|err| {
                panic!("failed to read directory entry in {}: {err}", dir.display())
            })
            .path();
        if path.is_dir() {
            if !is_test_dir(&path) {
                collect_library_sources(&path, files);
            }
            continue;
        }
        if path.extension().and_then(|ext| ext.to_str()) == Some("rs") && !is_test_file(&path) {
            files.push(path);
        }
    }
}

fn brace_delta(line: &str) -> isize {
    line.chars().fold(0, |depth, ch| match ch {
        '{' => depth + 1,
        '}' => depth - 1,
        _ => depth,
    })
}

fn opens_mod_block(trimmed: &str) -> bool {
    let after_visibility = trimmed
        .strip_prefix("pub(crate) ")
        .or_else(|| trimmed.strip_prefix("pub "))
        .unwrap_or(trimmed);
    after_visibility.starts_with("mod ") && trimmed.contains('{')
}

/// Yields the lines of `source` that are compiled into the library, dropping
/// the body of every `mod tests { ... }` and `#[cfg(test)] mod ... { ... }`
/// block. Brace depth is counted per character, so a stray brace inside a
/// string literal can cut a test block short or long; that is good enough for
/// a print-macro sweep. Test sections that are `include!`-ed from outside a
/// `tests` or `*_tests` directory (such as `*_sections/` or `tests_parts/`)
/// are swept as library code, so a print there fails this test loudly rather
/// than slipping through.
fn library_lines(source: &str) -> Vec<(usize, &str)> {
    let mut kept = Vec::new();
    let mut skip_depth: isize = 0;
    let mut pending_cfg_test = false;

    for (index, line) in source.lines().enumerate() {
        if skip_depth > 0 {
            skip_depth += brace_delta(line);
            continue;
        }

        let trimmed = line.trim_start();
        if trimmed.starts_with("#[cfg(test)]") {
            pending_cfg_test = true;
            continue;
        }
        let gated_mod = pending_cfg_test && opens_mod_block(trimmed);
        if !trimmed.starts_with("#[") {
            pending_cfg_test = false;
        }
        if gated_mod || (trimmed.starts_with("mod tests") && trimmed.contains('{')) {
            skip_depth = brace_delta(line);
            continue;
        }

        kept.push((index + 1, line));
    }

    kept
}

fn print_macro_hits(path: &Path) -> Vec<String> {
    let source = fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
    library_lines(&source)
        .into_iter()
        .filter(|(_, line)| {
            let trimmed = line.trim_start();
            !trimmed.starts_with("//") && PRINT_MACROS.iter().any(|m| trimmed.contains(m))
        })
        .map(|(number, line)| format!("{}:{number}: {}", path.display(), line.trim()))
        .collect()
}

#[test]
fn no_non_test_stdout_or_stderr_prints_in_library_sources() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    collect_library_sources(&src, &mut files);
    assert!(!files.is_empty(), "no library sources found under {}", src.display());

    let hits: Vec<String> = files.iter().flat_map(|path| print_macro_hits(path)).collect();
    assert!(
        hits.is_empty(),
        "library code must log instead of printing to stdout or stderr:\n{}",
        hits.join("\n")
    );
}

#[test]
fn library_lines_skip_inline_test_modules() {
    let source = "\
fn a() {}
#[cfg(test)]
mod tests {
    fn b() { if x { y } }
}
fn c() {}
mod tests {
    fn d() {}
}
#[cfg(test)]
#[rustfmt::skip]
pub(crate) mod helpers { fn e() {} }
fn f() {}
";
    let kept: Vec<&str> = library_lines(source).into_iter().map(|(_, line)| line).collect();
    assert_eq!(kept, ["fn a() {}", "fn c() {}", "#[rustfmt::skip]", "fn f() {}"]);
}
