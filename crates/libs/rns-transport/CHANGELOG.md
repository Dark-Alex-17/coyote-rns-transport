# Changelog

The section layout follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and versions follow the cargo semver rules described in the crate README.

## Unreleased

### Added

- In-crate `two_transport_loopback` test fixture: two `Transport`s joined
  over a real `TcpServer` / `TcpClient` pair on 127.0.0.1 with broadcast
  disabled on both sides, plus a smoke test that opens a link and exchanges
  one `Data` packet in each direction.

- `Link::rtt()` returns the link round-trip time: measured from the link
  request to the proof on the initiator (`RNS/Link.py:419`); on the
  responder the larger of its own measurement and the value in the
  initiator's RTT packet (`RNS/Link.py:518-522`); zero until then.

- `Transport::stop()` detaches every interface and waits for their workers,
  then cancels the transport and waits for the management task, up to a
  5 s budget after which whatever is still running is aborted and logged;
  a second call is a no-op. `Drop` still only cancels.

- `Transport::try_send_announce()` returns the announce build error to the
  caller instead of logging it.

- `rmpv` and `rand_core` are re-exported from the crate root so callers can
  name the exact versions the public API uses; an integration test exercises
  both through the crate root.

- `destination_hash::destination_address()` computes a destination address
  from a name hash and an identity hash (`RNS/Destination.py:116-130`).

- `AddressHash::short()` returns the first eight hex chars for log lines.

### Changed

- Workspace clippy configuration names the tokio `MutexGuard`,
  `RwLockReadGuard` and `RwLockWriteGuard` types under
  `await-holding-invalid-types`, with the lint set to `allow` in the
  workspace and `reticulumd` lint tables until the existing hits are
  cleared.

### Fixed

- `AddressHash::new_from_hex_string`, `Identity::new_from_hex_string` and
  `PrivateIdentity::new_from_hex_string` sliced the input by byte offset and
  panicked on a multibyte UTF-8 char at a slice boundary. They now decode
  with `hex::decode` and return `RnsError::IncorrectHash` on any malformed
  input; `+`-prefixed byte pairs, which `u8::from_str_radix` tolerated, are
  now rejected too.

- `Transport::send_announce` panicked when the announce could not be built
  (app data too large for the packet buffer). It now logs at warn and
  returns.

- An inbound Resource advertisement with a transfer size above
  `3 * MAX_EFFICIENT_SIZE` is dropped and logged at debug before any
  receiver state is created, matching the decode check in
  `RNS/Resource.py:1374`. Split Resources advertise each segment with its
  own size, so larger totals are unaffected.

- Multi-segment Resource advertisements and Resource retries were routed
  through `route_outbound_packet`, which never resolves a link id, so
  segments 2 and later of any Resource larger than `MAX_EFFICIENT_SIZE`
  were dropped as `DroppedNoRoute` unless `broadcast` was on. They are now
  sent on the link's bound interface like the first segment
  (`handle_resource_proof` and the resource retry worker), matching
  `RNS/Transport.py:1452-1454`; retry dispatch now resolves initiator-side
  links by link id as well (`out_links` is keyed by destination hash).

- `handle_link_resource_packet` sent a Resource advertisement reject
  (`ResourceReceiverCancel`) while still holding the link's mutex guard;
  `TransportHandler::send` re-locks that link in `note_link_packet_sent`,
  so any oversize request or response advertisement deadlocked the
  transport. The reject is now built under the guard and sent after it is
  dropped; the rejected advertisement still never reaches the
  `ResourceManager`.

- `ResourceManager::track_prepared` now registers the outbound sender in
  `outgoing` before the caller writes the advertisement, so a fast peer's
  first part request is served instead of dropped; the `pending_outgoing`
  staging map is gone. `track_prepared` and
  `ResourceManager::confirm_outbound_dispatch` keep their signatures but
  change semantics: `track_prepared` places the sender in `outgoing` with
  `ResourceStatus::None`; `confirm_outbound_dispatch(_, true)` marks a
  still-`None` sender advertised in place and then, when an early request
  already served parts, moves it on to `Transferring` or (all parts
  served) `AwaitingProof` with the bounded proof budget, exactly as a
  post-confirm request would have; a sender an early request already
  failed keeps `Failed` for the retry poll to report `OutboundFailed`;
  `confirm_outbound_dispatch(_, false)` removes it and emits
  `OutboundFailed`; `cancel_outgoing` before the confirm now returns a
  `ResourceInitiatorCancel` packet and emits `OutboundCancelled`.

## 0.14.0 - 2026-10-14

Initial fork release, based on upstream `reticulum-rs-transport` 0.14.0.

### Changed

- Package renamed to `coyote-rns-transport`; the library target stays
  `rns_transport`.

### Removed

- The `println!` on link close in `Link::finalize_local_close`; the
  `log::warn!` on the next line carries the information. See
  [docs/fork/PATCHSET.md](https://github.com/Dark-Alex-17/coyote-rns-transport/blob/main/docs/fork/PATCHSET.md).
