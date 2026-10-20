import hashlib
import json
import sys
import threading
import time

import RNS


def request_forms_body(size: int) -> bytes:
    return bytes((i % 251) for i in range(size))


def emit(payload: dict) -> None:
    # One write, so the report cannot interleave with RNS log lines
    # the transport thread prints to the same stdout.
    sys.stdout.write(json.dumps(payload) + "\n")
    sys.stdout.flush()


def serve_request(path, data, request_id):
    if isinstance(data, dict) and data.get("response") == "none":
        response = None
    elif isinstance(data, dict) and "response_size" in data:
        response = request_forms_body(int(data["response_size"]))
    else:
        response = f"reply:{data}"
    if response is None:
        response_kind = "none"
        response_bytes = b""
    elif isinstance(response, bytes):
        response_kind = "bytes"
        response_bytes = response
    else:
        response_kind = "text"
        response_bytes = response.encode("utf-8")
    emit(
        {
            "request_received": True,
            "request_id": request_id.hex(),
            "path": path,
            "response_kind": response_kind,
            "response_size": len(response_bytes),
            "response_sha256": None if response is None else hashlib.sha256(response_bytes).hexdigest(),
        }
    )
    return response


def run_client(
    client,
    active_link,
    message_data: str,
    request_size: int,
    identify: bool,
    timeout: float,
    request_timeout: float | None,
    deadline: float,
) -> int:
    if identify:
        client.identity = RNS.Identity()
        active_link.set_packet_callback(client._on_link_data)
        active_link.identify(client.identity)
        emit({"identified": client.identity.hash.hex()})
        while True:
            with client.lock:
                replies = list(client.received)
            if any(reply["data"] == "go" for reply in replies):
                break
            if time.time() > deadline:
                print("python_channel_client: timed out waiting for go", file=sys.stderr, flush=True)
                return 1
            time.sleep(0.05)

    done = threading.Event()
    result = {}
    request_data = ("x" * request_size) if request_size > 0 else message_data

    def on_response(receipt) -> None:
        result["response"] = receipt.response
        done.set()

    def on_failed(receipt) -> None:
        result["failed"] = True
        done.set()

    # Anchored here rather than at run() start so link setup and the identify
    # handshake do not eat into the window a silently dropped request gets.
    request_deadline = time.time() + timeout
    receipt = active_link.request(
        "/test/request",
        data=request_data,
        response_callback=on_response,
        failed_callback=on_failed,
        timeout=timeout if request_timeout is None else request_timeout,
    )
    if receipt is False:
        print("python_channel_client: failed to send request", file=sys.stderr, flush=True)
        return 1
    request_id = receipt.request_id.hex()
    request_form = "resource" if receipt.resource is not None else "packet"
    emit({"request_sent": True, "request_id": request_id, "request_form": request_form})
    while not done.is_set():
        if time.time() > request_deadline:
            break
        time.sleep(0.05)

    # Every terminal outcome exits 0 and reports which one it was; the
    # Rust test decides whether that outcome was the expected one.
    if "response" in result:
        response = result["response"]
        body = response if isinstance(response, bytes) else str(response).encode("utf-8")
        emit(
            {
                "request_id": request_id,
                "request_form": request_form,
                "response_size": len(body),
                "response_sha256": hashlib.sha256(body).hexdigest(),
            }
        )
        return 0
    if result.get("failed"):
        emit(
            {
                "request_id": request_id,
                "request_form": request_form,
                "request_failed": True,
                "status": receipt.status,
            }
        )
        return 0
    # A packet-form request that is silently dropped never fires
    # failed_callback: request_timed_out requires status DELIVERED
    # (RNS/Link.py:1398), which only the Resource form sets (:1372).
    emit(
        {
            "request_id": request_id,
            "request_form": request_form,
            "request_no_response": True,
            "status": receipt.status,
        }
    )
    return 0
