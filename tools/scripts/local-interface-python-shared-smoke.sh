#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

case "$(uname -s)" in
  Linux|Android) ;;
  *)
    echo "[local-interface-python-shared-smoke] ERROR: Python abstract Unix shared instance evidence requires Linux/Android" >&2
    exit 1
    ;;
esac

RETICULUM_PY_REPO="${RETICULUM_PY_REPO:-${ROOT_DIR}/.tmp/python-refs/Reticulum}"
PYTHON_BIN="${LXMF_PYTHON_BIN:-${PYTHON_BIN:-python3}}"
TIMEOUT_SECS="${TIMEOUT_SECS:-45}"
LOG_DIR="${LOG_DIR:-${ROOT_DIR}/target/local-interface-python-shared-smoke}"
REPORT_PATH="${REPORT_PATH:-${LOG_DIR}/report.json}"
mkdir -p "$LOG_DIR"

# Boundary phrase preserved for contract tests: does not prove broad application-level shared-instance traffic parity.
RUN_DIR="$(mktemp -d "${LOG_DIR}/run.XXXXXX")"
# A daemon attaches exactly one shared instance, so each Python shared instance gets its own
# reticulumd. Each daemon also gets its own directory: reticulumd keeps its packet hashlist,
# path table and service identities beside the database it is given.
RET_DIR_TCP="${RUN_DIR}/reticulumd-tcp"
RET_DIR_UNIX="${RUN_DIR}/reticulumd-unix"
RUST_CONFIG_PATH_TCP="${RET_DIR_TCP}/reticulumd-local-python-shared.toml"
RUST_CONFIG_PATH_UNIX="${RET_DIR_UNIX}/reticulumd-local-python-shared.toml"
DB_PATH_TCP="${RET_DIR_TCP}/reticulum.db"
DB_PATH_UNIX="${RET_DIR_UNIX}/reticulum.db"
RETICULUMD_LOG_TCP="${RET_DIR_TCP}/reticulumd.log"
RETICULUMD_LOG_UNIX="${RET_DIR_UNIX}/reticulumd.log"
RNSTATUS_JSON_TCP="${RET_DIR_TCP}/rnstatus.json"
RNSTATUS_JSON_UNIX="${RET_DIR_UNIX}/rnstatus.json"
RNSTATUS_HUMAN_TCP="${RET_DIR_TCP}/rnstatus.txt"
RNSTATUS_HUMAN_UNIX="${RET_DIR_UNIX}/rnstatus.txt"
PY_TCP_CONFIG_DIR="${RUN_DIR}/python-tcp"
PY_UNIX_CONFIG_DIR="${RUN_DIR}/python-unix"
PY_TCP_TRAFFIC_CONFIG_DIR="${RUN_DIR}/python-tcp-traffic"
PY_UNIX_TRAFFIC_CONFIG_DIR="${RUN_DIR}/python-unix-traffic"
PY_TCP_LOG="${RUN_DIR}/python-shared-tcp.log"
PY_UNIX_LOG="${RUN_DIR}/python-shared-unix.log"
PY_TCP_TRAFFIC_LOG="${RUN_DIR}/python-traffic-tcp.log"
PY_UNIX_TRAFFIC_LOG="${RUN_DIR}/python-traffic-unix.log"
PY_TCP_STATE="${RUN_DIR}/python-shared-tcp-state.json"
PY_UNIX_STATE="${RUN_DIR}/python-shared-unix-state.json"
PY_TCP_TRAFFIC_STATE="${RUN_DIR}/python-traffic-tcp-state.json"
PY_UNIX_TRAFFIC_STATE="${RUN_DIR}/python-traffic-unix-state.json"
PY_TCP_PORT_FILE="${RUN_DIR}/python-shared-tcp.port"
PY_UNIX_INSTANCE="codex-py-shared-$$"
UNIX_TEARDOWN_RESTART_VERIFIED=false
TCP_TEARDOWN_RESTART_VERIFIED=false
mkdir -p "$RET_DIR_TCP" "$RET_DIR_UNIX" "$PY_TCP_CONFIG_DIR" "$PY_UNIX_CONFIG_DIR" "$PY_TCP_TRAFFIC_CONFIG_DIR" "$PY_UNIX_TRAFFIC_CONFIG_DIR"
: >"$RETICULUMD_LOG_TCP"
: >"$RETICULUMD_LOG_UNIX"
: >"$PY_TCP_LOG"
: >"$PY_UNIX_LOG"
: >"$PY_TCP_TRAFFIC_LOG"
: >"$PY_UNIX_TRAFFIC_LOG"

