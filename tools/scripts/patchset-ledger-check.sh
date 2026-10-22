#!/usr/bin/env bash
set -euo pipefail

# Fork patch-set ledger completeness. The current release section of
# docs/fork/PATCHSET.md must (1) contain a file table equal to
# `git diff --name-only <base>..HEAD -- . ':!plans'` less the files that
# range deletes (the table lists files in the tree; a deletion is told in
# the patch narrative) and (2) cite, in that section, by the first eight
# characters of its hash, every commit in <base>..HEAD that touches a file
# other than the ledger itself and the crate CHANGELOG.md.
#
# Explicit mode names the range and the section; <head> (default HEAD) lets
# a released section be re-checked against its own tag once the branch has
# moved past it:
#   patchset-ledger-check.sh <base-sha> <label> [<previous-version> [<head>]]
# With no arguments (the CI form) the base is the newest
# coyote-rns-transport-v* tag that is an ancestor of HEAD, the previous
# version is that tag's version, and the label is "Unreleased" when the
# ledger has such a section, else the crate version in
# crates/libs/rns-transport/Cargo.toml.

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

fail() {
  echo "patchset ledger check failed: $*" >&2
  exit 1
}

ledger="docs/fork/PATCHSET.md"
crate_manifest="crates/libs/rns-transport/Cargo.toml"
tag_prefix="coyote-rns-transport-v"
tick='`'

head="HEAD"
case $# in
  0)
    base="$(git tag --list "${tag_prefix}*" --merged HEAD --sort=-v:refname | head -1)"
    [ -n "$base" ] || fail "no ${tag_prefix}* tag is reachable from HEAD; fetch the full history and tags (fetch-depth: 0); a release tag on an unmerged branch does not count"
    previous="${base#"$tag_prefix"}"
    if grep -q '^## Unreleased (upstream' "$ledger"; then
      label="Unreleased"
    else
      label="$(sed -n 's/^version = "\([^"]*\)".*/\1/p' "$crate_manifest" | head -1)"
      [ -n "$label" ] || fail "no version line in ${crate_manifest}"
    fi
    ;;
  2 | 3 | 4)
    base="$1"
    label="$2"
    previous="${3:-}"
    head="${4:-HEAD}"
    ;;
  *)
    echo "usage: $0 [<base-sha> <label> [<previous-version> [<head>]]]" >&2
    exit 2
    ;;
esac

changed="$(git diff --name-only --diff-filter=d "${base}..${head}" -- . ':!plans' | sort)"

if [ "$label" = "$previous" ]; then
  if [ -z "$changed" ]; then
    echo "patchset ledger ok: no files changed since ${base}"
    exit 0
  fi
  fail "no ledger section for the commits since ${base}; add an Unreleased section"
fi

grep -q "^## ${label} (upstream" "$ledger" || fail "no '## ${label} (upstream ...)' section in ${ledger}"

# The section runs to the previous release's heading when one is named,
# otherwise to whatever heading comes next.
if [ -n "$previous" ]; then
  section_end="^## ${previous} \\\\(upstream"
else
  section_end='^## '
fi
section="$(awk -v start="^## ${label} \\\\(upstream" -v end="$section_end" '
  $0 ~ start {f=1; next}
  f && $0 ~ end {f=0}
  f' "$ledger")"
rows="$(printf '%s\n' "$section" | { grep -oE "^\| ${tick}[^${tick}]+${tick}" || true; } | sed "s/^| ${tick}//; s/${tick}\$//" | sort)"

if ! diff <(printf '%s\n' "$changed") <(printf '%s\n' "$rows"); then
  fail "the ${label} table in ${ledger} differs from git diff --name-only ${base}..${head} (left: git, right: table)"
fi

# The ledger always writes eight-character hashes; %h would vary with the
# clone's abbreviation width.
missing=0
while read -r hash; do
  short="${hash:0:8}"
  if ! printf '%s\n' "$section" | grep -q "${tick}${short}${tick}"; then
    echo "uncited commit ${short}" >&2
    missing=1
  fi
done < <(git log --no-merges --format=%H "${base}..${head}" -- . ":!${ledger}" ':!crates/libs/rns-transport/CHANGELOG.md' ':!plans')
if [ "$missing" -ne 0 ]; then
  fail "the ${label} section of ${ledger} does not cite every non-ledger commit in ${base}..${head}"
fi

row_count="$(printf '%s\n' "$rows" | grep -c . || true)"
echo "patchset ledger ok: ${label} table has ${row_count} rows matching the files changed since ${base}; every non-ledger commit is cited"
