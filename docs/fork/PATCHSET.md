# coyote-rns-transport patch set

One section per published fork release of `coyote-rns-transport`. Each
section names the upstream `reticulum-rs-transport` release the fork build is
based on and lists every behavioural patch applied on top of it, with the
upstream site of the change and the fork commit that introduced it. The
package rename (`reticulum-rs-transport` to `coyote-rns-transport`) and the
matching edits to workspace manifests, scripts, CI workflows and docs are
not patches in this sense and are not listed here. The crate README
(`crates/libs/rns-transport/README.md`) carries the versioning policy, the
rebase recipe and a prose summary of the current patch list; this file is
the per-release ledger of the same patches, with commit hashes and file
sites, so a rebase can re-apply them one commit at a time.

## Unreleased

Fork commits `7f8b4c96` and `b64cb536`: send later Resource
segments and Resource retries on the link's bound interface. The second
commit reshapes the link lookup and its unit test; the file sites below
describe the combined result.

- `crates/libs/rns-transport/src/transport/resource_wire.rs`
  (`handle_resource_proof`): the advertisement for the next segment, built
  by the ResourceManager when the previous segment's proof arrives, was
  dispatched with `handler.send_packet(response)`, which routes by the path
  table and never resolves a link id. It is now sent as
  `TxMessage { tx_type: TxMessageType::Direct(response_iface), .. }` where
  `response_iface` is `link.ingress_iface().unwrap_or(iface)`, captured
  before the link guard is dropped, matching the lower half of the same
  file and `RNS/Transport.py:1452-1454`.
- `crates/libs/rns-transport/src/transport/jobs.rs` (`resource-retry`
  worker, both loops): request and advertisement retries resolve the link
  by the id `retry_requests` / `poll_outgoing` returned, using
  `resource_wire::link_by_id` (shared with `link_for_resource_packet`),
  then read the link's `ingress_iface()` under the link guard. `link_by_id`
  takes the `in_links` and `out_links` maps, tries the keyed lookups and
  then scans `out_links` by `link.id()` because that map is keyed by
  destination hash; a plain `get` on `out_links` never found an
  initiator-side link. A retry whose link is gone is skipped in both loops.
  Both dispatch through the new private
  `send_resource_retry`, which sends `Direct(iface)` and logs at debug when
  the dispatch trace reports no interface written. Path routing is only a
  fallback for a link without a bound interface.
- `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_resources.rs`
  (new): three broadcast-off loopback tests send a 300 KiB, a
  `MAX_EFFICIENT_SIZE + 64 KiB` and a `4 MiB + 1` response Resource and
  check the segment count, the payload and both terminal events. The two
  multi-segment cases time out on `2079a74e`; the single-segment case
  passes on both.
- `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_resources_directions.rs`
  (new): the initiator (out-link) side sending a split Resource, a split
  request Resource, and two split Resources back to back on one link.
- `crates/libs/rns-transport/src/transport/tests_parts/resource_retry_link_lookup.rs`
  (new): unit test for `link_by_id` resolving an `out_links` entry by link
  id, an `in_links` entry, and an unknown id, against link maps the test
  owns.
- `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_resources_retries.rs`
  (new): the listener's handler lock is held for five seconds so the
  connector's retry worker re-advertises (and, in the second test,
  re-requests); the retried packet must reach the listener's interface and
  the duplicate must not produce a second `Complete`. Both fail on
  `2079a74e`, each seeing a single advertisement or request.
- `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_resources_gone_link.rs`
  (new): the connector forgets its `out_links` after the first advertisement
  is on the wire; exactly one advertisement may reach the listener, the
  sender must still report `OutboundFailed` once its retries are spent, and
  a fresh link over the same transports must still carry a Resource.
- `crates/libs/rns-transport/src/transport/tests.rs`: one `include!` line
  for each of the five test files above.
- `crates/libs/rns-transport/CHANGELOG.md`: `### Fixed` bullet under
  `## Unreleased`.

