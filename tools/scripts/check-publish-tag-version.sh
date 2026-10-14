#!/usr/bin/env bash
set -euo pipefail

# Publish guard: when PUBLISH_REF is a coyote-rns-transport-v* release tag,
# the version it names must equal the version in crates/libs/rns-transport/Cargo.toml.
# Any other ref passes through unchecked.

if [ -z "${PUBLISH_REF:-}" ]; then
  echo "usage: PUBLISH_REF=<git ref or release tag> $0" >&2
  exit 2
fi

tag_prefix="coyote-rns-transport-v"
if [[ "$PUBLISH_REF" != "$tag_prefix"* ]]; then
  echo "Ref ${PUBLISH_REF} is not a ${tag_prefix}* release tag; skipping the version check and publishing the version it carries."
  exit 0
fi

crate_version="$(cargo pkgid --locked -p coyote-rns-transport | sed 's/.*[@#]//')"
tag_version="${PUBLISH_REF#"$tag_prefix"}"

if [ "$crate_version" != "$tag_version" ]; then
  echo "::error::Tag ${PUBLISH_REF} names version ${tag_version} but crates/libs/rns-transport/Cargo.toml has version ${crate_version}"
  exit 1
fi
