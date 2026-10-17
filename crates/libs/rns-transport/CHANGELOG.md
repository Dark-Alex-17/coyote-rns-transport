# Changelog

The section layout follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and versions follow the cargo semver rules described in the crate README.
Test-only files are not listed as entries of their own; each is named in a
trailing clause of the bullet for the behaviour it pins.

## Unreleased

### Added

- `destination::request::{RequestFrame, ResponseFrame, RequestId, PathHash,
  FrameDecodeError}`: the wire frames of the link request layer. A request
  is msgpack `[requested_at, path_hash, data]` (`RNS/Link.py:486-488`), a
  response `[request_id, body]` (`RNS/Link.py:848`); `PathHash::of` is
  `truncated_hash(path)` (`RNS/Link.py:486`) and `RequestId` has the packet
  form (`RNS/Link.py:996`) and the resource form (`RNS/Link.py:504`).
  `requested_at` always encodes as `float64` (marker `0xcb`), as the
  vendored `umsgpack` writes it, and decodes from nothing else. Before any
  value is built, decoding walks the input once without allocating and
  refuses, with a typed `FrameDecodeError`, container nesting deeper than
  128 levels, a length prefix larger than the remaining input, the reserved
  marker, a truncated value or trailing bytes; the error converts into
  `RnsError::PacketError`. Pinned against Python-generated vectors by
  `destination/request/tests.rs` and
  `tests/request_frame_wire.rs`; `ResourceAdvertisement::{pack,unpack}`
  field values, key order `t d n h r o i l q f m`
  (`RNS/Resource.py:1337-1347`) and flag layout (`RNS/Resource.py:1318`)
  are pinned against three Python-generated advertisements by
  `resource/tests_advert_kat.rs`.

- The fuzz crate `crates/libs/rns-transport/fuzz/` with the targets
  `request_frame_decode`, `response_frame_decode` and
  `resource_advertisement_decode`; `cargo run -p xtask -- sdk-fuzz-check`
  compiles it and runs the deterministic smoke test
  `fuzz_smoke_request_frames_and_advertisement_decoders_do_not_panic` in
  `destination/request/tests.rs`.

- `destination::request::{RequestAllow, RequestHandler, InboundRequest,
  RequestResponse}` and `SingleInputDestination::register_request_handler`
  / `deregister_request_handler`: the responder half of the link request
  layer (`RNS/Destination.py:381-412`). The table is keyed by
  `PathHash::of(path)` (`RNS/Destination.py:396`), an empty path is refused
  with `RnsError::InvalidArgument` (`:392`) and a second registration for
  the same path replaces the first, allow mode included (`:398`). A handler
  is any `Fn(InboundRequest) -> Future<Output = Option<RequestResponse>>`;
  `InboundRequest` carries the
  six arguments Python hands a `response_generator` (`RNS/Link.py:831`) and
  `RequestResponse::{Value, Bytes, Resource}` are the plain, `bytes` and
  file-response forms (`RNS/Link.py:839-852`). The registry is pinned by
  `destination/tests.rs`, `tests/request_handler_api.rs` and
  `transport/tests_parts/two_transport_loopback_requests_routing.rs`; the
  response forms by `transport/tests_parts/two_transport_loopback_requests.rs`
  and `two_transport_loopback_requests_probe.rs`.