if [[ ! -f "${RETICULUM_PY_REPO}/RNS/Reticulum.py" ]]; then
  echo "[local-interface-python-shared-smoke] ERROR: RETICULUM_PY_REPO does not point to a Reticulum checkout: ${RETICULUM_PY_REPO}" >&2
  exit 1
fi

# Three free loopback ports, held open until all are chosen so no two picks collide.
read -r RPC_PORT_TCP RPC_PORT_UNIX PY_TCP_PORT < <(
  python3 - <<'PY'
import socket
socks = [socket.socket(socket.AF_INET, socket.SOCK_STREAM) for _ in range(3)]
for sock in socks:
    sock.bind(("127.0.0.1", 0))
print(" ".join(str(sock.getsockname()[1]) for sock in socks))
for sock in socks:
    sock.close()
PY
)
RPC_ADDR_TCP="127.0.0.1:${RPC_PORT_TCP}"
RPC_ADDR_UNIX="127.0.0.1:${RPC_PORT_UNIX}"
printf '%s\n' "$PY_TCP_PORT" >"$PY_TCP_PORT_FILE"

cat >"${PY_TCP_CONFIG_DIR}/config" <<EOF
[reticulum]
share_instance = yes
shared_instance_type = tcp
shared_instance_port = ${PY_TCP_PORT}
force_shared_instance_bitrate = 1000000
enable_transport = no

[logging]
loglevel = 7
EOF

cat >"${PY_UNIX_CONFIG_DIR}/config" <<EOF
[reticulum]
share_instance = yes
shared_instance_type = unix
instance_name = ${PY_UNIX_INSTANCE}
force_shared_instance_bitrate = 1000000
enable_transport = no

[logging]
loglevel = 7
EOF

cat >"${PY_TCP_TRAFFIC_CONFIG_DIR}/config" <<EOF
[reticulum]
share_instance = yes
shared_instance_type = tcp
shared_instance_port = ${PY_TCP_PORT}
force_shared_instance_bitrate = 1000000
enable_transport = no

[logging]
loglevel = 7
EOF

cat >"${PY_UNIX_TRAFFIC_CONFIG_DIR}/config" <<EOF
[reticulum]
share_instance = yes
shared_instance_type = unix
instance_name = ${PY_UNIX_INSTANCE}
force_shared_instance_bitrate = 1000000
enable_transport = no

[logging]
loglevel = 7
EOF

