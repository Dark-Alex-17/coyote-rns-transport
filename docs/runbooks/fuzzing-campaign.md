# Fuzzing Campaign Runbook

This runbook defines parser/envelope fuzz coverage for SDK v2.5 and the
Reticulum transport wire decoders.

## Covered targets

- `crates/libs/rns-rpc/fuzz/fuzz_targets/rpc_frame_envelope.rs`
  - msgpack frame decode for `RpcRequest` and `RpcResponse`
  - HTTP response body extraction and JSON RPC response parsing
- `crates/libs/lxmf-sdk/fuzz/fuzz_targets/sdk_json_envelope.rs`
  - JSON envelope decode for `EventBatch`, `SdkEvent`, `SdkError`, `RuntimeSnapshot`, `DeliverySnapshot`, and `ConfigPatch`
- `crates/libs/rns-transport/fuzz/fuzz_targets/request_frame_decode.rs`
  - `RequestFrame::decode` on arbitrary bytes; an accepted frame must re-encode and decode to the same bytes
- `crates/libs/rns-transport/fuzz/fuzz_targets/response_frame_decode.rs`
  - `ResponseFrame::decode` on arbitrary bytes, with the same re-encode check
- `crates/libs/rns-transport/fuzz/fuzz_targets/resource_advertisement_decode.rs`
  - `ResourceAdvertisement::unpack` on arbitrary bytes; an accepted advertisement must survive a pack and unpack unchanged

## CI gate (fast)

```bash
cargo run -p xtask -- sdk-fuzz-check
```

The CI gate performs:
- fuzz target compile checks for all three fuzz crates
- deterministic fuzz-smoke parser runs in `reticulum-rs-rpc`, `lxmf-sdk` and `coyote-rns-transport`

## Long campaign (recommended pre-release)

Install a nightly toolchain (libFuzzer needs one) and `cargo-fuzz` once:

```bash
rustup toolchain install nightly
cargo install cargo-fuzz
```

Run a campaign with minimum runtime of 300 seconds per target. The script
runs every target with `cargo +nightly fuzz run`; set `FUZZ_TOOLCHAIN` to use
another toolchain name. It exits non-zero when any target leaves a file in
its `fuzz/artifacts/` directory.

```bash
FUZZ_MAX_TOTAL_TIME=300 ./tools/scripts/fuzz-campaign.sh
```

Policy:
- no crashes, panics, or sanitizer failures are acceptable
- any reproducer must be committed as regression test before release