- The `request-dispatch` transport worker: `RNS.Link.handle_request`
  (`RNS/Link.py:804-855`). A link hands every decrypted request packet to
  the worker in-path, the worker looks the handler up and runs it on a task
  of its own, as Python runs `handle_request` on a thread of its own
  (`RNS/Link.py:1004-1005`), so a slow or panicking handler delays no other
  request and never stops the transport. `RequestAllow::None` denies,
  `All` answers, `List` answers a peer whose `LINKIDENTIFY` identity is in
  the set, read from the link when each request arrives; a denied peer and
  a peer that never identified are both denied silently
  (`RNS/Link.py:818-824,855`). The handler table consulted is the one of
  the destination that owns the link (`:810`). Only an active link hands a
  request on (`:805`), and a request larger than the destination's
  `max_request_size` is ignored before any handler runs (`:999-1001`).
  `None` from the handler sends nothing (`:844`). A response that fits
  `Link::link_mdu()` goes out as one `Response` packet, a larger one as a
  response Resource (`:849-852`). A request body that is not a request
  frame is dropped with a debug log, as Python logs and drops a body
  `unpackb` or `handle_request` cannot read (`:1003-1007`, `:804-807`): no
  handler runs, nothing is answered and the link keeps answering. Pinned by
  `transport/tests_parts/two_transport_loopback_requests.rs`,
  `two_transport_loopback_requests_isolation.rs`,
  `two_transport_loopback_requests_probe.rs`,
  `two_transport_loopback_requests_routing.rs`,
  `two_transport_loopback_requests_malformed.rs` and
  `destination/link_parts/link_sections/request_dispatch_tests.rs`.

- `LinkEvent::Data` with `PacketContext::Request` keeps posting for every
  inbound request; a request whose path has a registered handler is
  answered by that handler, and broadcast consumers must not answer it.
  Pinned by `transport/tests_parts/two_transport_loopback_requests.rs`.

- The queue between a link and the dispatch worker holds 1024 requests. A
  full queue drops the request with a debug log and never blocks the link.
  Python has no bound here, so this is a resource limit rather than a
  protocol divergence. Pinned by
  `transport/tests_parts/two_transport_loopback_requests_isolation.rs`.

- A request that arrives as a request Resource (`RNS/Link.py:503-510`) is
  dispatched to the registered handler once it is assembled
  (`RNS/Link.py:885-889`). The request id is recomputed as
  `truncated_hash` over the assembled bytes, never read from the
  advertisement's `q`, and equals the `resource_request_id` that
  `Link::request_payload` reports to the requester. The
  `ResourceEventKind::Complete(ResourceComplete { is_request: true, .. })`
  event keeps posting on `Transport::resource_events()` for every such
  request; as for `LinkEvent::Data(Request)`, a request whose path has a
  registered handler is answered by that handler, and event consumers must
  not answer it. Pinned by
  `transport/tests_parts/two_transport_loopback_requests_resource_form.rs`,
  `two_transport_loopback_requests_resource_form_probe.rs`,
  `two_transport_loopback_requests_resource_form_probe2.rs` and
  `two_transport_loopback_requests_resource_form_probe3.rs`.

- Response forms on the wire. A `Value` or `Bytes` response whose
  `[request_id, body]` envelope fits the link MDU goes out as one
  `Response` packet, and otherwise as a response Resource whose data is
  that whole envelope (`RNS/Link.py:849-852`). A `Resource { data,
  metadata }` response always goes out as a response Resource whose data
  is the raw `data`, with the packed metadata ahead of it under the `x`
  flag (`RNS/Link.py:839-846`, `RNS/Resource.py:261-269`). Either
  Resource advertises `q == request_id` with the response flag `p` set
  (`RNS/Resource.py:1305-1318`). A response packet is never compressed;
  a response Resource honours the registration's `auto_compress`. A
  Python requester reads a response Resource as a file response only when
  it carries metadata (`RNS/Link.py:902-912`), so a
  `Resource { metadata: None }` answer is unpacked as an envelope there
  and is only for a peer that expects raw bytes. Pinned by
  `transport/tests_parts/two_transport_loopback_requests.rs`,
  `two_transport_loopback_requests_probe.rs`,
  `two_transport_loopback_requests_resource_form.rs`,
  `two_transport_loopback_requests_resource_form_probe.rs`,
  `two_transport_loopback_requests_resource_form_probe2.rs` and
  `two_transport_loopback_requests_resource_form_probe3.rs`.