write_report() {
  local status="$1"
  local reason="${2:-}"
  python3 - <<'PY' "$REPORT_PATH" "$status" "$reason" "$RUN_DIR" "$RPC_ADDR_TCP" "$RPC_UNIX_TCP" "$DB_PATH_TCP" "$RUST_CONFIG_PATH_TCP" "$RETICULUMD_LOG_TCP" "$RNSTATUS_JSON_TCP" "$RNSTATUS_HUMAN_TCP" "$RPC_ADDR_UNIX" "$RPC_UNIX_UNIX" "$DB_PATH_UNIX" "$RUST_CONFIG_PATH_UNIX" "$RETICULUMD_LOG_UNIX" "$RNSTATUS_JSON_UNIX" "$RNSTATUS_HUMAN_UNIX" "$RETICULUM_PY_REPO" "$PY_TCP_CONFIG_DIR" "$PY_UNIX_CONFIG_DIR" "$PY_TCP_TRAFFIC_CONFIG_DIR" "$PY_UNIX_TRAFFIC_CONFIG_DIR" "$PY_TCP_LOG" "$PY_UNIX_LOG" "$PY_TCP_TRAFFIC_LOG" "$PY_UNIX_TRAFFIC_LOG" "$PY_TCP_STATE" "$PY_UNIX_STATE" "$PY_TCP_TRAFFIC_STATE" "$PY_UNIX_TRAFFIC_STATE" "$PY_TCP_PORT" "$PY_UNIX_INSTANCE" "$UNIX_TEARDOWN_RESTART_VERIFIED" "$TCP_TEARDOWN_RESTART_VERIFIED"
import json
import pathlib
import subprocess
import sys

(
    report_path,
    status,
    reason,
    run_dir,
    tcp_rpc_addr,
    tcp_rpc_unix,
    tcp_db_path,
    tcp_rust_config_path,
    tcp_reticulumd_log,
    tcp_rnstatus_json,
    tcp_rnstatus_human,
    unix_rpc_addr,
    unix_rpc_unix,
    unix_db_path,
    unix_rust_config_path,
    unix_reticulumd_log,
    unix_rnstatus_json,
    unix_rnstatus_human,
    reticulum_py_repo,
    py_tcp_config_dir,
    py_unix_config_dir,
    py_tcp_traffic_config_dir,
    py_unix_traffic_config_dir,
    py_tcp_log,
    py_unix_log,
    py_tcp_traffic_log,
    py_unix_traffic_log,
    py_tcp_state,
    py_unix_state,
    py_tcp_traffic_state,
    py_unix_traffic_state,
    py_tcp_port,
    py_unix_instance,
    unix_teardown_restart_verified,
    tcp_teardown_restart_verified,
) = sys.argv[1:36]

def read_json(path):
    value_path = pathlib.Path(path)
    if not value_path.exists():
        return None
    try:
        return json.loads(value_path.read_text(encoding="utf-8"))
    except Exception as exc:
        return {"parse_error": str(exc)}

def read_interfaces(rnstatus_json_by_name):
    rows = {}
    for expected, path in rnstatus_json_by_name.items():
        payload = read_json(path)
        if not isinstance(payload, dict):
            continue
        row = next(
            (
                item
                for item in payload.get("interfaces", [])
                if item.get("name") == expected
            ),
            None,
        )
        if row:
            runtime = (row.get("settings") or {}).get("_runtime") or {}
            rows[expected] = {
                "type": row.get("type"),
                "enabled": row.get("enabled"),
                "socket_path": (row.get("settings") or {}).get("socket_path"),
                "host": row.get("host"),
                "port": row.get("port"),
                "shared_instance_type": (row.get("settings") or {}).get("shared_instance_type"),
                "startup_status": runtime.get("startup_status"),
                "runtime_iface": runtime.get("iface") or runtime.get("runtime_iface"),
            }
    return rows

def daemon_report(rpc_addr, rpc_unix, db_path, rust_config_path, reticulumd_log, rnstatus_json, rnstatus_human):
    entry = {
        "rpc_addr": rpc_addr,
        "rpc_unix": rpc_unix,
        "db_path": db_path,
        "rust_config_path": rust_config_path,
        "reticulumd_log": reticulumd_log,
        "rnstatus_json": rnstatus_json,
        "rnstatus_human": rnstatus_human,
    }
    human_path = pathlib.Path(rnstatus_human)
    if human_path.exists():
        entry["human_summary"] = human_path.read_text(encoding="utf-8", errors="replace")
    return entry

try:
    py_revision = subprocess.check_output(
        ["git", "-C", reticulum_py_repo, "rev-parse", "HEAD"],
        text=True,
        stderr=subprocess.DEVNULL,
    ).strip()
except Exception:
    py_revision = None

report = {
    "status": status,
    "evidence_scope": "python_shared_instance_tcp_unix_attach_announce_payload_and_lifecycle",
    "product_boundary": (
        "This proves two reticulumd daemons, one per pinned Python Reticulum shared instance "
        "(a daemon attaches exactly one shared instance, as a Python process holds one), "
        "attach a LocalClientInterface to real pinned Python Reticulum shared instances "
        "over TCP and Linux abstract Unix sockets, and that "
        "Python-origin announces move across the shared "
        "instance fanout toward attached local clients with exact payloads "
        "observed between Python peers. Both transports observe graceful "
        "reticulumd client teardown and reattachment after daemon restart; "
        "this does not prove daemon application-level packet consumption or broad "
        "shared-instance "
        "traffic parity."
    ),
    "reason": reason or None,
    "run_dir": run_dir,
    "daemons": {
        "tcp": daemon_report(
            tcp_rpc_addr,
            tcp_rpc_unix,
            tcp_db_path,
            tcp_rust_config_path,
            tcp_reticulumd_log,
            tcp_rnstatus_json,
            tcp_rnstatus_human,
        ),
        "unix": daemon_report(
            unix_rpc_addr,
            unix_rpc_unix,
            unix_db_path,
            unix_rust_config_path,
            unix_reticulumd_log,
            unix_rnstatus_json,
            unix_rnstatus_human,
        ),
    },
    "reticulum_py_repo": reticulum_py_repo,
    "python_rns_revision": py_revision,
    "python_tcp_config_dir": py_tcp_config_dir,
    "python_unix_config_dir": py_unix_config_dir,
    "python_tcp_traffic_config_dir": py_tcp_traffic_config_dir,
    "python_unix_traffic_config_dir": py_unix_traffic_config_dir,
    "python_tcp_log": py_tcp_log,
    "python_unix_log": py_unix_log,
    "python_tcp_traffic_log": py_tcp_traffic_log,
    "python_unix_traffic_log": py_unix_traffic_log,
    "python_tcp_state": read_json(py_tcp_state),
    "python_unix_state": read_json(py_unix_state),
    "python_tcp_traffic_state": read_json(py_tcp_traffic_state),
    "python_unix_traffic_state": read_json(py_unix_traffic_state),
    "python_tcp_port": int(py_tcp_port),
    "python_unix_socket_path": f"@rns/{py_unix_instance}",
    "unix_teardown_restart_verified": unix_teardown_restart_verified == "true",
    "tcp_teardown_restart_verified": tcp_teardown_restart_verified == "true",
    "interfaces": read_interfaces(
        {
            "local-python-tcp-attach": tcp_rnstatus_json,
            "local-python-unix-attach": unix_rnstatus_json,
        }
    ),
}
pathlib.Path(report_path).write_text(
    json.dumps(report, indent=2, sort_keys=True) + "\n",
    encoding="utf-8",
)
PY
}

