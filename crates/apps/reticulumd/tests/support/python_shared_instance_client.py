#!/usr/bin/env python3
"""Second local client of a Python rnsd shared instance.

Registers an echoing request handler, reports the Rust announce when it
arrives, sends one Link.request to the Rust destination, and then keeps
announcing its own destination until stdin closes so the Rust side can run
the reverse direction. stdout carries one JSON object per line; everything
else goes to stderr.
"""

from __future__ import annotations

import argparse
import json
import sys
import threading
import time

import RNS

PATH_REQUEST_INTERVAL = 2.0


def emit(payload: dict) -> None:
    # One write, so the report cannot interleave with RNS log lines
    # the transport thread prints to the same stdout.
    sys.stdout.write(json.dumps(payload) + "\n")
    sys.stdout.flush()


def log(message: str) -> None:
    print(f"python_shared_instance_client: {message}", file=sys.stderr, flush=True)


def echo_request(path, data, request_id, link_id, remote_identity, requested_at):
    return data


class RustAnnounceHandler:
    aspect_filter = None
    # rnsd answers this client's path request with a path response
    # (RNS/Transport.py:3459-3520); without this flag Python drops it before
    # the handler (RNS/Transport.py:2500-2504).
    receive_path_responses = True

    def __init__(self, rust_hash: bytes) -> None:
        self.rust_hash = rust_hash
        self.reported = False

    def received_announce(self, destination_hash, announced_identity, app_data) -> None:
        if destination_hash != self.rust_hash or self.reported:
            return
        self.reported = True
        emit(
            {
                "announce_received": destination_hash.hex(),
                "hops": RNS.Transport.hops_to(destination_hash),
            }
        )


def wait_for_path(rust_hash: bytes, destination, deadline: float, announce_interval: float) -> bool:
    last_path_request = 0.0
    while not RNS.Transport.has_path(rust_hash):
        now = time.time()
        if now > deadline:
            log("timed out waiting for a path to the Rust destination")
            return False
        if now - last_path_request >= PATH_REQUEST_INTERVAL:
            RNS.Transport.request_path(rust_hash)
            last_path_request = now
        destination.announce()
        time.sleep(announce_interval)
    return True


def wait_for_link(link, deadline: float) -> bool:
    while link.status != RNS.Link.ACTIVE:
        if link.status == RNS.Link.CLOSED:
            log("link to the Rust destination closed before it became active")
            return False
        if time.time() > deadline:
            log("timed out waiting for the link to the Rust destination")
            return False
        time.sleep(0.05)
    return True


def response_text(response) -> str:
    if isinstance(response, str):
        return response
    if isinstance(response, bytes):
        return response.hex()
    return str(response)


def hold_until_stdin_closes(destination, announce_interval: float) -> None:
    closed = threading.Event()

    def watch_stdin() -> None:
        sys.stdin.buffer.read()
        closed.set()

    threading.Thread(target=watch_stdin, daemon=True).start()
    while not closed.wait(announce_interval):
        destination.announce()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--config-dir", required=True)
    parser.add_argument("--rust-destination-hash", required=True)
    parser.add_argument("--request-path", default="/test/request")
    parser.add_argument("--request-data", default="hello-rust")
    parser.add_argument("--timeout", type=float, default=30.0)
    parser.add_argument("--announce-interval", type=float, default=0.25)
    args = parser.parse_args()

    rust_hash = bytes.fromhex(args.rust_destination_hash)
    log("starting Reticulum")
    reticulum = RNS.Reticulum(configdir=args.config_dir, loglevel=7)
    if not reticulum.is_connected_to_shared_instance:
        emit({"ready": False, "error": "not a shared-instance client"})
        return 1

    identity = RNS.Identity()
    destination = RNS.Destination(
        identity,
        RNS.Destination.IN,
        RNS.Destination.SINGLE,
        "test",
        "shared",
    )
    destination.register_request_handler(
        args.request_path,
        response_generator=echo_request,
        allow=RNS.Destination.ALLOW_ALL,
    )
    RNS.Transport.register_announce_handler(RustAnnounceHandler(rust_hash))
    emit({"ready": True, "destination_hash": destination.hash.hex()})

    deadline = time.time() + args.timeout
    if not wait_for_path(rust_hash, destination, deadline, args.announce_interval):
        return 1

    recalled = RNS.Identity.recall(rust_hash)
    if recalled is None:
        log("Rust destination identity not recalled")
        return 1
    rust_destination = RNS.Destination(
        recalled,
        RNS.Destination.OUT,
        RNS.Destination.SINGLE,
        "test",
        "shared",
    )
    link = RNS.Link(rust_destination)
    if not wait_for_link(link, deadline):
        return 1

    done = threading.Event()
    result = {}

    def on_response(receipt) -> None:
        result["response"] = receipt.response
        done.set()

    def on_failed(receipt) -> None:
        result["failed"] = True
        done.set()

    receipt = link.request(
        args.request_path,
        data=args.request_data,
        response_callback=on_response,
        failed_callback=on_failed,
        timeout=args.timeout,
    )
    if receipt is False:
        emit({"request_failed": True, "reason": "not sent"})
        return 1
    if not done.wait(max(deadline - time.time(), args.timeout)):
        emit({"request_failed": True, "reason": "timeout"})
        return 1
    if "response" not in result:
        emit({"request_failed": True, "reason": "failed"})
        return 1
    emit({"response": response_text(result["response"])})

    hold_until_stdin_closes(destination, args.announce_interval)
    return 0


if __name__ == "__main__":
    sys.exit(main())
