# Changelog

The section layout follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and versions follow the cargo semver rules described in the crate README.
Test-only files are not listed as entries of their own; each is named in a
trailing clause of the bullet for the behaviour it pins. A fixture hardening
that changes no library behaviour is the one exception: it is a Changed
bullet of its own, marked test-only.

## Unreleased

### Added

- `transport::{SharedInstanceEndpoint, SharedInstanceOptions}` and
  `Transport::{attach_shared_instance, detach_shared_instance,
  shared_instance, connected_to_shared_instance}`: the client posture of a
  Python program attaching to the instance that owns `shared_instance_port`
  (`RNS/Reticulum.py:422-441`; the port is `:295`, `:491-493`).
  `attach_shared_instance` spawns a `TcpClient`, or on unix targets a
  `LocalUnixClient`, at the endpoint, marks that interface as the shared
  instance, sets the connected flag and synthesizes a tunnel on it at
  attach (and again on every reconnect, see Changed), so inbound packets
  take the client-side hop (`RNS/Transport.py:1940`) and
  outbound packets on one-hop paths carry a transport header
  (`:1416-1427`); it returns the interface address. One shared instance is
  held at a time: a second call returns `RnsError::InvalidArgument` until
  `detach_shared_instance`, which stops the interface and clears the flag,
  returning `false` when none was attached.
  `SharedInstanceEndpoint::{Tcp, Unix}` is `#[non_exhaustive]`, with `Unix`
  present on unix targets only; `SharedInstanceOptions` has private fields
  and a `with_reconnect_wait` builder, default 8 s
  (`RNS/Interfaces/LocalInterface.py:54`), honoured by both clients.
  Pinned by `transport/tests_parts/two_transport_shared_instance.rs`: two
  clients on one TCP shared instance learn each other at one hop and open
  links both ways, the shared instance's tunnel table carries a path for
  each attached client, a second attach is refused, detach and stop release
  the interface, and a unix attach registers its client before the socket
  exists and connects once a listener binds, while retrying at the
  configured reconnect wait. By
  `transport/tests_parts/two_transport_shared_instance_probe.rs`: the
  option default and builder, detach on a never-attached transport,
  concurrent attaches admit one, an attach before the server listens
  connects once it does, detach silences the relay while re-attach restores
  one-hop paths, and a refused attach racing a detach leaves the slot and
  the posture in agreement. By
  `transport/tests_parts/two_transport_shared_instance_probe2.rs`: the unix
  flavour end to end, a TCP client retries at a short reconnect wait, stop
  with a wedged handler still forgets the slot within its budget, and
  `stop_interface` leaves the slot held until detach. And by
  `transport/tests_parts/two_transport_shared_instance_probe3.rs`: a detach
  queued before the attach's posture write wins the slot and the posture
  stays off, an attach that starts once stop has cancelled is refused and
  spawns nothing, and attach/detach cycles leave no interface behind. And by
  `transport/tests_parts/two_transport_shared_instance_probe4.rs`:
  `remove_interface` leaves the slot held until detach, and a second stop
  detaches an interface spawned inside the first stop's window. The
  `non_exhaustive` endpoint, the private option fields and the README
  claims are pinned by
  `crates/apps/reticulumd/tests/fork_release_docs_contract.rs`; the release
  notes and the ledger section are held to
  `crates/apps/reticulumd/tests/fork_publish_contract.rs` (the Unreleased
  cite check and the Unreleased ledger checks).
- `TcpClient::{with_reconnect_wait, reconnect_wait}` and
  `TcpClient::DEFAULT_RECONNECT_WAIT`, 5 s: the wait slept before each
  retry once a connect has failed or an established stream has dropped
  twice in a row, the knob `LocalUnixClient::with_reconnect_wait` already
  offered (`RNS/Interfaces/LocalInterface.py:54`, slept at `:160`).
  `attach_shared_instance` now passes `SharedInstanceOptions::reconnect_wait`
  to the `TcpClient` as it did to the `LocalUnixClient`, and its attach
  debug line reports the wait as in force for both endpoints. Both knobs
  clamp the wait to at least 1 ms; `LocalUnixClient::with_reconnect_wait`
  previously took a zero wait as given. The 5 s default is pinned by
  `transport/tests_parts/two_transport_shared_instance_probe7.rs`
  (`shared_instance_probe7_tcp_client_reconnect_wait_defaults_to_five_seconds`).
  `TcpClient::with_disconnect_events` and
  `LocalUnixClient::with_disconnect_events`, with
  `TcpClient::disconnect_events_enabled`: a sender, sibling of
  `with_reconnect_events`, that receives the interface address each time
  an established stream ends while the client is neither cancelled nor
  stopped, before it reconnects (`RNS/Interfaces/LocalInterface.py:255-261`);
  connect failures and detach or stop send nothing, and `reconnect_events`
  is unchanged. Pinned by `iface/tcp_client.rs` (a 50 ms wait makes a
  reconnect storm back off for 50 ms rather than 5 s; a dropped stream
  reports one disconnect event then the reconnect event, and a stop while
  connected reports none), `iface/local.rs` (the same disconnect twin for
  the unix client),
  `transport/tests_parts/two_transport_shared_instance_probe2.rs` (a TCP
  client attached with a 50 ms wait reaches a late-bound shared instance
  within 2 s),
  `transport/tests_parts/two_transport_shared_instance_probe5.rs` (the tcp
  attach line reports the wait in force and never says ignored) and
  `transport/tests_parts/two_transport_shared_instance_probe6.rs` (the tcp
  attach line is three key=value tokens ending at the wait).