cleanup() {
  local status=$?
  local pid
  for pid in "${RET_PID_TCP:-}" "${RET_PID_UNIX:-}" "${PY_TCP_PID:-}" "${PY_UNIX_PID:-}" "${PY_TCP_TRAFFIC_PID:-}" "${PY_UNIX_TRAFFIC_PID:-}"; do
    [[ -n "$pid" ]] || continue
    kill "$pid" >/dev/null 2>&1 || true
    wait "$pid" >/dev/null 2>&1 || true
  done
  if [[ -n "${RPC_UNIX_DIR:-}" ]]; then
    rm -rf "$RPC_UNIX_DIR"
  fi
  if [[ $status -ne 0 ]]; then
    echo "[local-interface-python-shared-smoke] failed; logs=${RUN_DIR}" >&2
  fi
}
trap cleanup EXIT

# The RPC unix sockets live in a short /tmp dir: the daemon binds each one through a
# staging path beside it, and a socket path longer than sun_path (107 bytes) fails to bind.
RPC_UNIX_DIR="$(mktemp -d /tmp/rns-shared-smoke-rpc.XXXXXX)"
RPC_UNIX_TCP="${RPC_UNIX_DIR}/tcp.sock"
RPC_UNIX_UNIX="${RPC_UNIX_DIR}/unix.sock"

fail() {
  local msg="$1"
  echo "[local-interface-python-shared-smoke] ERROR: ${msg}" | tee -a "$RETICULUMD_LOG_TCP" "$RETICULUMD_LOG_UNIX" >&2
  write_report "fail" "$msg"
  exit 1
}

start_python_shared() {
  local config_dir="$1"
  local state_path="$2"
  local log_path="$3"
  PYTHONPATH="$RETICULUM_PY_REPO${PYTHONPATH:+:${PYTHONPATH}}" "$PYTHON_BIN" - "$config_dir" "$state_path" >"$log_path" 2>&1 <<'PY' &
import json
import pathlib
import sys
import time

config_dir, state_path = sys.argv[1:3]

def write_state(payload):
    pathlib.Path(state_path).write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

try:
    import RNS
    reticulum = RNS.Reticulum(configdir=config_dir, loglevel=7)
    class TrafficHandler:
        aspect_filter = "codex.local.shared.traffic"

        def received_announce(self, destination_hash, announced_identity, app_data):
            if app_data is not None and app_data.startswith(b"codex-local-python-shared-payload:"):
                received_payloads.append(app_data.decode("utf-8"))

    received_payloads = []
    RNS.Transport.register_announce_handler(TrafficHandler())
    identity = RNS.Identity()
    destination = RNS.Destination(
        identity,
        RNS.Destination.IN,
        RNS.Destination.SINGLE,
        "codex",
        "local",
        "shared",
        "traffic",
    )
    reverse_payload = b"codex-local-python-shared-payload:shared-to-python"
    last_announce = 0.0
    while True:
        local_clients = getattr(RNS.Transport, "local_client_interfaces", [])
        local_client_stats = []
        for item in local_clients:
            local_client_stats.append(
                {
                    "name": str(item),
                    "online": getattr(item, "online", None),
                    "rxb": getattr(item, "rxb", None),
                    "txb": getattr(item, "txb", None),
                    "is_connected_to_shared_instance": getattr(item, "is_connected_to_shared_instance", None),
                }
            )
        shared = reticulum.shared_instance_interface
        if time.monotonic() - last_announce >= 1.0:
            destination.announce(app_data=reverse_payload)
            last_announce = time.monotonic()
        write_state(
            {
                "ready": True,
                "is_shared_instance": reticulum.is_shared_instance,
                "is_connected_to_shared_instance": reticulum.is_connected_to_shared_instance,
                "shared_instance_type": reticulum.shared_instance_type,
                "use_af_unix": reticulum.use_af_unix,
                "local_interface_port": reticulum.local_interface_port,
                "local_socket_path": reticulum.local_socket_path,
                "shared_interface": str(shared) if shared is not None else None,
                "local_client_count": len(local_clients),
                "local_clients": [str(item) for item in local_clients],
                "local_client_stats": local_client_stats,
                "local_client_rxb_total": sum((getattr(item, "rxb", 0) or 0) for item in local_clients),
                "local_client_txb_total": sum((getattr(item, "txb", 0) or 0) for item in local_clients),
                "received_payloads": received_payloads,
                "reverse_payload": reverse_payload.decode("utf-8"),
            }
        )
        time.sleep(0.25)
except BaseException as exc:
    write_state({"ready": False, "error": repr(exc)})
    raise
PY
  echo $!
}

