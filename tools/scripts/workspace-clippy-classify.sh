#!/usr/bin/env bash
set -euo pipefail

# CI-equivalent clippy classification. Runs the workspace clippy invocation
# CI uses (all targets, all features, -D warnings) with JSON output and
# classifies every diagnostic. await_holding_invalid_type is live at its
# default level: no lint table mentions it, production code allows it only
# per fn with a same-line comment naming the owner, and each test root allows
# it once at module level. Passes only when no Cargo.toml mentions the lint,
# no await_holding_invalid_type diagnostic fires at any level and every
# error-level diagnostic is the dead_code that macOS produces in the
# linux-only reticulumd python_channel_interop test target (the two files
# listed below). That tolerance applies on Darwin only: on Linux, where CI
# runs, no error-level diagnostic is tolerated.
# Error-level diagnostics without a code (parse, macro and linker errors)
# are never tolerated. The one codeless shape skipped is rustc's
# "aborting due to N previous errors" summary, which closes every failed
# crate including the tolerated dead_code one and only restates errors
# already classified; cargo's "could not compile" wrapper is skipped on the
# same grounds should it ever arrive as a compiler-message.
# Requires python3 for the classification pass.

if [ $# -ne 0 ]; then
  echo "usage: $0" >&2
  exit 2
fi

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

manifests="$(mktemp "${TMPDIR:-/tmp}/clippy-manifests.XXXXXX")"
json="$(mktemp "${TMPDIR:-/tmp}/clippy-json.XXXXXX")"
stderr_log="$(mktemp "${TMPDIR:-/tmp}/clippy-stderr.XXXXXX")"
trap 'rm -f "$manifests" "$json" "$stderr_log"' EXIT

git ls-files -z -- 'Cargo.toml' '*/Cargo.toml' >"$manifests"
# One grep over the whole list rather than xargs: xargs folds grep's "no
# match" (1) and "unreadable file" (2) into the same 123, and only the former
# may pass.
manifest_list=()
while IFS= read -r -d '' manifest; do
  manifest_list+=("$manifest")
done <"$manifests"
rc=0
manifest_hits="$(grep -l 'await_holding_invalid_type' -- ${manifest_list[@]+"${manifest_list[@]}"})" || rc=$?
[ "$rc" -le 1 ] || exit "$rc"
if [ -n "$manifest_hits" ]; then
  echo "FAIL: a Cargo.toml mentions await_holding_invalid_type; the lint runs at its default level and allowances live in code:"
  while IFS= read -r hit; do
    echo "  $hit"
  done <<<"$manifest_hits"
  exit 1
fi

rc=0
cargo clippy --workspace --all-targets --all-features --no-deps --message-format=json -- -D warnings >"$json" 2>"$stderr_log" || rc=$?
echo "cargo clippy (CI invocation) exit: $rc"
if [ "$rc" -ne 0 ]; then
  echo "cargo clippy stderr (last 40 lines):"
  tail -n 40 "$stderr_log"
fi

python3 - "$json" "$rc" "$(uname -s)" <<'PY'
import collections
import json
import sys

path = sys.argv[1]
clippy_rc = int(sys.argv[2])
host = sys.argv[3]
# The dead_code only appears where the linux-only test target is not built,
# so the tolerance is populated on Darwin alone; CI on Linux tolerates nothing.
allowed_files = set()
tolerance = "none"
if host == "Darwin":
    allowed_files = {
        "crates/apps/reticulumd/tests/python_channel_interop_parts/resource_memory_profile.rs",
        "crates/apps/reticulumd/tests/support/python_channel_process.rs",
    }
    tolerance = "the macOS-only dead_code in the linux-only reticulumd test target"


def restates_counted_errors(text):
    return text.startswith("aborting due to") or text.startswith("could not compile")


await_hits = 0
errors = []
codeless = []
codes = collections.Counter()
with open(path) as fh:
    for line in fh:
        try:
            m = json.loads(line)
        except json.JSONDecodeError:
            print(f"FAIL: non-JSON line in cargo clippy output: {line.rstrip()!r}")
            sys.exit(1)
        if m.get("reason") != "compiler-message":
            continue
        msg = m.get("message", {})
        code = (msg.get("code") or {}).get("code") or ""
        level = msg.get("level")
        if "await_holding_invalid_type" in code:
            await_hits += 1
        if level != "error":
            continue
        if code:
            spans = [s for s in msg.get("spans", []) if s.get("is_primary")]
            files = {s.get("file_name") for s in spans}
            codes[code] += 1
            errors.append((code, files))
            continue
        text = msg.get("message") or ""
        if not restates_counted_errors(text):
            codeless.append(text)
print(f"await_holding_invalid_type diagnostics (any level): {await_hits}")
print(f"error-level diagnostics by code: {dict(codes)}")
print(f"codeless error-level diagnostics: {len(codeless)}")
print(f"tolerated errors ({host}): {tolerance}")
bad = [e for e in errors if e[0] != "dead_code" or not e[1] or not e[1] <= allowed_files]
for code, files in errors:
    print(f"  error {code}: {sorted(files)}")
for text in codeless:
    print(f"  error (no code): {text}")
if await_hits:
    print("FAIL: await_holding_invalid_type fired outside the per-target allows (a production site or a new test root)")
    sys.exit(1)
if clippy_rc != 0 and not errors and not codeless:
    print(f"FAIL: cargo clippy exited {clippy_rc} without any error-level diagnostic; the invocation itself failed")
    sys.exit(1)
if codeless:
    print(f"FAIL: {len(codeless)} codeless error-level diagnostic(s), a compile or link failure rather than a lint: {codeless[0]}")
    sys.exit(1)
if bad:
    print(f"FAIL: {len(bad)} error diagnostic(s) outside the tolerated set ({host}: {tolerance})")
    sys.exit(1)
if errors:
    print(f"OK: zero await_holding_invalid_type diagnostics with the lint live at its default level; every error is {tolerance}")
else:
    print("OK: zero await_holding_invalid_type diagnostics with the lint live at its default level; no error-level diagnostic")
PY
