#!/usr/bin/env bash
set -euo pipefail

# Tracked-tree twin of fork_publish_contract::old_package_name_survives_only_as_prose.
# That test walks the whole checkout directory and so trips on git-excluded
# local files. This script applies the same needles, extension filter and
# skipped doc prefixes to `git ls-files` only, which is what a clean CI
# checkout contains. Exit 0 iff no tracked scanned file carries an
# old-package-name form.

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
status=0
while IFS= read -r file; do
  case "$file" in
    crates/apps/reticulumd/tests/fork_publish_contract.rs) continue ;;
    docs/goals/*|docs/migrations/*|docs/adr/*|docs/release-notes-*) continue ;;
    Cargo.lock|*/Cargo.lock) continue ;;
    target/*|*/target/*|vendor/*|*/vendor/*|artifacts/*|*/artifacts/*|generated/*|*/generated/*) continue ;;
  esac
  case "$file" in
    *.rs|*.toml|*.yml|*.yaml|*.sh|*.py|*.md|*.txt) ;;
    *) continue ;;
  esac
  for n in "${needles[@]}"; do
    if grep -nF -- "$n" "$file" >/dev/null 2>&1; then
      grep -nF -- "$n" "$file" | sed "s|^|$file:|"
      status=1
    fi
  done
done < <(git ls-files)
if [ "$status" -eq 0 ]; then
  echo "old-name scan ok: no tracked scanned file carries an old-package-name form"
fi
exit "$status"