start_python_traffic() {
  local config_dir="$1"
  local state_path="$2"
  local log_path="$3"
  local label="$4"
  PYTHONPATH="$RETICULUM_PY_REPO${PYTHONPATH:+:${PYTHONPATH}}" "$PYTHON_BIN" - "$config_dir" "$state_path" "$label" >"$log_path" 2>&1 <<'PY' &
import json
import pathlib
import sys
import time

config_dir, state_path, label = sys.argv[1:4]

def write_state(payload):
    pathlib.Path(state_path).write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

try:
    import RNS
    reticulum = RNS.Reticulum(configdir=config_dir, loglevel=7, require_shared_instance=True)
    received_payloads = []
    class TrafficHandler:
        aspect_filter = "codex.local.shared.traffic"

        def received_announce(self, destination_hash, announced_identity, app_data):
            if app_data is not None and app_data.startswith(b"codex-local-python-shared-payload:"):
                received_payloads.append(app_data.decode("utf-8"))

    RNS.Transport.register_announce_handler(TrafficHandler())
    identity = RNS.Identity()
    destination = RNS.Destination(
        identity,
        RNS.Destination.IN,
        RNS.Destination.SINGLE,
        "codex",
        "local",
        "shared",
        "traffic",
    )
    app_data = f"codex-local-python-shared-payload:python-to-shared:{label}".encode("utf-8")
    reverse_app_data = f"codex-local-python-shared-payload:shared-to-python:{label}".encode("utf-8")
    announced = 0
    reverse_announced = 0
    for _ in range(3):
        destination.announce(app_data=app_data)
        announced += 1
        destination.announce(app_data=reverse_app_data)
        reverse_announced += 1
        write_state(
            {
                "ready": True,
                "label": label,
                "is_connected_to_shared_instance": reticulum.is_connected_to_shared_instance,
                "announced_count": announced,
                "reverse_announced_count": reverse_announced,
                "received_payloads": received_payloads,
                "destination_hash": RNS.hexrep(destination.hash, delimit=False),
            }
        )
        time.sleep(0.5)
    while True:
        write_state(
            {
                "ready": True,
                "label": label,
                "is_connected_to_shared_instance": reticulum.is_connected_to_shared_instance,
                "announced_count": announced,
                "reverse_announced_count": reverse_announced,
                "received_payloads": received_payloads,
                "destination_hash": RNS.hexrep(destination.hash, delimit=False),
            }
        )
        time.sleep(0.5)
except BaseException as exc:
    write_state({"ready": False, "label": label, "error": repr(exc)})
    raise
PY
  echo $!
}

PY_TCP_PID="$(start_python_shared "$PY_TCP_CONFIG_DIR" "$PY_TCP_STATE" "$PY_TCP_LOG")"
PY_UNIX_PID="$(start_python_shared "$PY_UNIX_CONFIG_DIR" "$PY_UNIX_STATE" "$PY_UNIX_LOG")"

deadline=$((SECONDS + TIMEOUT_SECS))
wait_for_python_ready() {
  local state_path="$1"
  local pid="$2"
  local label="$3"
  while (( SECONDS < deadline )); do
    if ! kill -0 "$pid" >/dev/null 2>&1; then
      fail "Python ${label} shared instance exited before ready"
    fi
    if [[ -s "$state_path" ]] && python3 - <<'PY' "$state_path"
import json
import sys
state = json.load(open(sys.argv[1], "r", encoding="utf-8"))
if state.get("ready") is True and state.get("is_shared_instance") is True:
    raise SystemExit(0)
raise SystemExit(1)
PY
    then
      return 0
    fi
    sleep 0.2
  done
  fail "timed out waiting for Python ${label} shared instance"
}

wait_for_python_ready "$PY_TCP_STATE" "$PY_TCP_PID" "TCP"
wait_for_python_ready "$PY_UNIX_STATE" "$PY_UNIX_PID" "Unix"

cat >"$RUST_CONFIG_PATH_TCP" <<EOF
[[interfaces]]
type = "LocalClientInterface"
enabled = true
name = "local-python-tcp-attach"
shared_instance_type = "tcp"
host = "127.0.0.1"
port = ${PY_TCP_PORT}
fixed_mtu = 262144
force_shared_instance_bitrate = 1000000
EOF