Fork commits `e239a239`, `7cfe7e73`, `40b392b7`, `8eaf1ee7`, `915c214f`,
`cfd7b868` and `21b6dcb3`:
send Resource advertisement rejects outside the link lock, and register an
outbound Resource sender before its advertisement is written. The second
commit guards the dispatch confirm and moves the unconfirmed-sender tests;
the third only touches the `track_prepared` doc comment in
`manager_start.rs` (it cites `RNS/Resource.py:545` and `:552` for the
reference ordering); the fourth carries parts served before the confirm
into the sender status; the fifth adds the response-size reject loopback
test; the sixth only updates this ledger; the seventh adopts the
served-parts confirm tests. The file sites below describe the combined
result.

- `crates/libs/rns-transport/src/transport/resource_wire.rs`
  (`handle_link_resource_packet`): the response-size and request-size
  rejects (`ResourceReceiverCancel`) were sent with `handler.send` while the
  link's mutex guard was still held; `TransportHandler::send` re-locks the
  same link in `note_link_packet_sent`, so the first reject deadlocked the
  transport. Both branches now record the reject in a local
  `Option<TxMessage>` (built by the new private `advertisement_reject`
  helper, with the interface captured under the guard), skip the
  ResourceManager dispatch, and send the reject after the existing
  `drop(link)`. A rejected advertisement still never reaches the manager,
  including when the reject packet cannot be built.
- `crates/libs/rns-transport/src/resource/manager_start.rs`
  (`track_prepared`), `manager_segments.rs` (`confirm_outbound_dispatch`),
  `manager.rs` (struct, `cancel_outgoing`, `has_no_outbound_state`, the
  `ResourceReceiverCancel` arm), `manager_cleanup.rs` (`remove_link_state`):
  the `pending_outgoing` staging map is removed. `track_prepared` inserts the
  sender straight into `outgoing` with `ResourceStatus::None`, which
  `ResourceSender::poll` treats as nothing to retry while
  `handle_request_into` serves parts, so a part request that arrives between
  the advertisement write and the confirm is answered.
  `confirm_outbound_dispatch(_, true)` acts on the sender in place only
  while it is still in `ResourceStatus::None`: it calls the new private
  `ResourceSender::confirm_advertised`, which runs `mark_advertised` and
  then, when an early request already served parts, applies the transition
  a post-confirm request makes (all parts sent gives `AwaitingProof` with
  the bounded proof budget `max_retries.clamp(1, MAX_AWAITING_PROOF_RETRIES)`,
  otherwise `Transferring`) through the shared private
  `settle_transfer_status` helper, keeping the serve-time `last_activity`
  and `last_part_sent`; `handle_request_into` records those two timestamps
  while the status is `None` so the confirm can carry them over. Without
  this a fully served sender sat in `Advertised` and the retry poll re-sent
  the advertisement. A sender an early request already drove to `Failed`
  keeps that status for the retry poll to report as `OutboundFailed`.
  `(_, false)` removes it and emits `OutboundFailed`; a hash no longer
  present is a no-op. Both methods are `pub` and keep their signatures.
  `cancel_outgoing` before the confirm now finds the sender and returns a
  `ResourceInitiatorCancel` packet with `OutboundCancelled`.
- `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_resources_reject.rs`
  (new): the listener arms `set_max_request_size(1024)`, the connector sends
  a 4 KiB random request Resource and must see `OutboundRejected`, then a
  256-byte request on the same link must complete, and
  `Transport::stop_interface` on the listener must return within 5 s. On
  `73e04b76` the reject bytes leave the wire before the re-lock, so the
  connector still sees `OutboundRejected`; the listener's handler then wedges
  and the test times out waiting for the second request's `Complete`.
- `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_resources_reject_response.rs`
  (new): the other reject branch. The connector bounds its request's
  response at 1024 bytes with `send_request_resource_with_max_response_size`,
  the listener's 4 KiB response must see `OutboundRejected`, a 256-byte
  response on the same link must complete, and `Transport::stop_interface`
  on the connector must return within 5 s. On `73e04b76` the connector's
  handler wedges after the reject, as the listener's does in the request-size
  test.
