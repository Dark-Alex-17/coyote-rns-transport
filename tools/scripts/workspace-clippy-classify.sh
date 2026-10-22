#!/usr/bin/env bash
set -euo pipefail

# CI-equivalent clippy classification. Runs the workspace clippy invocation
# CI uses (all targets, all features, -D warnings) with JSON output and
# classifies every diagnostic. Passes only when no await_holding_invalid_type
# diagnostic fires at any level and every error-level diagnostic is the
# macOS-only dead_code in the linux-only reticulumd python_channel_interop
# test target (the two files listed below), which CI on Linux never sees.
# Requires python3 for the classification pass.

if [ $# -ne 0 ]; then
  echo "usage: $0" >&2
  exit 2
fi

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

json="$(mktemp "${TMPDIR:-/tmp}/clippy-json.XXXXXX")"
trap 'rm -f "$json"' EXIT

rc=0
cargo clippy --workspace --all-targets --all-features --no-deps --message-format=json -- -D warnings >"$json" 2>/dev/null || rc=$?
echo "cargo clippy (CI invocation) exit: $rc"

python3 - "$json" "$rc" <<'PY'
import collections
import json
import sys

path = sys.argv[1]
clippy_rc = int(sys.argv[2])
allowed_files = {
    "crates/apps/reticulumd/tests/python_channel_interop_parts/resource_memory_profile.rs",
    "crates/apps/reticulumd/tests/support/python_channel_process.rs",
}
await_hits = 0
errors = []
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
        if level == "error" and code:
            spans = [s for s in msg.get("spans", []) if s.get("is_primary")]
            files = {s.get("file_name") for s in spans}
            codes[code] += 1
            errors.append((code, files))
print(f"await_holding_invalid_type diagnostics (any level): {await_hits}")
print(f"error-level diagnostics by code: {dict(codes)}")
bad = [e for e in errors if e[0] != "dead_code" or not e[1] or not e[1] <= allowed_files]
for code, files in errors:
    print(f"  error {code}: {sorted(files)}")
if await_hits:
    print("FAIL: await_holding_invalid_type fired despite the lint tables")
    sys.exit(1)
if clippy_rc != 0 and not errors:
    print(f"FAIL: cargo clippy exited {clippy_rc} without any error-level diagnostic; the invocation itself failed")
    sys.exit(1)
if bad:
    print(f"FAIL: {len(bad)} error diagnostic(s) outside the known macOS-only dead_code in the linux-only reticulumd test target")
    sys.exit(1)
print("OK: zero await_holding_invalid_type diagnostics; every error is the macOS-only dead_code in the linux-only reticulumd test target")
PY