cat >"$RUST_CONFIG_PATH_UNIX" <<EOF
[[interfaces]]
type = "LocalClientInterface"
enabled = true
name = "local-python-unix-attach"
shared_instance_type = "unix"
socket_path = "@rns/${PY_UNIX_INSTANCE}"
fixed_mtu = 262144
force_shared_instance_bitrate = 1000000
EOF

cargo build -p reticulumd --bin reticulumd --quiet
cargo build -p rns-tools --bin rnstatus-rs --quiet

start_reticulumd() {
  local kind="$1"
  local suffix="${kind^^}"
  local rpc_addr="RPC_ADDR_${suffix}"
  local rpc_unix="RPC_UNIX_${suffix}"
  local db_path="DB_PATH_${suffix}"
  local config_path="RUST_CONFIG_PATH_${suffix}"
  local log_path="RETICULUMD_LOG_${suffix}"
  "${ROOT_DIR}/target/debug/reticulumd" \
    --rpc "${!rpc_addr}" \
    --rpc-unix "${!rpc_unix}" \
    --db "${!db_path}" \
    --config "${!config_path}" \
    --strict-interface-startup >>"${!log_path}" 2>&1 &
  printf -v "RET_PID_${suffix}" '%s' "$!"
}

rnstatus_snapshot() {
  local kind="$1"
  local suffix="${kind^^}"
  local rpc_addr="RPC_ADDR_${suffix}"
  local json_path="RNSTATUS_JSON_${suffix}"
  local human_path="RNSTATUS_HUMAN_${suffix}"
  local log_path="RETICULUMD_LOG_${suffix}"
  "${ROOT_DIR}/target/debug/rnstatus-rs" --rpc "${!rpc_addr}" --json >"${!json_path}" 2>>"${!log_path}" \
    && "${ROOT_DIR}/target/debug/rnstatus-rs" --rpc "${!rpc_addr}" >"${!human_path}" 2>>"${!log_path}"
}

start_reticulumd tcp
start_reticulumd unix
TRAFFIC_STARTED=false

verify_unix_detach_and_restart() {
  kill -INT "$RET_PID_UNIX"
  wait "$RET_PID_UNIX"
  RET_PID_UNIX=""

  local teardown_deadline=$((SECONDS + 15))
  while (( SECONDS < teardown_deadline )); do
    if python3 - <<'PY' "$PY_UNIX_STATE"
import json
import sys
state = json.load(open(sys.argv[1], "r", encoding="utf-8"))
raise SystemExit(0 if state.get("local_client_count") == 1 else 1)
PY
    then
      break
    fi
    sleep 0.2
  done
  if ! python3 - <<'PY' "$PY_UNIX_STATE"
import json
import sys
state = json.load(open(sys.argv[1], "r", encoding="utf-8"))
raise SystemExit(0 if state.get("local_client_count") == 1 else 1)
PY
  then
    fail "Python Unix shared instance did not observe reticulumd client teardown"
  fi

  start_reticulumd unix
  local restart_deadline=$((SECONDS + TIMEOUT_SECS))
  while (( SECONDS < restart_deadline )); do
    if ! kill -0 "$RET_PID_UNIX" >/dev/null 2>&1; then
      fail "reticulumd exited during Unix shared-instance restart"
    fi
    if rnstatus_snapshot unix \
      && python3 - <<'PY' "$RNSTATUS_JSON_UNIX" "$PY_UNIX_STATE"
import json
import sys
status_path, state_path = sys.argv[1:3]
status = json.load(open(status_path, "r", encoding="utf-8"))
state = json.load(open(state_path, "r", encoding="utf-8"))
row = next((item for item in status.get("interfaces", []) if item.get("name") == "local-python-unix-attach"), None)
runtime = ((row or {}).get("settings") or {}).get("_runtime") or {}
raise SystemExit(0 if state.get("local_client_count", 0) >= 2 and runtime.get("startup_status") == "attached" else 1)
PY
    then
      UNIX_TEARDOWN_RESTART_VERIFIED=true
      deadline=$((SECONDS + TIMEOUT_SECS))
      return 0
    fi
    sleep 0.2
  done
  fail "Python Unix shared instance did not reattach after reticulumd restart"
}

