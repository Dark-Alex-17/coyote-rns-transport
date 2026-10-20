#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

# libFuzzer needs a nightly toolchain; the campaign runs every target on it.
TOOLCHAIN="${FUZZ_TOOLCHAIN:-nightly}"
MAX_TOTAL_TIME="${FUZZ_MAX_TOTAL_TIME:-300}"

if ! cargo +"$TOOLCHAIN" fuzz --help >/dev/null 2>&1; then
  echo "cargo-fuzz on the ${TOOLCHAIN} toolchain is required. Install with:" >&2
  echo "  rustup toolchain install ${TOOLCHAIN} && cargo install cargo-fuzz" >&2
  exit 1
fi

run_target() {
  local fuzz_dir="$1"
  local target="$2"
  echo "Running ${fuzz_dir} target ${target} for ${MAX_TOTAL_TIME}s"
  cargo +"$TOOLCHAIN" fuzz run \
    --fuzz-dir "$fuzz_dir" \
    "$target" \
    -- -max_total_time="$MAX_TOTAL_TIME"
}

run_target crates/libs/rns-rpc/fuzz rpc_frame_envelope
run_target crates/libs/lxmf-sdk/fuzz sdk_json_envelope
run_target crates/libs/rns-transport/fuzz request_frame_decode
run_target crates/libs/rns-transport/fuzz response_frame_decode
run_target crates/libs/rns-transport/fuzz resource_advertisement_decode

artifacts="$(find crates/libs/*/fuzz/artifacts -type f 2>/dev/null || true)"
if [[ -n "$artifacts" ]]; then
  echo "Fuzz campaign found crash artifacts:" >&2
  echo "$artifacts" >&2
  exit 1
fi

echo "Fuzz campaign completed: no crash artifacts"