- `SingleInputDestination::announce_with_context`, the announce of
  `announce` with the packet context set to `PathResponse` when asked, as
  Python's `Destination.announce(path_response=True)` does
  (`RNS/Destination.py:308-315`), and
  `Transport::announce_destinations_to_shared_instance`, which announces
  every registered single destination as a path response
  with its registered app data and returns how many announces were handed
  to the transport. While attached to a shared instance, a `SINGLE`
  destination is announced that way 0.25 s after `register_destination`,
  as Python's client does from a thread (`RNS/Transport.py:2911-2916`);
  an unattached transport announces nothing on registration. Pinned by
  `transport/tests_parts/two_transport_shared_instance_parity.rs` (a
  registered destination is announced as a path response with no announce
  from the test, an unattached transport announces nothing on
  registration, the path-response announce differs from a plain one only
  in its context, and the re-announce sends every destination with its
  app data and reports the count) and by
  `transport/tests_parts/two_transport_shared_instance_probe7.rs` (the
  registration announce waits its 0.25 s rather than going out at once).

### Changed

- On every shared-instance reconnect the attached client re-synthesizes
  the tunnel on the interface at once and, `reconnect_wait` plus 2 s
  later, re-announces every registered single destination as a path
  response if the transport is still attached then
  (`RNS/Transport.py:3705-3709`, `RNS/Interfaces/LocalInterface.py:178-181`).
  A dropped connection while attached, with the interface still flagged as
  the shared instance, closes every in and out link with its own side's
  `LinkCloseReason`, drops their resource state and clears the path,
  announce, link and tunnel tables, the held announces and the packet
  cache's reverse routes, Python's `reverse_table`, through
  `PacketCache::clear_reverse_routes` (`transport/packet_cache.rs`), which
  leaves the duplicate-filter hashes in place
  (`RNS/Transport.py:3690-3702`, `RNS/Interfaces/LocalInterface.py:255-261`);
  a detach, a stop, a dropped connected posture or an interface no longer
  flagged as the shared instance flushes nothing. The links are taken out
  of both maps before they are closed, so a link an application opens on
  the `Closed` event is not evicted by the clears. Both are served by one
  task per attach that reads the clients' reconnect and disconnect events,
  serving a queued disconnect before the reconnect behind it. The warning
  for a re-announce whose packet fails to build names the transport as
  `tp(<name>)`, like the registration announce's. Pinned by
  `transport/tests_parts/two_transport_shared_instance_reconnect.rs` (a
  restart of the shared instance flushes the path, closes the link with
  `InitiatorClosed`, keeps the attach and re-announces both destinations
  as path responses within the bound into the new instance's tunnel table;
  a detach, a dropped posture and an unflagged interface each keep the
  path and the link across the restart; the consumer serves a queued
  disconnect before the reconnect behind it; and each table's `clear`
  empties every collection it holds and reports the count), by
  `transport/tests_parts/two_transport_shared_instance_flush.rs` (before
  the drop the path, announce, link and tunnel tables, the held announces
  and the reverse route of a Data packet the shared instance sent over a
  link each hold something, after it each is empty and the cached hashes
  are kept) and by
  `transport/tests_parts/two_transport_shared_instance_probe7.rs` (the
  re-announce fires no sooner than `reconnect_wait` plus 2 s after the
  re-accept, a posture dropped after the reconnect re-announces nothing,
  every further restart re-announces again, and a pending link is torn
  down by the drop like an active one).
