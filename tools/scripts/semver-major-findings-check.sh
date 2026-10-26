#!/usr/bin/env bash
set -euo pipefail

# Records the cargo-semver-checks verdict for a coyote-rns-transport release.
# Usage:
#   semver-major-findings-check.sh <baseline-rev> <expected-major-count> [lint ...]
# Runs check-release with --release-type patch so every lint is evaluated (a
# declared 0.x minor bump would otherwise skip them all), then requires the
# summary to report exactly <expected-major-count> major findings, 0 minor,
# and every named lint to appear in the failure list. Exit 0 on match.
#
# cargo-semver-checks builds rustdoc JSON for the current and the baseline
# crate under the active target dir, which is the same target/doc the rustdoc
# gate writes. A fresh doc build there lets one side parse the other side's
# JSON and report "no semver update required" for a changed API, so the run
# gets its own target dir, semver-findings-check under the operator's
# CARGO_TARGET_DIR when set and under target otherwise.

usage() {
  echo "usage: $0 <baseline-rev> <expected-major-count> [lint ...]" >&2
}

if [ $# -lt 2 ]; then
  usage
  exit 2
fi

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

fail() {
  echo "semver findings check failed: $*" >&2
  exit 1
}

baseline="$1"
expected="$2"
[[ "$expected" =~ ^[0-9]+$ ]] || {
  echo "expected-major-count must be an integer, got '${expected}'" >&2
  usage
  exit 2
}
# Decimal, without leading zeros, so "01" is matched and echoed as 1.
expected=$((10#$expected))
shift 2

status=0
semver_target_dir="${CARGO_TARGET_DIR:-$ROOT_DIR/target}/semver-findings-check"
out="$(CARGO_TERM_COLOR=never NO_COLOR=1 CARGO_TARGET_DIR="$semver_target_dir" \
  cargo semver-checks check-release -p coyote-rns-transport \
  --baseline-rev "$baseline" --release-type patch 2>&1)" || status=$?
summary="$(printf '%s\n' "$out" | { grep -E 'Summary' || true; } | tail -1)"
echo "$summary"

if [ "$expected" -eq 0 ]; then
  [ "$status" -eq 0 ] || fail "expected no findings, cargo semver-checks exited ${status}"
  echo "semver ok: no findings against ${baseline}"
  exit 0
fi

# Anchored on the left so an expected count of 1 cannot match "11 major".
if ! printf '%s\n' "$summary" | grep -qE "(^|[^0-9])${expected} major and 0 minor"; then
  printf '%s\n' "$out" | tail -40 >&2
  fail "expected ${expected} major and 0 minor findings"
fi
for lint in "$@"; do
  [[ "$out" == *"--- failure ${lint}:"* ]] || fail "expected lint ${lint} in the findings"
  echo "found ${lint}"
done
echo "semver ok: ${expected} major and 0 minor findings against ${baseline}"