- `Transport::request` and `destination::request::{RequestOptions,
  RequestReceipt, RequestStatus, RequestError, RequestResponseData}`: the
  requester half of the link request layer, `RNS.Link.request`
  (`RNS/Link.py:473-510`) and `RNS.RequestReceipt` (`RNS/Link.py:1311-1500`)
  without callbacks. The request packs as `Link::request_payload` does and
  goes out as one `Request` packet when it fits `Link::link_mdu()`, as a
  request Resource otherwise (`:493-510`). `RequestReceipt` is `Clone`;
  `response()` resolves on any clone once the request is `Ready` or
  `Failed`, and `subscribe()` hands out a `watch::Receiver<RequestStatus>`
  that reports the moves after the call and whose sender is dropped once
  the request concludes or its link goes away unconcluded. Dropping every
  clone of a receipt cancels nothing: the responder still serves the
  request and the response still reaches the broadcasts. `started_at()` is
  `Some` from the send for a packet request and `None` for a request Resource until it
  completes or a response advertisement is admitted, whichever comes first
  (`:1331-1334`, `:1370-1371`, `:1062-1063`); `concluded_at()` is set once
  the receipt is `Ready` or `Failed`. `response_size()` and
  `response_transfer_size()` are set for a packet response too, to the
  packed body length where Python records `len(packb(body)) - 2`
  (`:1016-1017`); for a response Resource the first is `d` of the first
  advertisement, set once, and the second is `t` summed over the segments
  of a split response (`:1057-1061`). `progress()` is cumulative across
  those segments (`RNS/Resource.py:1145-1200`).
  A response whose id matches a pending receipt resolves it in-path, under
  the link lock, whether it arrives as a `Response` packet or as a response
  Resource; the `LinkEvent::Data(Response)` and `resource_events()`
  broadcasts still carry it. A response Resource with metadata is a file
  response, `RequestResponseData::data()` and `metadata()` (`:899-903`);
  every other response is the envelope body, `value()` (`:907-911`); the
  metadata is held to the same nesting depth as the frames before it is
  decoded. `RequestResponseData` is a cheap handle: clones share one body,
  as do the results of every `response()` call on a receipt.
  The default timeout is `rtt * 6 + 11.25 s` from `Link::rtt()`
  (`:491`); `RequestOptions::with_timeout` overrides it and
  `with_max_response_size` fails a larger response with
  `RequestError::Oversize`, measured on the packed body of a response
  packet and on the advertised data size `d` of a response Resource
  (`:874-876`, `:1048-1053`). The status order
  follows Python: a packet-form request goes `Sent -> Ready`, or
  `Sent -> Receiving -> Ready` for a Resource response, and never reaches
  `Delivered` (`:1421-1427`, `:1445-1450` mark only the packet receipt);
  a resource-form request reaches `Delivered` when its Resource completes
  (`:1372`), then `Receiving -> Ready` or `Ready`. The failures:
  `Timeout` once the request clock runs out; `Rejected` for a request
  Resource the responder refused; `Send` for a request Resource that could
  not be sent, exhausted its retries or was cancelled locally through
  `Transport::cancel_resource`, and for a receipt displaced by a second
  request under the same id; `Receive` for a response Resource that
  fails in transfer (a part failure, receiver retry exhaustion, a remote
  cancel); `Decode` for a response that is not an envelope, and for a
  response Resource without metadata whose envelope names another request
  than the advertisement that picked the receipt (`:908-912` keys
  `handle_response` on the envelope's id); `Oversize` as above;
  `LinkClosed` for a link that closes from either side, for
  `Transport::stop`, and for a transport dropped with the receipt still
  pending, where `status()` and `concluded_at()` conclude the receipt
  `LinkClosed` on their own once its entry is gone, without a prior
  `response()`, and `response()` returns the same. The receipt sweep in
  `Transport::stop` shares `stop`'s 5 s budget: a link whose lock is still
  held when it runs out is skipped with a warning, the other links are
  still swept, and the skipped link's receipts conclude `LinkClosed` once
  the link is dropped. A request on a link that is not `Active` is refused
  with `RnsError::ConnectionError`, `Stale` included, where Python drops a
  packet only on a `CLOSED` one (`RNS/Packet.py:294-295`). Pinned by
  `transport/tests_parts/two_transport_loopback_requests_receipt.rs`,
  `two_transport_loopback_requests_receipt_timeouts.rs`,
  `two_transport_loopback_requests_receipt_probe.rs`,
  `two_transport_loopback_requests_receipt_probe2.rs`,
  `two_transport_loopback_requests_receipt_probe3.rs`,
  `two_transport_loopback_requests_receipt_probe4.rs`,
  `two_transport_loopback_requests_receipt_probe5.rs`,
  `two_transport_loopback_requests_receipt_probe6.rs`,
  `two_transport_loopback_requests_receipt_probe7.rs`,
  `two_transport_loopback_requests_resource_form_sizes_probe.rs`,
  `two_transport_loopback_requests_receipt_stop.rs`,
  `two_transport_loopback_requests_errors.rs`,
  `tests/request_receipt_api.rs`, `tests/request_receipt_readme.rs`,
  `destination/request/receipt_tests.rs`
  and `destination/link_parts/link_sections/pending_requests_tests.rs`.

- `SendPacketOutcome::is_sent`: whether a send reached an interface,
  directly or by broadcast; the in-crate `matches!` twins use it.

### Changed

- The request timeout runs from the send for a packet request and from
  `Delivered` for a request Resource (`RNS/Link.py:1331-1334`,
  `:1370-1373`); no clock runs while the request Resource is in flight, so
  its own outcome governs there, `Rejected` or `Send`. The link watchdog
  expires the clock, and it never fires while a response Resource is being
  received (`:1389`): the Resource's own failure fails the receipt
  `Receive`, and a receipt that fails for another reason while a response
  Resource is inbound cancels that Resource (`:1435`), with the receiver
  cancel packet sent only on an `Active` link (`RNS/Resource.py:1111-1116`).
  A request Resource cancelled locally through `Transport::cancel_resource`
  fails its receipt `Send` (`RNS/Link.py:1377-1385` fails the receipt for
  every Resource status but complete); a response already inbound for that
  receipt is cancelled like any other response of a failed receipt.
  A receipt moves to `Receiving` only for a response advertisement the
  ResourceManager admitted and built a receiver for (`RNS/Link.py:1055-1064`
  touches the receipt only under `if response_resource != None`), and only
  when that receiver was built for the receipt's request id on the same
  link, so a re-advertisement of an active segment under another id marks
  nothing; an advertisement the manager drops, for the inbound caps, an
  out-of-order or duplicate segment or an unreasonable part count, leaves
  the receipt as it was with its clock running, and a repeat advertisement
  of the segment in flight (`RNS/Resource.py:594-598`) adds nothing to
  `response_transfer_size()` (`RNS/Resource.py:238-240`). Two faces of one
  divergence from Python's `DELIVERED` guard (`RNS/Link.py:1398`, `:1408`)
  remain: a receipt that never gets any response, a packet-form receipt
  or a resource-form receipt in `Delivered`, fails `Timeout`, where Python
  leaves a packet-form receipt pending forever; and a packet-form receipt
  whose response packet is over `max_response_size` fails `Oversize`
  (`:874-876`, `:1017`), where the `:1408` guard leaves it pending because
  a packet-form receipt is never `DELIVERED`. An over-limit response
  packet for a receipt already receiving a response Resource (the limit
  stays in force while the receipt is `Receiving`) fails it `Oversize`
  the same way and cancels that Resource with a receiver cancel to the
  responder, as a failed receipt cancels its response Resource
  (`RNS/Link.py:1435`). An in-limit response packet for a receipt already
  receiving a response Resource resolves the receipt `Ready` from the
  packet and leaves that Resource to its receiver's retry watchdog, as
  Python resolves on the packet alone (`RNS/Link.py:873`, `:1437-1443`).
  Pinned by
  `transport/tests_parts/two_transport_loopback_requests_receipt_timeouts.rs`,
  `two_transport_loopback_requests_receipt_probe.rs`,
  `two_transport_loopback_requests_receipt_probe2.rs`,
  `two_transport_loopback_requests_receipt_probe4.rs`,
  `two_transport_loopback_requests_resource_form_sizes_cancel.rs`,
  `two_transport_loopback_requests_resource_form_sizes_cancel_probe.rs`,
  `destination/request/receipt_tests.rs`,
  `destination/link_parts/link_sections/pending_requests_tests.rs`,
  `resource/tests_timeouts_lifecycle.rs`, and the `*_with_mtu` fixture
  constructors of `two_transport_loopback_lifecycle.rs` and
  `two_transport_loopback_requests.rs` that run a pair over `LEGACY_MTU`.

- The `max_request_size` gate on a request Resource, applied at its
  advertisement before any part is accepted, compares the advertised data
  size `d` (`ResourceAdvertisement.read_size`, `RNS/Resource.py:1285-1287`,
  `RNS/Link.py:1036-1046`) instead of the compressed transfer size `t`, so
  a compressible request over the limit is rejected as Python rejects it.
  The limit is inclusive (`d <= max_request_size` is admitted), the
  requester sees the rejection as `ResourceEventKind::OutboundRejected`,
  and the link goes on carrying request Resources afterwards. Pinned by
  `two_transport_loopback_requests_resource_form.rs`,
  `two_transport_loopback_requests_resource_form_probe.rs` and
  `two_transport_loopback_resources_reject.rs`.

- The `max_response_size` gate on a response Resource reads the advertised
  data size `d` (`RNS/Link.py:1049`, `RNS/Resource.py:1285-1287`) instead
  of the compressed transfer size `t`, so a compressible response over the
  limit is refused however small its transfer, and reads it at every
  segment advertisement (`d` is the whole response on each one,
  `RNS/Resource.py:297`): the limit is no longer spent by the first
  advertisement, so one the ResourceManager drops (inbound caps, an
  out-of-order or duplicate segment) leaves it in force for the next, and
  it goes when the response resolves, fails or is refused. A response
  is solicited by every request packet the link sends, however it was
  built (`Transport::request`, `Link::request_packet` or
  `request_packet_with_max_response_size`), and by every Resource sent
  through `send_request_resource*`; a response advertisement whose `q`
  names a request nothing on the link sent is dropped before it reaches
  the ResourceManager, with a debug log and no reject packet, as Python's
  pending-request loop (`RNS/Link.py:1046-1047`) falls through it. The
  transport records the id of each request packet as it goes out, the
  `send_request_resource*` record is dropped when the advertisement is
  not sent, and `Link::expect_response` and `Link::response_size_allowed`
  carry this beside the existing `set_response_size_limit` and
  `take_response_limit_if_allowed`, which keep their signatures (the
  send-time record, `Link::note_request_sent`, is crate-private); the
  packet-response path stays one-shot. The send-time record of a request
  packet, and the `send_request_resource*` record, follow a receipt's
  lifetime rules when no receipt owns the id: the clock runs from the send
  for a request packet and from the request Resource's delivery
  (`OutboundComplete`, `RNS/Link.py:1370-1373`) for a request Resource,
  stops while a response Resource is inbound, and that Resource's own
  outcome clears the record; otherwise the link watchdog prunes it at the
  default request timeout (`rtt * 6 + 11.25 s`, `RNS/Link.py:491`).
  Python's marker is the `RequestReceipt`: one `DELIVERED` past its
  timeout leaves `pending_requests` (`RNS/Link.py:1397-1405`), one
  `RECEIVING` is never timed out (`:1389`), and one that never reached
  `DELIVERED` stays, so a packet-form marker there can outlive this
  record. A receipt's record goes with the receipt. The
  assembled response Resource is measured against the record's limit once
  more when it completes, metadata excluded, for every id form (one a
  receipt owns, a receipt-less 16-byte one from `request_packet*` or
  `send_request_resource*`, and a legacy-length one), since the advertised
  `d` is the peer's claim and an uncompressed response can carry up to `t`
  bytes; Python trusts the advertisement (`RNS/Link.py:1049`). One over
  the limit is logged, fails the receipt `Oversize` when one owns the id,
  clears the record, and its `Complete` is withheld from
  `resource_events()` subscribers.
  Pinned by
  `transport/tests_parts/two_transport_loopback_requests_resource_form_sizes.rs`
  (`..._sizes_bare_request_packet_solicits_its_response` among them),
  `two_transport_loopback_requests_resource_form_sizes_probe.rs`,
  `two_transport_loopback_requests_resource_form_sizes_probe2.rs`,
  `two_transport_loopback_requests_resource_form_sizes_probe3.rs`,
  `two_transport_loopback_requests_resource_form_sizes_probe4.rs` (the
  receipt-less record's lifetime as a slow responder sees it),
  `two_transport_loopback_requests_resource_form_sizes_probe5.rs` (a
  lying `d` fails the receipt `Oversize` at the recheck, a completed or
  remotely cancelled response ends a receipt-less record, a bare
  `request_packet` record ends on completion, and the legacy-id limit is
  inclusive at both measurements),
  `two_transport_loopback_resources.rs`,
  `two_transport_loopback_resources_directions.rs`,
  `two_transport_loopback_resources_retries.rs`,
  `two_transport_loopback_resources_receipt_less_receiving.rs` (a
  two-segment response under a receipt-less id, stalled past the default
  request timeout, still completes),
  `two_transport_loopback_resources_reject_response.rs` (whose bare
  response fixtures now send the soliciting request Resource first, and
  whose reject test sends a second response under the rejected id and
  sees it dropped without a reject) and
  `destination/link_parts/link_sections/pending_requests_tests.rs`.

- `Transport::send_direct`, `send_response_resource_with_compression` and
  the transport's link lookup delegate to free functions in
  `transport/request_dispatch.rs`, which the dispatch worker shares. No
  behaviour change. The `test_bridge` module moved from `transport/mod.rs`
  into `transport/test_bridge.rs`, unchanged. Pinned by
  `transport/tests_parts/two_transport_loopback_resources.rs` and
  `two_transport_loopback_resources_directions.rs`.

- `Link::request_payload` and `unpack_response_envelope` delegate to the new
  frames. The bytes are unchanged. The response decoder now refuses a
  request id that is not a 16-byte `bin` (a 16-byte `str` was accepted
  before, as `rmpv::Value::as_slice` reads both), nesting deeper than 128
  levels, the reserved marker, and a length prefix larger than the
  remaining input before anything is allocated. Pinned by
  `destination/request/tests.rs`, `tests/request_frame_wire.rs` and
  `request_payload_tests.rs`.

## 0.14.1 - 2026-10-15

### Added

- `Link::rtt()` returns the link round-trip time: measured from the link
  request to the proof on the initiator (`RNS/Link.py:419`); on the
  responder the larger of its own measurement and the value in the
  initiator's RTT packet (`RNS/Link.py:518-522`); zero until then. Pinned by
  `transport/tests_parts/two_transport_loopback_lifecycle.rs`.

- `Transport::stop()` detaches every interface and waits for their workers,
  then cancels the transport and waits for the management task, up to a
  5 s budget after which whatever is still running is aborted and logged;
  a second call is a no-op. `Drop` still only cancels. Pinned by
  `two_transport_loopback_lifecycle.rs` and
  `two_transport_loopback_lifecycle_probe.rs`.

- `Transport::try_send_announce()` returns the announce build error to the
  caller instead of logging it. Pinned by the two lifecycle test files above.

- `rmpv` and `rand_core` are re-exported from the crate root so callers can
  name the exact versions the public API uses; the integration test
  `tests/crate_root_reexports.rs` exercises both through the crate root.

- `destination_hash::destination_address()` computes a destination address
  from a name hash and an identity hash (`RNS/Destination.py:116-130`).
  Pinned against `SingleInputDestination::new` by a unit test and against
  independently computed vectors by `tests/destination_address_kat.rs`.

- `AddressHash::short()` returns the first eight hex chars for log lines.
  Pinned by `short_is_first_eight_hex_chars` in `hash.rs`.

### Changed

- The crate version is `0.14.1`; the discovery announce carries it in the
  `TRANSPORT_VERS` field (`discovery/announce.rs`), so peers observe the fork
  patch release through discovery.

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
  now rejected too. Pinned by the parser tests in `hash.rs` and
  `identity/tests.rs`.

- `Transport::send_announce` panicked when the announce could not be built
  (app data too large for the packet buffer). It now logs at warn and
  returns. Pinned by `two_transport_loopback_lifecycle.rs`.

- An inbound Resource advertisement with a transfer size above
  `3 * MAX_EFFICIENT_SIZE` is dropped and logged at debug before any
  receiver state is created, matching the decode check in
  `RNS/Resource.py:1374`. Split Resources advertise each segment with its
  own size, so larger totals are unaffected. Pinned by
  `resource/tests_advert_limits.rs` and the over-cap probe in
  `two_transport_loopback_lifecycle.rs`.

- Multi-segment Resource advertisements and Resource retries were routed
  through `route_outbound_packet`, which never resolves a link id, so
  segments 2 and later of any Resource larger than `MAX_EFFICIENT_SIZE`
  were dropped as `DroppedNoRoute` unless `broadcast` was on. They are now
  sent on the link's bound interface like the first segment
  (`handle_resource_proof` and the resource retry worker), matching
  `RNS/Transport.py:1452-1454`; retry dispatch now resolves initiator-side
  links by link id as well (`out_links` is keyed by destination hash).
  Pinned, on the in-crate `two_transport_loopback` fixture
  (`transport/tests_parts/two_transport_loopback.rs`: two `Transport`s over a
  real TCP pair on 127.0.0.1, broadcast off), by `two_transport_loopback_resources.rs`,
  `two_transport_loopback_resources_directions.rs`,
  `two_transport_loopback_resources_retries.rs`,
  `two_transport_loopback_resources_gone_link.rs` and
  `resource_retry_link_lookup.rs`; proven with Python on either end by two
  ignored `reticulumd` interop tests
  (`python_channel_interop_parts/multi_segment_python_interop.rs`) that build
  the Rust `Transport` with broadcast off and move a 4 MiB + 1 byte
  (five-segment) response Resource in each direction, comparing SHA-256
  digests on both ends; the Python endpoint gains the
  `multi-segment-response` payload kind for them.

- `handle_link_resource_packet` sent a Resource advertisement reject
  (`ResourceReceiverCancel`) while still holding the link's mutex guard;
  `TransportHandler::send` re-locks that link in `note_link_packet_sent`,
  so any oversize request or response advertisement deadlocked the
  transport. The reject is now built under the guard and sent after it is
  dropped; the rejected advertisement still never reaches the
  `ResourceManager`. Pinned by
  `two_transport_loopback_resources_reject.rs` and
  `two_transport_loopback_resources_reject_response.rs`.

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
  `ResourceInitiatorCancel` packet and emits `OutboundCancelled`. Pinned by
  `resource/tests_split_unconfirmed.rs` and
  `resource/tests_split_unconfirmed_served.rs`; `tests_split_lazy.rs`,
  `tests_timeouts.rs` and `tests_timeouts_cleanup.rs` follow the sender into
  `outgoing`.

## 0.14.0 - 2026-10-14

Initial fork release, based on upstream `reticulum-rs-transport` 0.14.0.

### Changed

- Package renamed to `coyote-rns-transport`; the library target stays
  `rns_transport`.

### Removed

- The `println!` on link close in `Link::finalize_local_close`; the
  `log::warn!` on the next line carries the information. See
  [docs/fork/PATCHSET.md](https://github.com/Dark-Alex-17/coyote-rns-transport/blob/main/docs/fork/PATCHSET.md).