- `crates/libs/rns-transport/src/resource/tests_split_unconfirmed.rs` (new):
  nine ResourceManager tests for the unconfirmed sender (an early part
  request is served, nothing is retried, and the confirm leaves the sender
  `Transferring` with later requests still served; a failed confirm drops
  the sender and its tail chain; a local cancel before the confirm returns
  an initiator cancel; a sequencing-error request before the confirm leaves
  the sender `Failed` and the next poll reports it once; link teardown
  before the confirm fails the sender once and the late confirm is a no-op;
  a repeated confirm leaves an advertised sender alone and the advertisement
  budget then runs as usual; a failed confirm settles only the hash it
  names; a single-part sender whose only part was served before the confirm
  is `AwaitingProof` afterwards, is not re-advertised by the retry poll, and
  completes on the proof; the same sender without a proof is dropped once
  the bounded proof budget is spent). The first, third and fourth
  (`a_sender_failed_before_the_dispatch_confirm_stays_failed`) fail on
  `73e04b76`; the fourth also fails on `1365b6a2`, where the confirm still
  read `Advertised`; the first and the last two fail on `40b392b7`, where the
  confirm left a served sender in `Advertised` and the retry poll re-sent
  the advertisement. The split fixture uses random bytes so its first
  segment keeps more than one part.
- `crates/libs/rns-transport/src/resource/tests_split_unconfirmed_served.rs`
  (new): six ResourceManager tests for a sender that served parts before the
  confirm, on the `tests_split_unconfirmed.rs` fixtures (a repeated confirm
  leaves a `Transferring` sender alone and later requests are still served;
  a repeated confirm leaves an `AwaitingProof` sender alone and the proof
  still completes it; a failed confirm after served parts drops the sender
  and its tail chain and a later request is not served; a proof that lands
  before the confirm completes the sender and the late confirm is a no-op;
  a split send whose first segment served a part before the confirm hands
  off to the second segment on the proof, and that segment's advertisement
  is live in the retry poll; a remote cancel after served parts and before
  the confirm drops the sender as `OutboundRejected` and the late confirm is
  a no-op). The first, second and fifth fail on `40b392b7`, where the
  confirm left a served sender in `Advertised` and the retry poll re-sent
  the advertisement.
- `crates/libs/rns-transport/src/resource/tests_split_lazy.rs`: the
  `split_send_builds_only_the_first_segment_up_front` assertion is rewritten
  to the new placement; `tests_timeouts.rs` and `tests_timeouts_cleanup.rs`
  lose their `pending_outgoing` asserts and `tests_timeouts.rs` gains an
  `outgoing_segment_chains.is_empty()` assert.
- `crates/libs/rns-transport/src/transport/tests.rs` and
  `crates/libs/rns-transport/src/resource/tests.rs`: two `include!` lines
  in the former, two in the latter.
- `crates/libs/rns-transport/CHANGELOG.md`: two `### Fixed` bullets under
  `## Unreleased`, including the `track_prepared` /
  `confirm_outbound_dispatch` semantics note.

Fork commit `e69eff43`: harden the hex parsers, make announce building
non-fatal, drop over-cap Resource advertisements, and add the additive
lifecycle and helper API. Every public change is additive; no existing
signature, enum or public field changes.

- `crates/libs/rns-transport/src/hash.rs`
  (`AddressHash::new_from_hex_string`) and
  `crates/libs/rns-transport/src/identity.rs`
  (`Identity::new_from_hex_string`, `PrivateIdentity::new_from_hex_string`,
  new private `decode_key_pair_hex`): the parsers byte-sliced the `&str`
  two chars at a time, which panics on a multibyte UTF-8 char at a slice
  boundary. All three now run `hex::decode` first and map any length or
  digit error to `RnsError::IncorrectHash`. `hash.rs` also gains
  `AddressHash::short()` (first eight hex chars) for warn-level log lines.
  Tests in `hash.rs` and `identity/tests.rs`: a correct-byte-length input
  with `U+00E9` straddling the first two-char slice and a non-hex ASCII
  input of the right length return `IncorrectHash` for each parser; the
  leading-char case panics with `end byte index 2 is not a char boundary`
  on `21b6dcb3`. Further tests for each parser: `U+00E9` straddling the
  last two-char slice and `U+1F600` in the middle return `IncorrectHash`,
  empty and odd-length input is rejected, uppercase hex is accepted; and
  `short_is_first_eight_hex_chars` in `hash.rs`.