- `filter_duplicate_packet` (`transport/inbound_processing.rs`) returns
  early while attached to a shared instance, so nothing is inserted into
  the local duplicate-filter hashlist and nothing is rolled back: Python's
  `add_packet_hash` skips the hashlist and `packet_filter` accepts every
  packet in that mode (`RNS/Transport.py:1618-1627`), the owner
  deduplicates. The packet cache's other halves are still written while
  attached: the reverse-routing note `handle_data` leaves for each received
  Data packet (Python's `reverse_table`) and the hash of each packet sent,
  the receipt guard. Pinned by `transport/inbound_processing_tests.rs`
  (`shared_instance_client_leaves_the_packet_cache_untouched`,
  `shared_instance_client_accepts_the_same_packet_twice`).
- `Transport::stop` forgets an attached shared instance and clears the
  connected flag, since its interface is detached with the rest; a stopped
  transport reports `shared_instance()` as `None`. The posture is now
  cleared after the management task has stopped, within stop's 5 s budget;
  when the handler stays locked past it the slot is still forgotten and a
  warning naming the interface says the posture was not cleared; the
  warning is emitted only when a slot was actually held. A slot that stop
  does clear is logged at debug under the links target, naming the
  interface. `attach_shared_instance` after `stop()` has cancelled the
  transport returns `RnsError::InvalidArgument` and spawns nothing.
  `attach_shared_instance` sets the posture only while the slot still holds
  the new interface, so a concurrent detach or stop cannot leave the
  posture set with the slot empty. An interface spawned after stop's sweep
  but before stop cancels the transport stays registered with the slot
  empty; a second stop detaches it. `stop` also joins the tasks the
  shared-instance client spawns (each attach's reconnect consumer, the
  delayed registration announces and the delayed re-announces, which
  Python runs as threads it never joins) within its budget, right after
  the cancel, aborting and warning only on expiry, so none of them
  announces on a stopped transport; the set they live in is reaped of
  finished tasks before every spawn. Pinned by
  `transport/tests_parts/two_transport_shared_instance.rs` (stop releases
  the interface and the posture) and
  `transport/tests_parts/two_transport_shared_instance_probe.rs` (detach
  after stop reports `false`, a refused attach racing a detach keeps the
  slot and the posture consistent) and
  `transport/tests_parts/two_transport_shared_instance_probe3.rs` (a detach
  queued before the attach's posture write wins the slot and the posture
  stays off, an attach that starts once stop has cancelled is refused and
  spawns nothing) and
  `transport/tests_parts/two_transport_shared_instance_probe4.rs` (a second
  stop detaches an interface spawned inside the first stop's window, the
  stop parked on a held worker so the attach lands in it for certain) and
  `transport/tests_parts/two_transport_shared_instance_probe5.rs`, through
  the record store in
  `transport/tests_parts/two_transport_loopback_requests_isolation.rs` (the
  attach line never warns, both the tcp and the unix line report the
  reconnect wait in force, a stale-slot detach and an attach whose
  interface vanished between its sections log at debug) and
  `transport/tests_parts/two_transport_shared_instance_probe6.rs` (the
  attach line is space-separated key=value tokens and both arms end at the
  wait) and `transport/tests_parts/two_transport_shared_instance_stop.rs`
  (`shared_instance_stop_refuses_a_later_attach_and_spawns_nothing`,
  `shared_instance_stop_logs_the_slot_it_clears_at_debug`) and
  `transport/tests_parts/two_transport_shared_instance_tasks.rs` (a stop
  inside the registration delay joins the announce task, which sends
  nothing, and a stop after an attach joins the consumer; the set is empty
  after both).
- The `TestLogger` in
  `transport/tests_parts/two_transport_loopback_requests_isolation.rs`
  keeps every record at `Warn` or above on the links target
  unconditionally; Debug, Info and Trace records still need the "shared
  instance" needle. `shared_instance_assert_never_warned` in
  `transport/tests_parts/two_transport_shared_instance_probe5.rs` so
  covers any warning naming the transport, not only the shared-instance
  lines; `shared_instance_test_logger_keeps_a_needle_less_warning` in
  `transport/tests_parts/two_transport_shared_instance_tasks.rs` pins that
  a needle-less warning is stored and a needle-less debug record is not.
  The reconnect-order test in
  `transport/tests_parts/two_transport_shared_instance_reconnect.rs` runs
  twenty rounds, each on its own transport name, channels and consumer,
  since an unbiased select would pass a single round by luck. Test-only;
  no library behaviour changes. Pinned by those same files.
- The proof-cap fixture
  `transport/tests_parts/two_transport_loopback_link_packet_proven_probe2.rs`
  holds its handler stall until the test releases it after the burst
  instead of for a fixed 1500 ms, so the proven set no longer depends on
  runner speed. Test-only; no library behaviour changes. Pinned by that
  same file.

## 0.15.0 - 2026-10-20

### Breaking

- `LinkEvent` is `#[non_exhaustive]` and gains `PacketProven { packet_hash }`.
  Migration: an out-of-crate exhaustive match adds a `PacketProven` arm or a
  wildcard arm; see Changed.

- `ResourceEventKind` is `#[non_exhaustive]` and `OutboundRejected` is now
  the struct variant `OutboundRejected { reason: RejectReason }`. Migration:
  write `OutboundRejected { .. }` or `OutboundRejected { reason }` in
  patterns, and add a wildcard arm; see Changed.

- `RnsError` is `#[non_exhaustive]` and gains `NoPath`. Migration: add a
  wildcard arm; see Changed.

- `Transport::link` returns `Result<Arc<Mutex<Link>>, RnsError>`,
  `Err(RnsError::NoPath)` when no next hop is known and `broadcast` is off.
  Migration: use `?` or match the error; discover the destination first with
  `Transport::request_path` and `Transport::await_path`; see Changed.

- `AnnounceEvent` is `#[non_exhaustive]` and gains the public field
  `is_path_response`. Migration: a struct literal outside the crate no longer
  compiles; destructure with `..`; see Changed.

- `DeliveryReceipt` has a private `link_id` field, so it is built with
  `DeliveryReceipt::new(message_id)` and no longer with a struct literal;
  `PartialEq` and `Eq` compare `link_id` too. Migration: compare hashes with
  `matches_packet_hash`; see Changed.

- `delivery::send_via_link` returns an `io::Error` immediately when the link
  request cannot be sent for lack of a path, instead of waiting out link
  activation. Migration: expect the error without the activation delay, and
  discover the destination first with `Transport::request_path` and
  `Transport::await_path` as for `Transport::link`; see Changed.

The shapes these migrations replace are pinned by
`tests/breaking_changes_compile_fail.rs`, which drives `rustc` against the
crate's rlib: each replaced shape fails to compile outside the crate, with
rustc's diagnostic for the reason its bullet gives, and each migrated shape
compiles.

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
  `resource/tests_advert_kat.rs`. `FrameDecodeError` is
  `#[non_exhaustive]`.

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
  file-response forms (`RNS/Link.py:837-852`). `RequestAllow` and
  `RequestResponse` are `#[non_exhaustive]`. The registry is pinned by
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
  `destination/link_parts/link_sections/request_dispatch_tests.rs`, and
  with Python RNS 1.5.2 on the requesting side by the reticulumd tests in
  `python_channel_interop_parts/python_to_rust_request_handler.rs`
  (ALLOW_ALL in packet form, ALLOW_LIST answering the identified peer and
  silently dropping an unlisted one, the request id compared as hex on
  both sides) and
  `python_channel_interop_parts/python_request_forms_matrix.rs` (every
  request and response wire-form cell above the 16 KiB TCP link MDU in
  both directions, the packet-for-packet Python-to-Rust cell living in
  the sibling fixture above, the form pinned through
  `Transport::resource_events()`; a Resource request from an unlisted
  peer is accepted on size alone and dropped before the handler, as
  Python does, `RNS/Link.py:1036-1040`) and
  `python_channel_interop_parts/python_request_edges.rs` (a packet-form
  request over `max_request_size` ignored before any handler runs,
  nothing answered, the Python receipt left `SENT`, and a fresh client
  still served). The Python endpoint gains the
  `request-forms` payload kind, `--request-size`, `--identify`,
  `--request-timeout`, `--request-path` and `--prove-all` for them
  (`python_channel_endpoint.py`, `python_channel_request_forms.py`,
  spawned and read on the Rust side by `python_channel_events.rs` and
  `python_channel_process.rs`); its request-forms handler also takes a
  `delay` key that holds the answer back that many seconds (reporting
  `request_received` before the sleep and `request_answered` after it),
  `response: map` and `response: file` keys for a dict and a
  `(BufferedReader, metadata)` reply, and a `teardown_after` key that
  closes the link from the Python side while the request is pending; its
  client reports the wire form it used.
  `python_channel_interop_parts/python_request_unknown_path.rs` pins a
  request for a path no handler is registered on, in both wire forms on
  either responder: dropped silently on the Rust responder (the packet
  cell going on to show a second client still served; the Resource form
  accepted on size alone first, then dropped on the path-hash miss, so
  the Python requester fails from `DELIVERED`, `RNS/Link.py:1372`,
  `:1398`), and on the Python responder left unanswered
  (`RNS/Link.py:810`, the request Resource accepted on size alone first,
  `:1036-1040`).

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
  `two_transport_loopback_requests_resource_form_probe2.rs`,
  `two_transport_loopback_requests_resource_form_probe3.rs` and the
  reticulumd tests
  `python_to_rust_request_handler_resource_request_over_mdu_gets_a_resource_response`
  and `python_to_rust_request_handler_resource_request_over_mdu_gets_a_packet_response`.

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
  request Resource otherwise (`:493-510`). `RequestStatus` and `RequestError`
  are `#[non_exhaustive]`. `RequestReceipt` is `Clone`;
  `response()` resolves on any clone once the request is `Ready` or
  `Failed`, and `subscribe()` hands out a `watch::Receiver<RequestStatus>`
  that reports the moves after the call and whose sender is dropped once
  the request concludes or its link goes away unconcluded. Dropping every
  clone of a receipt cancels nothing: the responder still serves the
  request and the response still reaches the broadcasts, unless the
  receipt's `max_response_size` refuses it: an over-limit packet Response
  is not posted as `LinkEvent::Data`, and a response Resource that
  assembles over it has its `Complete` withheld. `started_at()` is
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
  still swept, a swept receipt's inbound response Resource is cancelled
  and reported `InboundFailed("cancelled")` once inside `stop`, after
  whatever the manager still held queued when `stop` ran, which the sweep
  publishes first in the order of the periodic drains and routes to no
  receipt (a receipt whose transfer was dropped before the stop concludes
  `LinkClosed` like every other receipt the sweep reaches), and the
  skipped link's receipts conclude `LinkClosed` once the link is dropped. A
  request on a link that is not `Active` is refused with
  `RnsError::ConnectionError`, `Stale` included, where Python drops a
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
  `two_transport_loopback_requests_resource_form_sizes_probe16.rs` (the
  stop sweep publishes a sibling link's queued `link_closed` failure
  before the `cancelled` failure it raises for the receipt it sweeps, each
  once, and both receipts conclude `LinkClosed`),
  `two_transport_loopback_requests_errors.rs`,
  `two_transport_loopback_requests_receipt_orders.rs` (the four status
  orders pinned exactly, the responder delayed so no change is coalesced
  on the watch; the `Delivered` presence pins of
  `two_transport_loopback_requests_receipt.rs`,
  `two_transport_loopback_requests_resource_form_sizes.rs` and
  `two_transport_loopback_requests_resource_form_sizes_probe15.rs`
  hold the responder until the subscriber has read `Delivered`),
  `tests/request_receipt_api.rs`, `tests/request_receipt_readme.rs`,
  `destination/request/receipt_tests.rs`,
  `destination/request/receipt_order_tests.rs`,
  `destination/link_parts/link_sections/pending_requests_tests.rs`,
  `destination/link_parts/link_sections/response_record_tests.rs` (a
  request Resource outcome never marks a packet-form entry `Delivered`)
  and the reticulumd
  `python_channel_interop_parts/rust_to_python_request_receipt.rs` (the
  packet form against a Python `register_request_handler`, and a `None`
  return concluding `Failed(Timeout)` because Python then sends nothing,
  `RNS/Link.py:844`) and `python_request_forms_matrix.rs` for the
  Resource forms (a request Resource with a Resource and with a packet
  response, a packet request with a Resource response) and two
  concurrent packet-form requests resolving separately, the request id
  compared as hex on both sides for every cell, and
  `python_request_unknown_path.rs` for a path Python has no handler on,
  `rust_to_python_request_receipt_unregistered_path_packet_request_times_out_undelivered`
  and `..._resource_request_is_delivered_then_times_out` (status trails
  `[Sent, Failed]` and `[Sent, Delivered, Failed]`, the Resource form's
  clock running from delivery, asserted as the Delivered-to-Failed
  interval covering the timeout), with
  `rust_to_python_request_receipt_resource_request_none_response_is_delivered_then_times_out`
  (`python_request_size_limits.rs`) as the `None`-reply twin of that
  trail, and `python_request_late_reply.rs` (a Python reply arriving
  after the Rust timeout, in packet or Resource form, revives nothing and
  the link keeps serving; a Resource request the Rust handler answers
  `None` fails on the Python side from `DELIVERED`, `RNS/Link.py:1372`,
  `:1398`), and `python_request_edges.rs` (a Python `Link.teardown`
  under a pending request concluding `LinkClosed` about a second in,
  well ahead of a 30 s timeout; a Python dict reply read as a
  `Value::Map` with umsgpack's type mapping and key order intact and
  re-packing to the bytes Python packed, as a packet and as a 48 KiB
  Resource; a Python file reply, always a Resource even far below the
  MDU, read through `data()` and `metadata()` with `value()` `None`).

- `SendPacketOutcome::is_sent`: whether a send reached an interface,
  directly or by broadcast; the in-crate `matches!` twins use it.

- `LinkEvent::PacketProven { packet_hash }`: posted on a link's event
  stream once per plain Data packet this side sent on the link, from
  either side of it, when the peer's proof of it arrives. Every plain Data
  packet (`PacketContext::None`) sent on a link has its hash recorded on
  the link as it is sent (Python records a `PacketReceipt` for every Data
  packet outside the link-control and Resource contexts, request and
  response packets included, which expire unproved, and Channel frames,
  which the peer proves, `RNS/Transport.py:1359-1377`; this side records
  plain Data only), and
  the first valid proof naming a recorded hash consumes it and posts the
  event, the way Python's proof removes the one `SENT` receipt it
  validates (`RNS/Transport.py:2748-2761`). The record keeps the latest
  1024 hashes, oldest dropped first, Python's `Transport.MAX_RECEIPTS`
  (`RNS/Transport.py:159`, `:745-748`) applied per link; Python also fails
  a link receipt after `max(rtt * 6, 5 ms)` (`RNS/Packet.py:428`), which
  the record does not, so a proof for a hash the cap dropped posts
  nothing. It never fires
  for a request, a response or a Resource packet, none of which is proved;
  nor for a Channel frame, which the peer proves (`RNS/Link.py:1147-1151`)
  but whose hash is never recorded, its proof going to the channel
  machinery; nor for a replayed proof or a hash this side never sent. A
  Python peer proves link packets only when its destination's proof
  strategy is `PROVE_ALL`, or `PROVE_APP` says yes (`RNS/Link.py:961-968`),
  and the default is `PROVE_NONE` (`RNS/Destination.py:161`); this crate
  always proves. It is not a request-delivery signal: a packet-form
  `RequestReceipt` never reports `Delivered`. Every validly signed link
  proof also reaches the transport `ReceiptHandler` as a `DeliveryReceipt`
  naming the link, a Channel-frame proof, one for a hash this side never
  sent, or a replayed proof the ingress packet filter lets through (one
  the bounded duplicate cache has forgotten) included: the receipt is not
  filtered by the record. Pinned by
  `transport/tests_parts/two_transport_loopback_link_packet_proven.rs`,
  `transport/tests_parts/two_transport_loopback_link_packet_proven_probe.rs`,
  `transport/tests_parts/two_transport_loopback_link_packet_proven_probe2.rs`
  (the cap and a Channel frame, through the transport send path; the
  cap burst's proofs and receipts are collected on progress, so a slow
  runner stretches the wait and only a stall fails it),
  `transport/tests_parts/two_transport_loopback_link_packet_proven_probe3.rs`
  (a replayed proof: dropped by the ingress packet filter it adds neither
  receipt nor event; delivered past the filter it adds a second receipt
  naming the link and no event, and a fresh packet still earns its one),
  `destination/link_parts/link_sections/data_proof_tests.rs`,
  `tests/link_event_packet_proven_api.rs` and the reticulumd tests
  `rust_to_python_plain_data_packet_is_proven_by_a_prove_all_destination`,
  `rust_to_python_plain_data_packet_is_not_proven_by_a_default_destination`
  (a Python destination on `PROVE_ALL` proves the plain Data packet; the
  stock `PROVE_NONE` default does not) and
  `rust_to_python_request_receipt_under_prove_all_only_the_plain_data_packet_is_proven`
  (a request packet on a `PROVE_ALL` link earns no proof, only plain Data
  does, `RNS/Link.py:950`, `:961`).

- `DeliveryReceipt::link_id()`: `Some(link id)` for a proof addressed to a
  link, `None` for a destination proof. Pinned by
  `transport/tests_parts/two_transport_loopback_link_packet_proven.rs`,
  `transport/tests_parts/two_transport_loopback_link_packet_proven_probe.rs`
  (the single-destination `None` case) and
  `tests/link_event_packet_proven_api.rs`.

- `AnnounceEvent::is_path_response`: true for an announce the destination
  sent in answer to a path request, which it files under
  `PacketContext::PathResponse` (`RNS/Destination.py:308`), false for an
  unsolicited one; the same flag Python hands its announce handlers
  (`RNS/Transport.py:2524-2528`). The flag repeats the context the
  announcing peer put on the packet and is not matched against any path
  request this transport sent; it informs, it does not check. Python's
  announce handler path drops a path response unless the handler sets
  `receive_path_responses` (`RNS/Transport.py:2500-2504`); here every
  announce reaches `Transport::recv_announces()` with the flag set or
  clear, and filtering is the consumer's. Pinned by
  `transport/tests_parts/two_transport_loopback_announce_path_response.rs`.

- `Transport::{send_resource_with_outcome, send_request_resource_with_outcome,
  send_response_resource_with_outcome}`: the existing `send_*` entry points
  plus an `OutboundOutcome`, returned as `(Hash, OutboundOutcome)`. The
  outcome is a future that resolves once, with the verdict the resource
  events report: `resource::OutboundResult::Complete` when the peer proves
  the final segment (a split resource resolves once, not per segment);
  `Failed(OutboundFailure::{NotSent, RetriesExhausted, Segmenting,
  LinkClosed})` when the advertisement never left, the retry budget ran
  out, a later segment could not be built or the link went away, where
  `NotSent` is the verdict of `track_prepared_with_outcome` followed by
  `confirm_outbound_dispatch(.., false)` and never reaches a caller of the
  three entry points, which return `Err(ConnectionError)` instead of the
  outcome in that case;
  `Rejected(RejectReason::PeerCancelled)` when the peer answers the
  advertisement, or a later segment of it, with a receiver cancel
  (`RNS/Resource.py:155-165`), which
  carries only the resource hash, so `RejectReason::{Oversize { limit },
  Unknown}` are reserved and no code path produces them in this release;
  and `Cancelled` after `cancel_resource`. A transport dropped before a
  verdict resolves `Failed(LinkClosed)` rather than hanging, and
  `Transport::stop()` concludes every outbound transfer `Failed(LinkClosed)`
  the same way on every link its sweep locks within the 5 s stop budget;
  a transfer on a link the sweep could not lock keeps its outcome pending
  until the transport is dropped, and the sweep logs how many links it
  skipped. Dropping the
  outcome cancels nothing: the transfer runs on and `resource_events()`
  still carries its terminal event. `Err(ConnectionError)` means the
  advertisement never left, so there is no transfer to follow.
  `resource::{OutboundOutcome, OutboundResult, OutboundFailure,
  RejectReason}` are the new types, the three enums `#[non_exhaustive]`,
  and `ResourceManager::track_prepared_with_outcome` is the manager-level
  pairing of `track_prepared` with an outcome. Pinned by
  `resource/tests_outcome.rs` (one test per verdict, including
  `Failed(Segmenting)` from a reader that fails while the next segment is
  built on the first segment's proof),
  `transport/tests_parts/two_transport_loopback_resources_outcome.rs`,
  `two_transport_loopback_resources_outcome_stop.rs` (`stop()` on the
  sender resolves a running split transfer `Failed(LinkClosed)`),
  `two_transport_loopback_resources_outcome_probe.rs` (a dropped outcome
  leaves the transfer running to `OutboundComplete`; a response Resource
  resolves `Complete` under the requester's `max_response_size`,
  `Rejected(PeerCancelled)` over it and `Failed(RetriesExhausted)` when no
  request solicited it; a stopped and dropped sender resolves
  `Failed(LinkClosed)`),
  `two_transport_loopback_resources_outcome_probe2.rs` (a hook applying a
  size policy read off the advertisement sorts two sends in flight on one
  link into `Complete` and `Rejected(PeerCancelled)`; two split Resources
  in flight each resolve `Complete` under their own hash; `stop()`
  concludes every transfer on the links it locks; a sender dropped without
  `stop()` resolves `Failed(LinkClosed)`, the only loopback pin that goes
  through the drop path alone),
  `two_transport_loopback_resources_outcome_probe3.rs` (an accepting hook
  is consulted once per segment advertisement of a three-segment Resource
  and sees the sender's hash as `original_hash` on each; a panicking hook
  that is never replaced resolves `Failed(RetriesExhausted)` on the sender
  with nothing assembled, the retries never re-running the hook because
  they re-send the first advertisement packet and the receiver's duplicate
  filter drops it, where Python builds a fresh packet per retry,
  `RNS/Resource.py:597-598`; `stop()` skips a link whose lock it cannot
  take within the budget and that transfer's outcome resolves
  `Failed(LinkClosed)` only when the transport is dropped) and
  `tests/outbound_outcome_api.rs` (the three enums and `RnsError` need a
  wildcard arm from another crate, a snippet of the README's shape
  compiles, an unknown link id is `InvalidArgument` on all three entry
  points, and the plain `send_*` signatures are unchanged).

- `Transport::set_resource_accept_hook` and the `transport::ResourceAcceptHook`
  alias (`Arc<dyn Fn(&ResourceAdvertisement, &LinkId, Option<&Identity>)
  -> bool + Send + Sync>`): once installed, the hook is consulted for every
  inbound plain resource advertisement, one that is neither a request nor
  a response, with the link it arrived on and the peer's identity if it has
  identified itself; `false` refuses the transfer with a `RESOURCE_RCL`
  receiver cancel (`RNS/Resource.py:155-165`) exactly as the size caps do,
  so the sender sees `Rejected(PeerCancelled)` and nothing is assembled.
  This is the `ACCEPT_APP` callback of `RNS/Link.py:1066-1074`; request and
  response advertisements stay with `max_request_size` and
  `max_response_size` (`RNS/Link.py:1036-1064`) and never reach the hook.
  A split resource consults the hook once per segment advertisement, so a
  refusal is consulted once and later segments are never offered. A hook
  that panics is logged at error and the advertisement is dropped
  unanswered, neither accepted nor refused, as `RNS/Link.py:1073-1074`
  logs the callback's exception; the transport's workers survive it.
  A later call replaces the hook; without one every plain advertisement is
  accepted, as before. Pinned by
  `transport/tests_parts/two_transport_loopback_resources_accept_hook.rs`
  (`accept_hook_panic_drops_the_advertisement_and_the_transport_survives`
  for the panic path) and `two_transport_loopback_resources_outcome_probe.rs`
  (the hook sees `None` before the peer identifies and `Some(identity)`
  after, a refused split resource draws one receiver cancel and one
  `Rejected`, and a response advertisement never reaches the hook).

### Changed

- The crate version is `0.15.0`; the discovery announce carries it in the
  `TRANSPORT_VERS` field (`discovery/announce.rs`), so peers observe the fork
  release through discovery. The release contracts are pinned by
  `crates/apps/reticulumd/tests/fork_publish_contract.rs` and
  `fork_release_docs_contract.rs`.

- `LinkEvent` is `#[non_exhaustive]`. The two in-workspace exhaustive
  matches (`destination/link_parts/link_sections/next_watchdog_deadline.rs`
  and `crates/apps/rns-tools/src/bin/independent_interop_node/events.rs`)
  gained a `PacketProven` arm, and the latter, being in another crate, a
  wildcard arm as well; every out-of-tree exhaustive match needs one. This
  is a breaking change and is listed under `### Breaking` above. The
  attribute is pinned by `tests/link_event_packet_proven_api.rs`.

- `ResourceEventKind::OutboundRejected` is now
  `OutboundRejected { reason: RejectReason }`, carrying why the transfer was
  refused as far as the sender can tell (`PeerCancelled` for a receiver
  cancel, the only reason produced in this release). This is a breaking
  change; every in-workspace match was adapted: in this crate the link's
  `pending_requests.rs` and the resource and transport tests, in
  `lxmf-runtime` the link delivery and its tests, in `rns-tools` the `rncp`
  client and server and the independent interop node's event feed, and in
  `reticulumd` the remote-control link bridge, the inbound worker core and
  the Python channel interop test support. An out-of-tree
  `OutboundRejected` pattern needs `{ .. }` or `{ reason }`. The reticulumd
  test `rust_sender_maps_pinned_python_receiver_cancel_to_rejection`
  (`python_channel_interop_parts/resource_faults.rs`) pins a Python
  receiver cancel to `PeerCancelled`.

- `ResourceEventKind` is `#[non_exhaustive]`: an out-of-tree exhaustive
  match on it no longer compiles without a wildcard arm. The enum was
  already broken for such a match this release by `OutboundRejected {
  reason }`, so the attribute lands in the same minor bump rather than
  costing another one. The in-workspace matches in `reticulumd` (the
  remote-control link bridge and the inbound worker core, which log an
  unknown kind at debug) and in the independent interop node (recorded as
  `"unknown"`) gained the arm; `rncp` and the Python channel test support
  already had one. Pinned by `tests/outbound_outcome_api.rs`.

- `AnnounceEvent` is `#[non_exhaustive]`: an out-of-tree struct literal
  no longer compiles, and a destructuring pattern needs `..`. Reading the
  fields is unchanged. The struct was already broken for a literal this
  release by the new `is_path_response` field. No cross-crate pin: a
  struct-literal refusal is a compile failure, and the crate's tests
  carry no compile-fail harness; the in-crate constructors are
  unaffected.

- `DeliveryReceipt` carries a private `link_id`, so it is built through
  `DeliveryReceipt::new` and no longer by struct literal; `message_id`
  stays public. `PartialEq` and `Eq` now compare `link_id` as well; a
  hash-only comparison is `matches_packet_hash`.

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
  `two_transport_loopback_resources_reject.rs`, and with Python RNS 1.5.2
  on the requesting side by the reticulumd test
  `python_to_rust_request_handler_resource_request_over_max_request_size_is_rejected_promptly`
  (`python_request_size_limits.rs`: a Python request Resource above the
  limit fails on the Python side through its receiver-cancel path in well
  under the request timeout, the handler never running).

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
  `two_transport_loopback_requests_resource_form_sizes_probe6.rs` (a
  limit set before or after the request Resource, or before a bare
  request packet, is kept, and a lying `d` under a
  receipt-less 16-byte or legacy-length record is caught at the
  post-assembly measurement and its `Complete` withheld),
  `two_transport_loopback_resources.rs`,
  `two_transport_loopback_resources_directions.rs`,
  `two_transport_loopback_resources_retries.rs`,
  `two_transport_loopback_resources_receipt_less_receiving.rs` (a
  two-segment response under a receipt-less id, stalled past the default
  request timeout, still completes),
  `two_transport_loopback_resources_reject_response.rs` (whose bare
  response fixtures now send the soliciting request Resource first, and
  whose reject test sends a second response under the rejected id and
  sees it dropped without a reject),
  `destination/link_parts/link_sections/pending_requests_tests.rs`, and
  with Python RNS 1.5.2 on the responding side by the reticulumd test
  `rust_to_python_request_receipt_resource_response_over_max_response_size_fails_oversize`
  (`python_request_size_limits.rs`: `Oversize` from the advertisement
  gate against a Python Resource reply, the link still serving afterwards).

- `Link::set_response_size_limit` composes with the request record in
  either order: called before `send_request_resource*` or `request_packet*`
  the limit survives the record those make without one, and called after
  it changes the record's limit alone, never its clock or request
  Resource. `Link::expect_response` keeps a limit already recorded under
  the id when given none. `Transport::request` still replaces the record
  whole, so a request sent without a limit under an id a displaced entry
  recorded one for does not inherit it. Pinned by
  `destination/link_parts/link_sections/response_packet_tests.rs` and
  `transport/tests_parts/two_transport_loopback_requests_resource_form_sizes_probe7.rs`
  (the public `expect_response` keeps a prior limit when given none and
  replaces it when given one).

- A request packet re-sent under the id of a record that is receiving its
  response, or whose request Resource is still in flight, no longer
  restarts that record's clock: `Link::note_request_sent` re-arms a running
  clock alone. A later segment advertisement refused by the record's limit
  cancels the partial assembly the record was receiving instead of
  orphaning it in the ResourceManager. The Link half, the record handing
  back what it was receiving, is pinned by
  `destination/link_parts/link_sections/response_record_tests.rs`; the wire
  half, the reject path sending that cancel, by
  `transport/tests_parts/two_transport_loopback_requests_resource_form_sizes_probe8.rs`
  (a later advertisement over the limit of a record receiving again
  cancels that partial assembly too).

- A record no receipt owns that is removed or replaced while receiving a
  response Resource hands that transfer back to be cancelled, like the
  receipt-owned entry does (`RNS/Link.py:1435`). Before, an over-limit
  response packet for a receipt-less `request_packet_with_max_response_size`
  record dropped the record with the inbound hashes, so no receiver cancel
  went out and the partial assembly went on to complete unmeasured, and a
  `Transport::request` or `send_request_resource*` re-sent under such a
  record's id left its transfer running. `take_response_limit_if_allowed`
  and `expect_response` keep their signatures and route the cancel through
  the link's owed cancels, which the transport drains on the link's next
  inbound packet, Resource packets included, before that packet's own
  advertisement is admitted. The cancel's own failure event is published,
  not routed, so a retry of the cancelled transfer admitted by that same
  packet keeps its fresh record; this holds by construction:
  `ResourceManager::cancel_incoming_response` hands the event back to its
  caller instead of queueing it, so it never enters the manager's event
  queue; everything the manager queued for this link before a Resource
  packet it admits (or a proof it handles) is routed by that packet's
  drain; a refused or unsolicited packet leaves the queue for the periodic
  drain, which routes it; other links' events wait for that periodic drain
  as well. Before, a Resource packet or proof drained the whole queue and
  published the other links' events unrouted, so a receipt on a sibling
  link could sit `Receiving` after its transfer was dropped. Pinned by
  `destination/link_parts/link_sections/response_record_tests.rs` and
  `transport/tests_parts/two_transport_loopback_requests_resource_form_sizes_cancel2.rs`
  (exactly one `ResourceReceiverCancel` reaches the responder), and
  `transport/tests_parts/two_transport_loopback_requests_resource_form_sizes_probe8.rs`
  (the public `expect_response` over a receipt-less record receiving its
  response: the displaced transfer is cancelled and the replacing record
  keeps or sets the limit), and
  `transport/tests_parts/two_transport_loopback_requests_resource_form_sizes_probe9.rs`
  (where the cancel leaves: an over-limit packet response sends it on that
  packet, a `send_request_resource*` re-sent under a receiving id cancels
  it in the call before the request Resource is advertised, and the cancel
  `expect_response` owes leaves on the link's next inbound packet, the
  responder's own receiver cancel included) and
  `transport/tests_parts/two_transport_loopback_requests_resource_form_sizes_probe10.rs`
  (the cancel owed to a displaced record runs before the responder's retry
  of that very advertisement is admitted: the retry is asked for parts
  again under the fresh record, which keeps its limit, and a later
  over-limit advertisement under the id is still refused with a receiver
  cancel; an advertisement for another id is admitted and assembled), and
  `transport/tests_parts/two_transport_loopback_requests_resource_form_sizes_probe11.rs`
  (a second displace-and-retry cycle on one id behaves like the first,
  and a wider limit set while displacing governs the re-admitted record),
  `transport/tests_parts/two_transport_loopback_requests_resource_form_sizes_probe12.rs`
  (an in-limit packet response answers a receipt-less `Receiving` record
  and spends it, no receiver cancel goes back, and a real many-part
  response over the legacy MTU still assembles) and
  `transport/tests_parts/two_transport_loopback_requests_resource_form_sizes_probe13.rs`
  (a manager failure queued before an unrelated Resource packet on the
  link still reaches the receipt as `Err(RequestError::Receive)`; this
  biases the race the receipt-timeouts fixture
  `..._failed_response_resource_fails_receive` runs against the retry
  worker so the inbound packet all but certainly wins),
  `transport/tests_parts/two_transport_loopback_requests_resource_form_sizes_probe14.rs`
  (a queued failure leaves on the advertisement another receipt admits,
  which still assembles; and on one admitting advertisement an owed cancel
  goes out once while a failure queued for a sibling link of the same
  connector fails that link's receipt `Receive`, with no receiver cancel on
  either link afterwards; and a refused advertisement on the link routes
  nothing, so a failure queued for that link reaches its receipt on the
  periodic drain),
  `transport/tests_parts/two_transport_loopback_requests_resource_form_sizes_probe15.rs`
  (the responder's part requests and proof for a resource-form request on
  one link leave a sibling link's queued failure for the periodic drain,
  which fails that receipt `Receive` while the request is proved and
  answered), and `resource/tests_timeouts_lifecycle.rs`
  (`..._drain_events_for_link_leaves_other_links_events_queued`: a
  packet's drain takes its link's events in order and leaves the others
  queued in order; `..._drops_the_receiver_and_the_assembly_together`: a
  cancel finding both reports the transfer once, under the original hash).

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

- `Transport::link` returns `Result<Arc<Mutex<Link>>, RnsError>` and fails
  fast with the new `RnsError::NoPath` when the path table has no next hop
  for the destination and the transport's `broadcast` is off. The request
  used to be dropped as `DroppedNoRoute` on the way out and the caller
  learned of it only when the pending link outlived its establishment
  timeout, which the transport sizes from the path as `RNS.Link.__init__`
  does: the first hop's timeout plus `ESTABLISHMENT_TIMEOUT_PER_HOP` (6 s)
  per hop, where a destination without a path entry counts as
  `PATHFINDER_M` (128) hops and has no first-hop metrics, so 6 s + 128 x
  6 s = 774 s, not the 12 s `DEFAULT_ESTABLISHMENT_TIMEOUT` earlier notes
  claimed. `NoPath` sends nothing, no link request and no path request;
  discovering the destination first is `Transport::request_path` plus
  `Transport::await_path`, then `link`. With `broadcast`
  on the request still goes out on every interface, as
  `RNS/Transport.py:1443` floods a packet without a path entry. `RnsError`
  is now `#[non_exhaustive]`. `delivery::send_via_link` and the
  lxmf-runtime link delivery surface that failure immediately instead of
  waiting out their activation timeout. The lxmf-runtime delivery fails
  fast only when its first attempt finds no path; a later attempt that
  finds none, after a pending link's expiry removed the path, spends that
  attempt in `await_path` and keeps the remaining attempts, and the final
  error carries both the activation failure and the missing path. Pinned by
  `transport/tests_parts/link_no_path_fast_fail.rs`,
  `tests/outbound_outcome_api.rs`
  (`link_without_a_path_fails_fast_and_repeatably_when_broadcast_is_off`),
  `delivery.rs`
  (`send_via_link_without_path_fails_before_the_activation_wait`),
  the reticulumd test
  `open_refreshed_remote_link_without_a_path_refreshes_once_and_fails` and
  the lxmf-runtime tests
  `link_delivery_without_path_fails_before_the_first_activation_wait` and
  `link_delivery_waits_for_a_path_that_vanished_between_attempts`; the two
  in-crate fast-fail pins bound the call at 500 ms (`delivery.rs` at 1 s).

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
