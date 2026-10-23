#!/usr/bin/env bash
set -euo pipefail

# Python-in-the-loop interop classification. Runs the ignored
# python_channel_interop batch against a Python Reticulum checkout, keeps the
# nextest output in a temp file, reads the failed test names from its
# `FAIL [` lines and passes only when every failure is in the tolerated set
# below and at least one test passed. Anything else fails naming the
# untolerated tests.
#
# Tolerated failures are environment reds, not product defects: on macOS the
# backbone and serial-pty cases listed below fail against the Python side on
# Darwin and are recorded as such; on other OSes nothing is tolerated. The
# pinned-resource case asserts that the checkout is at the exact revision it
# pins, so it is tolerated only when the checkout is somewhere else.
# Requires RETICULUM_PY_REPO (a Python Reticulum checkout); honours
# LXMF_PYTHON_BIN and PYTHON_RNS_PATH as the tests do. Needs cargo-nextest
# and python3. Takes about ten minutes.

usage() {
  echo "usage: RETICULUM_PY_REPO=<path> [LXMF_PYTHON_BIN=<python3>] [PYTHON_RNS_PATH=<path>] $0" >&2
  exit 2
}

if [ $# -ne 0 ] || [ -z "${RETICULUM_PY_REPO:-}" ]; then
  usage
fi

# Edit here when the environment changes.
DARWIN_ENVIRONMENT_FAILURES=(
  python_to_rust_backbone_channel_roundtrip
  python_to_rust_backbone_link_data_roundtrip
  python_to_rust_backbone_request_response_roundtrip
  python_to_rust_backbone_resource_backed_request_response_roundtrip
  rust_to_python_backbone_channel_roundtrip
  rust_to_python_backbone_link_data_roundtrip
  rust_to_python_backbone_raw_resource_roundtrip
  rust_to_python_backbone_request_response_roundtrip
  rust_to_python_channel_buffer_roundtrip
  python_rust_ax25_kiss_serial_channel_roundtrip_over_pty
  python_rust_ifac_kiss_serial_channel_roundtrip_over_pty
)
PIN_DRIFT_FAILURE=pinned_python_resource_cancels_after_missing_part_retry_budget
PIN_SOURCE="crates/apps/reticulumd/tests/python_channel_interop_parts/resource_timeout_reference.rs"

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

tolerated=()
if [ "$(uname -s)" = "Darwin" ]; then
  tolerated=("${DARWIN_ENVIRONMENT_FAILURES[@]}")
fi

pinned="$(sed -n 's/.*PINNED_RETICULUM: &str = "\([0-9a-f]*\)".*/\1/p' "$PIN_SOURCE" | head -n 1)"
if [ -z "$pinned" ]; then
  echo "FAIL: no PINNED_RETICULUM constant in $PIN_SOURCE"
  exit 1
fi
checkout="$(git -C "$RETICULUM_PY_REPO" rev-parse HEAD)"
echo "python reticulum checkout: $checkout (pinned: $pinned)"
if [ "$checkout" != "$pinned" ]; then
  echo "checkout differs from the pin; tolerating $PIN_DRIFT_FAILURE"
  tolerated+=("$PIN_DRIFT_FAILURE")
fi
echo "tolerated failures ($(uname -s)): ${tolerated[*]:-none}"

log="$(mktemp "${TMPDIR:-/tmp}/python-interop.XXXXXX")"
trap 'rm -f "$log"' EXIT

rc=0
cargo nextest run -j 1 --no-fail-fast --color never -p reticulumd --test python_channel_interop --run-ignored only --no-capture 2>&1 | tee "$log" || rc=$?
echo "cargo nextest (python_channel_interop batch) exit: $rc"

# bash before 4.4 treats an empty array as unbound under set -u.
python3 - "$log" "$rc" ${tolerated[@]+"${tolerated[@]}"} <<'PY'
import re
import sys

path = sys.argv[1]
nextest_rc = int(sys.argv[2])
tolerated = set(sys.argv[3:])
status = re.compile(r"^\s*(PASS|FAIL) \[")
passed = 0
failed = set()
with open(path, errors="replace") as fh:
    for line in fh:
        m = status.match(line)
        if not m:
            continue
        name = line.split()[-1]
        if m.group(1) == "PASS":
            passed += 1
        else:
            failed.add(name)
print(f"passed: {passed}, failed: {len(failed)}")
untolerated = sorted(failed - tolerated)
if untolerated:
    print(f"FAIL: {len(untolerated)} failure(s) outside the tolerated set:")
    for name in untolerated:
        print(f"  {name}")
    sys.exit(1)
if nextest_rc != 0 and not failed:
    print(f"FAIL: cargo nextest exited {nextest_rc} without a FAIL line; the batch itself did not run")
    sys.exit(1)
if passed == 0:
    print("FAIL: no test passed")
    sys.exit(1)
print(f"OK: {passed} passed; every failure is tolerated: {', '.join(sorted(failed)) or 'none'}")
PY