- `crates/libs/rns-transport/src/transport/core.rs` (`send_announce`): the
  `.expect("valid announce packet")` is replaced by a warn (short
  destination hash and the error) and an early return. New
  `try_send_announce` beside it takes the destination guard in a scoped
  block, logs the same trace line as `send_announce`, builds the packet,
  drops the guard and returns the build error through
  `Result<(), RnsError>` (`Ok(())` means the packet was built and handed
  to the transport, not that an interface accepted it).
- `crates/libs/rns-transport/src/transport/mod.rs`: new private
  `manage_task: std::sync::Mutex<Option<JoinHandle<()>>>` field on
  `Transport` and the `mod lifecycle;` line; `Transport::new` in `core.rs`
  stores the `manage_transport` `JoinHandle` in that field.
- `crates/libs/rns-transport/src/transport/lifecycle.rs` (new):
  `Transport::stop` takes every interface worker tree with
  `take_worker_tree` and detaches the interfaces under one scoped
  `iface_manager` guard, waits for the workers, then cancels the token,
  takes the stored management task handle (std guard dropped before any
  await) and waits for it. The interface workers go first because one may
  be blocked handing a packet to the management task. The whole call shares
  a 5 s budget (`tokio::time::timeout_at`); a handle still running at the
  deadline is aborted and logged at warn with the interface's short hash.
  `Drop` stays cancel-only.
- `crates/libs/rns-transport/src/resource/advertisement_limits.rs`
  (`advertisement_exceeds_inbound_limits`): a new first branch, keyed on
  the new `pub(crate) MAX_REFERENCE_ADVERTISED_TRANSFER_SIZE`
  (`3 * MAX_EFFICIENT_SIZE`), logs at debug and rejects, mirroring the
  `ValueError("Invalid transfer size")` raised by `RNS/Resource.py:1374`.
  The 64 MiB `MAX_INBOUND_RESOURCE_TRANSFER_SIZE` branch below it is
  unreachable while the reference cap is the smaller one; it is kept so the
  upstream diff stays small and because the receiver re-checks that cap.
  `MAX_EFFICIENT_SIZE` is unchanged.
- `crates/libs/rns-transport/src/destination/link_parts/link_sections/remove_channel_handler.rs`:
  `Link::rtt()` exposes the `rtt` field: set on activation on the initiator
  (`RNS/Link.py:419`); on the responder set to the larger of its own
  measurement and the value in the initiator's RTT packet
  (`RNS/Link.py:518-522`), zero until then.
- `crates/libs/rns-transport/src/destination_hash.rs`:
  `destination_address(name_hash, identity_hash)` computes the address the
  way `create_address_hash` in `destination_parts/destination.rs` does
  (`RNS/Destination.py:116-130`), with a unit test against
  `SingleInputDestination::new` for three names.
- `crates/libs/rns-transport/src/lib.rs`: documented `pub use rand_core;`
  and `pub use rmpv;`.
- `crates/libs/rns-transport/src/resource/tests.rs`:
  `resource_manager_rejects_advertisement_over_transfer_size_limit` is
  renamed `..._over_reference_transfer_size_limit` and its bounds move to
  `3 * MAX_EFFICIENT_SIZE` (plus one rejected, exactly at the cap
  accepted); the old second half, which accepted a 64 MiB single-segment
  advertisement, fails on the new guard by design. One `include!` line for
  the file below.
- `crates/libs/rns-transport/src/resource/tests_advert_limits.rs` (new):
  the first segment of a 64-segment split Resource at `MAX_EFFICIENT_SIZE`
  per segment (`d` at 64 times that) is admitted, and a well-formed
  advertisement after a dropped over-cap one is still admitted on the same
  link.
