#!/usr/bin/env bash
set -euo pipefail

# Records the cargo-semver-checks verdict for a coyote-rns-transport release.
# Usage:
#   semver-major-findings-check.sh <baseline-rev> <expected-major-count> [lint ...]
# Runs check-release with --release-type patch so every lint is evaluated (a
# declared 0.x minor bump would otherwise skip them all), then requires the
# summary to report exactly <expected-major-count> major findings, 0 minor,
# and every named lint to appear in the failure list. Exit 0 on match.

if [ $# -lt 2 ]; then
  echo "usage: $0 <baseline-rev> <expected-major-count> [lint ...]" >&2
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
shift 2

status=0
out="$(cargo semver-checks check-release -p coyote-rns-transport \
  --baseline-rev "$baseline" --release-type patch 2>&1)" || status=$?
summary="$(printf '%s\n' "$out" | sed 's/\x1b\[[0-9;]*m//g' | { grep -E 'Summary' || true; } | tail -1)"
echo "$summary"

if [ "$expected" -eq 0 ]; then
  [ "$status" -eq 0 ] || fail "expected no findings, cargo semver-checks exited ${status}"
  echo "semver ok: no findings against ${baseline}"
  exit 0
fi

if ! printf '%s\n' "$summary" | grep -qE "${expected} major and 0 minor"; then
  printf '%s\n' "$out" | tail -40 >&2
  fail "expected ${expected} major and 0 minor findings"
fi
for lint in "$@"; do
  printf '%s\n' "$out" | grep -q -- "--- failure ${lint}:" || fail "expected lint ${lint} in the findings"
  echo "found ${lint}"
done
echo "semver ok: ${expected} major and 0 minor findings against ${baseline}"
