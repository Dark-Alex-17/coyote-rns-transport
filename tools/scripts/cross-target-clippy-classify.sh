#!/usr/bin/env bash
set -euo pipefail

# Cross-target clippy classification for await_holding_invalid_type. Code
# behind cfg(target_os = "linux") never compiles in the macOS clippy run, so
# a guard held across an await there goes unseen. This runs clippy for
# x86_64-unknown-linux-gnu from any host with zig as the C cross-compiler
# and counts the lint's diagnostics per file.
#
# Scope: default features, every workspace member except reticulumd and
# rns-tools. Those two pull btleplug -> libdbus-sys, which needs a Linux
# pkg-config at build-script time; --all-features reaches the same crate, so
# neither is attempted here. The clippy run does not pass -D warnings: a
# pre-existing dead_code warning in
# crates/libs/rns-transport/src/iface/rnode_ble_parts/runtime_status_handle.rs
# would make cargo exit non-zero on every run and the verdict requires a
# clean exit.
#
# Passes only when cargo exits 0 and no await_holding_invalid_type diagnostic
# fires at any level. Prints SKIP: and exits 0 when zig or the target's
# rust-std is missing; a skip is not a pass, the ubuntu CI run is the
# authority for this check.
# Requires zig, the x86_64-unknown-linux-gnu rust-std and python3.

if [ $# -ne 0 ]; then
  echo "usage: $0" >&2
  exit 2
fi

TARGET="x86_64-unknown-linux-gnu"

if ! command -v zig >/dev/null 2>&1; then
  echo "SKIP: zig is not on PATH (install it from https://ziglang.org/download or 'brew install zig'); a skip is not a pass, the ubuntu CI run is the authority"
  exit 0
fi
if ! command -v rustup >/dev/null 2>&1 || ! rustup target list --installed | grep -qx "$TARGET"; then
  echo "SKIP: rust-std for $TARGET is not installed (run 'rustup target add $TARGET'); a skip is not a pass, the ubuntu CI run is the authority"
  exit 0
fi

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

wrappers="$(mktemp -d "${TMPDIR:-/tmp}/cross-clippy-zig.XXXXXX")"
json="$(mktemp "${TMPDIR:-/tmp}/cross-clippy-json.XXXXXX")"
trap 'rm -rf "$wrappers" "$json"' EXIT

# cc-rs recognises zig cc as clang and adds --target=<rust triple>, which zig
# rejects; the wrapper drops it and supplies zig's own triple instead.
cat >"$wrappers/cc" <<'SH'
#!/usr/bin/env bash
args=()
for arg in "$@"; do
  case "$arg" in
    --target=*) ;;
    *) args+=("$arg") ;;
  esac
done
exec zig cc -target x86_64-linux-gnu "${args[@]}"
SH
cat >"$wrappers/ar" <<'SH'
#!/usr/bin/env bash
exec zig ar "$@"
SH
chmod 0755 "$wrappers/cc" "$wrappers/ar"
export CC_x86_64_unknown_linux_gnu="$wrappers/cc"
export AR_x86_64_unknown_linux_gnu="$wrappers/ar"

rc=0
cargo clippy --workspace --exclude reticulumd --exclude rns-tools --all-targets --no-deps --target "$TARGET" --message-format=json >"$json" 2>/dev/null || rc=$?
echo "cargo clippy (--target $TARGET) exit: $rc"

python3 - "$json" "$rc" <<'PY'
import collections
import json
import sys

path = sys.argv[1]
clippy_rc = int(sys.argv[2])
per_file = collections.Counter()
levels = collections.Counter()
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
        if "await_holding_invalid_type" not in code:
            continue
        levels[msg.get("level")] += 1
        spans = [s for s in msg.get("spans", []) if s.get("is_primary")]
        for file_name in sorted({s.get("file_name") for s in spans}) or ["<no primary span>"]:
            per_file[file_name] += 1
hits = sum(levels.values())
print(f"await_holding_invalid_type diagnostics (any level): {hits}")
if hits:
    print(f"  by level: {dict(levels)}")
    for file_name, count in sorted(per_file.items()):
        print(f"  {count:4d}  {file_name}")
    print("FAIL: await_holding_invalid_type fired on the Linux target outside the per-target allows")
    sys.exit(1)
if clippy_rc != 0:
    print(f"FAIL: cargo clippy exited {clippy_rc}; the cross-target build itself failed (check zig, the wrappers and the target's rust-std)")
    sys.exit(1)
print("OK: zero await_holding_invalid_type diagnostics on x86_64-unknown-linux-gnu with the lint live at its default level")
PY