verify_tcp_detach_and_restart() {
  kill -INT "$RET_PID_TCP"
  wait "$RET_PID_TCP"
  RET_PID_TCP=""

  local teardown_deadline=$((SECONDS + 15))
  while (( SECONDS < teardown_deadline )); do
    if python3 - <<'PY' "$PY_TCP_STATE"
import json
import sys
state = json.load(open(sys.argv[1], "r", encoding="utf-8"))
raise SystemExit(0 if state.get("local_client_count") == 1 else 1)
PY
    then
      break
    fi
    sleep 0.2
  done
  if ! python3 - <<'PY' "$PY_TCP_STATE"
import json
import sys
state = json.load(open(sys.argv[1], "r", encoding="utf-8"))
raise SystemExit(0 if state.get("local_client_count") == 1 else 1)
PY
  then
    fail "Python TCP shared instance did not observe reticulumd client teardown"
  fi

  start_reticulumd tcp
  local restart_deadline=$((SECONDS + TIMEOUT_SECS))
  while (( SECONDS < restart_deadline )); do
    if ! kill -0 "$RET_PID_TCP" >/dev/null 2>&1; then
      fail "reticulumd exited during TCP shared-instance restart"
    fi
    if rnstatus_snapshot tcp \
      && python3 - <<'PY' "$RNSTATUS_JSON_TCP" "$PY_TCP_STATE"
import json
import sys
status_path, state_path = sys.argv[1:3]
status = json.load(open(status_path, "r", encoding="utf-8"))
state = json.load(open(state_path, "r", encoding="utf-8"))
row = next((item for item in status.get("interfaces", []) if item.get("name") == "local-python-tcp-attach"), None)
runtime = ((row or {}).get("settings") or {}).get("_runtime") or {}
raise SystemExit(0 if state.get("local_client_count", 0) >= 2 and runtime.get("startup_status") == "attached" else 1)
PY
    then
      TCP_TEARDOWN_RESTART_VERIFIED=true
      deadline=$((SECONDS + TIMEOUT_SECS))
      return 0
    fi
    sleep 0.2
  done
  fail "Python TCP shared instance did not reattach after reticulumd restart"
}

while (( SECONDS < deadline )); do
  if ! kill -0 "$RET_PID_TCP" >/dev/null 2>&1; then
    fail "reticulumd exited before Python shared-instance attach status became healthy (tcp)"
  fi
  if ! kill -0 "$RET_PID_UNIX" >/dev/null 2>&1; then
    fail "reticulumd exited before Python shared-instance attach status became healthy (unix)"
  fi
  if grep -q "bind private rpc unix socket" "$RETICULUMD_LOG_TCP"; then
    fail "reticulumd could not bind its private RPC unix socket (tcp)"
  fi
  if grep -q "bind private rpc unix socket" "$RETICULUMD_LOG_UNIX"; then
    fail "reticulumd could not bind its private RPC unix socket (unix)"
  fi
  if rnstatus_snapshot tcp && rnstatus_snapshot unix; then
    if [[ "$TRAFFIC_STARTED" == false ]] && python3 - <<'PY' "$RNSTATUS_JSON_TCP" "$RNSTATUS_JSON_UNIX" "$PY_TCP_STATE" "$PY_UNIX_STATE"
import json
import sys

tcp_json_path, unix_json_path, tcp_state_path, unix_state_path = sys.argv[1:5]
for json_path, state_path, name in [
    (tcp_json_path, tcp_state_path, "local-python-tcp-attach"),
    (unix_json_path, unix_state_path, "local-python-unix-attach"),
]:
    state = json.load(open(state_path, "r", encoding="utf-8"))
    if (state.get("local_client_count") or 0) < 1:
        raise SystemExit(1)
    payload = json.load(open(json_path, "r", encoding="utf-8"))
    row = next(
        (
            item
            for item in payload.get("interfaces", [])
            if item.get("type") == "local_client" and item.get("name") == name
        ),
        None,
    )
    if row is None:
        raise SystemExit(1)
    runtime = ((row.get("settings") or {}).get("_runtime") or {})
    if runtime.get("startup_status") != "attached":
        raise SystemExit(1)
PY
    then
      PY_TCP_TRAFFIC_PID="$(start_python_traffic "$PY_TCP_TRAFFIC_CONFIG_DIR" "$PY_TCP_TRAFFIC_STATE" "$PY_TCP_TRAFFIC_LOG" "tcp")"
      PY_UNIX_TRAFFIC_PID="$(start_python_traffic "$PY_UNIX_TRAFFIC_CONFIG_DIR" "$PY_UNIX_TRAFFIC_STATE" "$PY_UNIX_TRAFFIC_LOG" "unix")"
      TRAFFIC_STARTED=true
    fi

    if python3 - <<'PY' "$RNSTATUS_JSON_TCP" "$RNSTATUS_JSON_UNIX" "$RNSTATUS_HUMAN_TCP" "$RNSTATUS_HUMAN_UNIX" "$PY_TCP_STATE" "$PY_UNIX_STATE" "$PY_TCP_TRAFFIC_STATE" "$PY_UNIX_TRAFFIC_STATE" "$PY_TCP_PORT" "$PY_UNIX_INSTANCE"