- `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_lifecycle.rs`
  (new, two `include!` lines in `transport/tests.rs` for this file and the
  probe file below): `Link::rtt` is
  non-zero and below the loopback timeout on both ends; `Transport::stop`
  on the listener returns within 5 s, leaves `interface_hashes()` empty,
  the server status `closed` and the port bindable, and a second `stop`
  plus the connector's `stop` return within 5 s; `send_announce` with
  `PACKET_DATA_MAX + 1` bytes of app data returns (panics with
  `valid announce packet` on `21b6dcb3`) and `try_send_announce` returns
  `Err(RnsError::OutOfMemory)` for it and `Ok` for a short payload; an
  advertisement with `t = MAX_REFERENCE_ADVERTISED_TRANSFER_SIZE + 1` sent
  on a live link produces no part
  request back and no resource event, and the link then carries a `Data`
  packet each way.
- `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_lifecycle_probe.rs`
  (new): a successful `try_send_announce` is observed by the peer with the
  same app data; `stop` on a transport that never attached an interface
  returns promptly, twice; `stop` releases a `TcpClient` stuck reconnecting
  to a dead port (the client is detached and its stream state is `closed`)
  and a second `stop` is a no-op; after `stop` on the listener the public
  API keeps answering within the budget without panicking
  (`try_send_announce`, `send_announce`, `send_to_in_links_with_report`
  reporting zero sent links, `add_destination`, the peer's
  `send_to_out_links_with_report` and the peer's own `stop`); `stop` on the
  listener returns within the budget while the peer floods announces and
  leaves the server `closed`; two concurrent `stop` calls on one transport
  both return and release the listening socket.
- `crates/libs/rns-transport/tests/crate_root_reexports.rs` (new
  integration test target): `rns_transport::rand_core::OsRng` drives the
  public constructors, `rns_transport::rmpv` round-trips a `Value`, and
  `destination_address` matches `SingleInputDestination::new` from outside
  the crate.
- `crates/libs/rns-transport/CHANGELOG.md`: six `### Added` and three
  `### Fixed` bullets under `## Unreleased`.

Fork commit `0197768d`: revise the smoke-script leg of the link-close patch.
This is a mixed commit; only the files below are the patch leg, so re-apply
those hunks rather than cherry-picking the whole commit.

- `tools/scripts/python-lxmd-rust-lxmd-smoke.sh`: no longer sets
  `NO_COLOR=1` for the Rust `lxmd` process. Both link-close waits now match
  the warn's own message text with the needle `close /[0-9a-f]{32}/`, so
  they no longer depend on how the log subscriber renders the target.
- `crates/libs/rns-transport/README.md`: the patch-list paragraph is
  reworded to describe the message-text needle.
- `crates/apps/reticulumd/tests/fork_publish_contract.rs`: the test
  `link_close_needle_is_derivable_from_the_surviving_warn` pins the needle
  to the surviving `warn!` and to both smoke-script wait sites.

## 0.14.0 (upstream 0.14.0)

Fork commit `3f20c991`: drop the stdout print on link close.

- `crates/libs/rns-transport/src/destination/link_parts/link_sections/next_watchdog_deadline.rs:143`
  (upstream site, inside `Link::finalize_local_close`): the line
  `println!("{}", link_close_line(&self.id));` is removed. The
  `log::warn!("close {}", self.id);` immediately after it survives and
  carries the same information through the logging facade.
- `crates/libs/rns-transport/src/destination/link_parts/clamp_link_signalling.rs`:
  `link_close_line` gains `#[cfg(test)]`; its only remaining caller is the
  compatibility-marker unit test.
- `crates/libs/rns-transport/src/no_stdout_prints_tests.rs` (new): source
  sweep test that walks `src/`, skips test files and inline test modules,
  and fails if any remaining line invokes `print!`, `println!`, `eprint!`,
  `eprintln!` or `dbg!`.
- `crates/libs/rns-transport/src/lib.rs`: registers `no_stdout_prints_tests`
  under `#[cfg(test)]`.
- `tools/scripts/python-lxmd-rust-lxmd-smoke.sh`: sets `NO_COLOR=1` for the
  Rust `lxmd` process so the smoke script's "link close" wait can match the
  plain `warn!` rendering now that the stdout line is gone (superseded on
  `main` by `0197768d`; see Unreleased).
