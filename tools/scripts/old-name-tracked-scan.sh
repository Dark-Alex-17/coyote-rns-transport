#!/usr/bin/env bash
set -euo pipefail

# Tracked-files-only form of
# fork_publish_contract::old_package_name_survives_only_as_prose. That test
# reads `git ls-files --cached --others --exclude-standard` (tracked plus
# committable untracked files) and skips dot-directories other than .github.
# This script applies the same needles, extension filter and skipped doc
# prefixes to `git ls-files` only, with no dot-directory skip. Exit 0 iff no
# tracked scanned file carries an old-package-name form.

if [ $# -ne 0 ]; then
  echo "usage: $0" >&2
  exit 2
fi

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

# Assembled from the bare name so this file carries no runnable form itself.
old='reticulum-rs-transport'
needles=(
  "-p ${old}"
  "--package ${old}"
  "\"-p\", \"${old}\""
  "\"${old}\""
  "package = \"${old}\""
  "package: \"${old}\""
  "name = \"${old}\""
  "${old} = \""
  "crates.io/crates/${old}"
)

needle_file="$(mktemp "${TMPDIR:-/tmp}/old-name-needles.XXXXXX")"
trap 'rm -f "$needle_file"' EXIT
printf '%s\n' "${needles[@]}" >"$needle_file"

files=()
while IFS= read -r file; do
  case "$file" in
    crates/apps/reticulumd/tests/fork_publish_contract.rs) continue ;;
    docs/goals/*|docs/migrations/*|docs/adr/*|docs/release-notes-*) continue ;;
    Cargo.lock|*/Cargo.lock) continue ;;
    target/*|*/target/*|vendor/*|*/vendor/*|artifacts/*|*/artifacts/*|generated/*|*/generated/*) continue ;;
  esac
  case "$file" in
    *.rs|*.toml|*.yml|*.yaml|*.sh|*.py|*.md|*.txt) files+=("$file") ;;
  esac
done < <(git ls-files)

# One grep over the whole list; its exit 1 (no match) is the pass case.
status=0
grep -nHF -f "$needle_file" -- "${files[@]}" || status=$?
case "$status" in
  0) exit 1 ;;
  1)
    echo "old-name scan ok: no tracked scanned file carries an old-package-name form"
    exit 0
    ;;
  *) exit "$status" ;;
esac