import json
import pathlib
import sys

(
    tcp_json_path,
    unix_json_path,
    tcp_human_path,
    unix_human_path,
    tcp_state_path,
    unix_state_path,
    tcp_traffic_state_path,
    unix_traffic_state_path,
    tcp_port_raw,
    unix_instance,
) = sys.argv[1:11]
for path in [
    tcp_json_path,
    unix_json_path,
    tcp_human_path,
    unix_human_path,
    tcp_state_path,
    unix_state_path,
    tcp_traffic_state_path,
    unix_traffic_state_path,
]:
    if not pathlib.Path(path).exists():
        raise SystemExit(1)
tcp_port = int(tcp_port_raw)
tcp_state = json.load(open(tcp_state_path, "r", encoding="utf-8"))
unix_state = json.load(open(unix_state_path, "r", encoding="utf-8"))
tcp_traffic_state = json.load(open(tcp_traffic_state_path, "r", encoding="utf-8"))
unix_traffic_state = json.load(open(unix_traffic_state_path, "r", encoding="utf-8"))

expected = [
    (tcp_json_path, tcp_human_path, "local-python-tcp-attach", "tcp", "127.0.0.1", tcp_port, None),
    (unix_json_path, unix_human_path, "local-python-unix-attach", "unix", None, None, f"@rns/{unix_instance}"),
]
for json_path, human_path, name, shared_type, host, port, socket_path in expected:
    interfaces = json.load(open(json_path, "r", encoding="utf-8")).get("interfaces", [])
    row = next(
        (
            item
            for item in interfaces
            if item.get("type") == "local_client" and item.get("name") == name
        ),
        None,
    )
    if row is None or row.get("enabled") is not True:
        raise SystemExit(1)
    settings = row.get("settings") or {}
    if settings.get("shared_instance_type") != shared_type:
        raise SystemExit(1)
    if settings.get("mtu") != 262144 or settings.get("bitrate") != 1000000:
        raise SystemExit(1)
    if host is not None and row.get("host") != host:
        raise SystemExit(1)
    if port is not None and row.get("port") != port:
        raise SystemExit(1)
    if socket_path is not None and settings.get("socket_path") != socket_path:
        raise SystemExit(1)
    runtime = settings.get("_runtime") or {}
    if runtime.get("startup_status") != "attached":
        raise SystemExit(1)
    runtime_iface = runtime.get("iface") or runtime.get("runtime_iface")
    if not isinstance(runtime_iface, str) or not runtime_iface:
        raise SystemExit(1)
    human = open(human_path, "r", encoding="utf-8", errors="replace").read()
    for token in [name, " local_client ", " attached"]:
        if token not in human:
            raise SystemExit(1)

for state in [tcp_state, unix_state]:
    if (state.get("local_client_count") or 0) < 2:
        raise SystemExit(1)
    if (state.get("local_client_rxb_total") or 0) <= 0:
        raise SystemExit(1)
    if (state.get("local_client_txb_total") or 0) <= 0:
        raise SystemExit(1)
for label, state, shared_state in [
    ("tcp", tcp_traffic_state, tcp_state),
    ("unix", unix_traffic_state, unix_state),
]:
    if state.get("ready") is not True:
        raise SystemExit(1)
    if state.get("is_connected_to_shared_instance") is not True:
        raise SystemExit(1)
    if (state.get("announced_count") or 0) < 3:
        raise SystemExit(1)
    if (state.get("reverse_announced_count") or 0) < 3:
        raise SystemExit(1)
    if f"codex-local-python-shared-payload:python-to-shared:{label}" not in shared_state.get("received_payloads", []):
        raise SystemExit(1)
    if "codex-local-python-shared-payload:shared-to-python" not in state.get("received_payloads", []):
        raise SystemExit(1)
PY
    then
      if [[ "$UNIX_TEARDOWN_RESTART_VERIFIED" == false ]]; then
        verify_unix_detach_and_restart
        continue
      fi
      if [[ "$TCP_TEARDOWN_RESTART_VERIFIED" == false ]]; then
        verify_tcp_detach_and_restart
        continue
      fi
      write_report "pass"
      echo "[local-interface-python-shared-smoke] pass"
      echo "[local-interface-python-shared-smoke] report=${REPORT_PATH}"
      echo "[local-interface-python-shared-smoke] logs=${RUN_DIR}"
      exit 0
    fi
  fi
  sleep 1
done

fail "timed out waiting for healthy Python shared-instance attach status"
