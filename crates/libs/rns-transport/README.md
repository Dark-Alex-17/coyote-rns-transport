# coyote-rns-transport

## What this is

Coyote's published fork of the upstream `reticulum-rs-transport` crate from
[LXMF-rs](https://github.com/FreeTAKTeam/LXMF-rs): the transport, link,
interface, receipt, and resource layers of the Reticulum network stack.

Apart from the patch list below and the API breaks `CHANGELOG.md` lists under
`### Breaking`, only the package name changes: the library target stays
`rns_transport`, so every `use rns_transport::...` path written against
upstream resolves unchanged against this crate.

The version follows cargo semver relative to the upstream baseline. `0.14.0`
is upstream `0.14.0` renamed, with the link-close patch applied. A fork
release that only fixes bugs, or adds API a consumer need not opt into, bumps
the patch digit; a fork change that breaks the public API bumps the minor
digit, and `0.15.0` is the first such minor bump; its breaking changes are
listed under `### Breaking` in `CHANGELOG.md`. A fork release that adds public
API without breaking any may bump the minor digit too, so the digit tells a
consumer there is new surface to opt into; `0.17.0` is the first such additive
minor, and its `### Breaking` says so in place of a list. When upstream ships a
release the fork is rebased onto it and takes the first version above both the
upstream number and the fork's last published number: the next minor digit
when the rebase breaks the fork's public API, the next patch digit otherwise.

## Consumer

```toml
[dependencies]
coyote-rns-transport = "0.17.0"
```

```rust
use rns_transport::destination::link::Link;
```

Replace upstream `reticulum-rs-transport` with this crate rather than adding
it alongside: both crates have the library name `rns_transport` and cannot
coexist in one dependency graph.

## Request handlers

A `SingleInputDestination` answers link requests through handlers registered
by path, as `RNS.Destination.register_request_handler` does. A handler is a
closure (or a `RequestHandler` impl) that receives the `InboundRequest` and
returns a future of `Option<RequestResponse>`:

```rust
use std::sync::Arc;
use rns_transport::destination::request::{InboundRequest, RequestAllow, RequestResponse};

destination.lock().await.register_request_handler(
    "/echo",
    Arc::new(|request: InboundRequest| async move {
        Some(RequestResponse::Value(request.into_data()))
    }),
    RequestAllow::All,
    true,
)?;
```

Registering a path again replaces the earlier handler;
`deregister_request_handler(path)` removes it and returns whether one was
registered (`RNS/Destination.py:400-412`). `RequestAllow` carries the three
policies of `RNS/Destination.py:74-77`: `None` answers nobody, `All` answers
every peer, and `List(HashSet<AddressHash>)` answers only a peer that
identified itself over the link with one of the listed identity hashes
(`RNS/Link.py:821`); a request the policy denies, one from an unidentified
peer included, is dropped with a debug log (`RNS/Link.py:855`). A handler
that returns `None` sends no response at all. Each handler runs on a task of
its own with no concurrency cap, so a slow handler holds up only its own
request; a handler that needs a bound holds its own `Semaphore`. A handler
that panics is caught and logged at warn, and its request goes unanswered.

`set_max_request_size`, `clear_max_request_size` and `max_request_size` on
the destination bound the packed size of a request it answers
(`RNS/Destination.py:369-379`); with no limit set every size is accepted. A
request that fits the link MDU arrives as one packet and is measured against
the limit before any handler runs (`RNS/Link.py:999-1001`). A larger request
arrives as a request Resource, measured against the limit at its
advertisement, before any part is accepted (`RNS/Link.py:1036-1042`): an
oversize one is rejected there, which the requester sees as
`ResourceEventKind::OutboundRejected` and as `RequestError::Rejected` on its
receipt. Once an accepted request Resource is assembled the same handler
runs, with the request id recomputed from the assembled bytes.

`set_max_response_size`, `clear_max_response_size` and `max_response_size`
on the destination bound the size of a response it sends; with no limit set
every size goes out. The cap covers the responses the dispatcher sends for
the destination's registered handlers; `Transport::send_response` and
`Transport::send_response_resource` are addressed to a link, not a
destination, and are not measured. The dispatcher measures a `Value` or
`Bytes` response as its packed `[request_id, body]` frame, before the MDU
check that would turn an oversize frame into a Resource, and a
`RequestResponse::Resource` as the size its advertisement carries, the
uncompressed `data` behind the packed metadata and its length prefix (a
metadata-less Resource measures as its data alone), like
`max_request_size`, which reads the advertised size rather than the
compressed one. A packet response is measured here as its whole
`[request_id, body]` frame while the requester measures the packed body
alone, so for the packet form the responder refuses first; for the Resource
form both caps read the advertised size and compose at one number. A
response at the cap goes out; one over it is not sent at all: the
dispatcher logs one warn naming the request id, the link, the measured
length and the cap, and a handler that tracked its outcome resolves
`ResponseOutcome::TooLarge { len, max }`. A handler that did not track is
told nothing, and the requester sees an unanswered request. The
requester's cap on what it will accept is
`RequestOptions::with_max_response_size`; `set_max_response_size` is the
responder's cap on what it will send. Python has no responder cap:
`handle_request` sends whatever the handler returns (`RNS/Link.py:846-852`).

Between a link and the transport's request dispatch worker sits a bounded
queue of 1024 requests; a full queue drops the request with a debug log
rather than blocking the link. This is a resource limit, not a protocol
difference: Python starts a thread per request and has no bound
(`RNS/Link.py:1004-1005`). A request arriving on a link that is not
`Active` is ignored at the link with a debug log (`RNS/Link.py:805`). The
worker drops, each with a debug log and never a panic, a malformed request
frame, a request naming a destination the transport does not hold, a request
over the destination's `max_request_size`, a request whose path hash has no
handler, and a request a `RequestAllow` policy denies.

On the response side, a `Value` or `Bytes` that fits the link MDU goes out as
one packet, never compressed; one that outgrows the MDU travels as a Resource
whose data is the whole `[request_id, body]` envelope. A
`RequestResponse::Resource { data, metadata }` always travels as a Resource
whose data is the raw `data`, with the packed metadata ahead of it. A Python
requester reads a response Resource as a file response only when it carries
metadata (`RNS/Link.py:902-912`), so a `Resource { metadata: None }` answer is
unpacked as an envelope there and is only for a peer that expects raw bytes.
Both advertise the request id in `q` with the response flag `p` set, and both
honour the registration's `auto_compress`. A response Resource reaches the
requester through `Transport::resource_events()` and resolves its
`RequestReceipt`.

A handler that wants to know what became of its response calls
`request.track_outcome()` before `into_data` consumes the request and gets
a `ResponseOutcomeReceiver`, a future the dispatcher resolves after the
handler has returned, so it is awaited outside the handler. The
`ResponseOutcome` it resolves to is `#[non_exhaustive]`: `Resource` carries
the `OutboundResult` of every response that leaves as a Resource, the
`Resource` form and an oversize `Value` or `Bytes` alike; `Packet` says the
`RESPONSE` packet reached at least one interface, and no more, since a packet
response is never proved (see Delivery proofs on a link); `TooLarge` says
the destination's `max_response_size` refused the response and nothing left,
its `len` the measured size and `max` the cap; `NotSent` covers a handler
that returned `None`, a link that could no longer exchange data, a send that
failed before dispatch, a handler that panicked after tracking, and a
response task that `Transport::stop` or `cancel` aborted before the outcome
was known: every path that drops the sender. Calling `track_outcome` a
second time resolves the earlier receiver as `NotSent`. Python has no
counterpart: `handle_request` sends and keeps nothing (`RNS/Link.py:846-852`).

A handler that wants the request body owned before it decides whether to
watch the response calls `into_parts` instead, which consumes the request
and returns the data and a `ResponseOutcomeTracker`; the tracker's `outcome`
hands out the `ResponseOutcomeReceiver` under the same rule as
`track_outcome`, armed inside the handler before it returns. A tracker
dropped without being armed leaves the slot as it was: with no earlier
`track_outcome`, the dispatcher logs nothing and resolves nothing, so a
handler that chose not to watch leaves no "outcome receiver dropped" line
behind.

`LinkEvent::Data` with `PacketContext::Request` keeps posting for every
inbound request packet, its `LinkPayload` carrying the request id, and
`ResourceEventKind::Complete` with `is_request` for every assembled request
Resource; the same packet or Resource is handed to the dispatcher. A request
whose path has a registered handler is answered by that handler, and
broadcast consumers must not answer it.

## Sending requests

`Transport::request` sends a request over an active link and returns a
`RequestReceipt`, as `RNS.Link.request` does. The request goes out as one
packet when it fits the link MDU and as a request Resource otherwise; the
response is matched to the receipt as it arrives, as a packet or as a
Resource, so nothing has to be read from the broadcast streams:

```rust
use rns_transport::{destination::request::RequestOptions, rmpv};

let receipt = transport
    .request(&link_id, "/echo", rmpv::Value::from("ping"), RequestOptions::default())
    .await?;
let response = receipt.response().await?;
assert_eq!(response.value(), Some(&rmpv::Value::from("ping")));
```

The receipt is `Clone`, and `response()` resolves on any clone once the
request is `Ready` or `Failed`; a second call returns the same outcome. A
packet-form request goes `Sent -> Ready`, or `Sent -> Receiving -> Ready`
when the response is a Resource; a resource-form request reaches `Delivered`
when its Resource completes, and only a resource-form request ever reports
`Delivered`. `status()` reads the current status and `subscribe()` returns a
tokio `watch` receiver that reports each change from then on. A `watch`
keeps only the latest value, so quick successive changes coalesce and a fast
`Sent -> Ready` can skip `Receiving`; the sender is dropped when the request
concludes, so `changed()` returns an error after the final status and
`borrow()` keeps it. `request_id()` is the id the responder knows the request
by. `progress()` is the share of the response received, `0.0` to `1.0`
(`RNS/Link.py:1429`, `:1439`), counted across every segment of a split
response; a packet response jumps straight to `1.0`. `response_size()` is
`d` of the first response advertisement or the packed body length of a
response packet (`RNS/Link.py:1057-1058`, `:1016`), and
`response_transfer_size()` is `t` summed over the response segments or the
same figure as `response_size()` for a packet (`:1059-1061`); both are
`None` until a response arrives. `started_at()` is the send for a
packet-form request (`RNS/Link.py:1331-1334`) and `None` for a resource-form
one until it reaches `Delivered` or a response Resource is admitted,
whichever comes first (`:1370-1371`, `:1062-1063`); `concluded_at()` is set
once the receipt is `Ready` or `Failed`.

The timeout defaults to `rtt * 6 + 11.25 s` (`RNS/Link.py:491`,
`RNS/Resource.py:118`) and `RequestOptions::with_timeout` overrides it. Two
clocks exist: a packet-form request counts from the send, a resource-form
request from `Delivered`, when the request Resource has been proved
(`RNS/Link.py:1370-1373`). No clock runs while the request Resource is in
flight or while a response Resource is being received (`Receiving`,
`RNS/Link.py:1389`); the Resource's own outcome decides those. A receipt
still waiting when its clock runs out fails with `RequestError::Timeout`,
in any status except `Receiving`, the packet form in `Sent` included; Python
fails only a `Delivered` receipt (see Known divergences). The other failures:
a link that closes, from either side, fails every pending receipt with
`LinkClosed`, as does `Transport::stop()` within its budget; a request Resource the
responder refuses fails `Rejected`; one that cannot be sent, exhausts its
retries or is cancelled through `Transport::cancel_resource` fails `Send`; a
response Resource that fails in transfer fails `Receive`; a response that is
not a `[request_id, body]` envelope, names another request, or carries
metadata that is not msgpack fails `Decode`; and a response over the size
limit fails `Oversize`. `RequestError` is `#[non_exhaustive]`.

`RequestOptions::with_max_response_size` is measured against the uncompressed
size of the response: the packed body of a response packet or the advertised
data size of a response Resource. A response Resource over the limit is
refused at its advertisement, a response packet over it is dropped, and a
response Resource that assembles over it (the advertised size was the peer's
claim) is failed after assembly; each of the three fails the receipt with
`Oversize`; the packet case and the check after assembly are stricter than
Python (see Known divergences). An over-limit packet Response is the one
`LinkEvent::Data` the link withholds, and a response Resource that assembles
over the limit has its `Complete` withheld.

Every request packet the link sends, however it was built, and every
Resource sent through `send_request_resource*` solicits its response; a
response Resource advertised for a request nothing on the link sent is
dropped without a reject. The solicitation lasts the default request timeout,
counted from the send for a request packet and from the request Resource's
delivery for `send_request_resource*`, unless a response Resource is being
received, in which case that Resource's own outcome ends it. An id a
`Transport::request` receipt owns is solicited for the receipt's own timeout
instead, default or not. How this receipt-less record differs from Python's
receipt-only bookkeeping is under Known divergences.

A response Resource with metadata is a file response, read through
`RequestResponseData::data()` and `metadata()`; every other response is the
envelope body, read through `value()`, or `into_value()` to take it. Dropping
every clone of a receipt does not cancel the request: the responder still
serves it and the response still reaches the link's broadcasts, less the two
over-limit cases above. A request on a link that is not `Active` (`Stale`
included) is refused with `RnsError::ConnectionError` before anything is
sent (see Known divergences).

## Delivery proofs on a link

A plain Data packet sent on a link is proved by the peer (`RNS/Link.py:950-962`),
and the proof posts `LinkEvent::PacketProven { packet_hash }` on the link's
event stream, on whichever side sent the packet, once per packet: the hash is
recorded when the packet is sent and the first valid proof naming it consumes
the record, the way Python's proof removes the one outstanding receipt it
validates (`RNS/Transport.py:2748-2761`). A link remembers at most 1024
unproved sent packets, oldest dropped first, so a proof for a dropped record
posts nothing. Requests, responses and Resource packets are never recorded or
proved, and a Channel frame, though proved, posts no `PacketProven`. Every
validly signed link proof, a Channel frame's proof included, reaches the
transport's `ReceiptHandler` as a `DeliveryReceipt` whose `link_id()` names
the link, and so does a replayed proof the ingress packet filter lets
through (one the bounded duplicate cache has forgotten); the receipt is not
filtered by the record. The request layer's own `RequestReceipt` is the
delivery signal for a request. A stock Python peer never proves a link
packet, and Python records and culls its receipts differently (see Known
divergences).

## Outbound resource outcomes

`Transport::send_resource_with_outcome`, `send_request_resource_with_outcome`
and `send_response_resource_with_outcome` are the existing `send_*` entry
points plus an `OutboundOutcome`, returned as `(Hash, OutboundOutcome)`: a
future that resolves once, with the verdict `resource_events()` reports for
the same transfer. `OutboundResult`, `OutboundFailure` and `RejectReason`
are `#[non_exhaustive]`, so a match on any of them keeps a wildcard arm:

```rust
use rns_transport::resource::OutboundResult;

let (hash, outcome) = transport
    .send_resource_with_outcome(&link_id, payload, None, true)
    .await?;
match outcome.await {
    OutboundResult::Complete => log::info!("{hash} proven"),
    OutboundResult::Failed(failure) => log::warn!("{hash} failed: {failure:?}"),
    OutboundResult::Rejected(reason) => log::warn!("{hash} refused: {reason:?}"),
    OutboundResult::Cancelled => log::info!("{hash} cancelled here"),
    _ => log::warn!("{hash} ended with a verdict this match predates"),
}
```

`Complete` when the peer proves the final segment; a split resource resolves
once, when its last segment concludes, not per segment. `Failed(failure)`
when the transfer stops without a proof: `NotSent` when the advertisement
was never dispatched, a verdict the three `send_*_with_outcome` entry points
never hand out, since they return `Err(ConnectionError)` instead of the
outcome when the advertisement does not leave; it reaches only a caller of
`ResourceManager::track_prepared_with_outcome` that follows up with
`confirm_outbound_dispatch(.., false)`. `RetriesExhausted` when the retry
budget ran out while the resource was advertised, transferring or awaiting
its proof, `Segmenting` when a later segment of a split resource could not
be built or the peer's requests put the sender in a failed state, and
`LinkClosed` when the link went away under the transfer. `Rejected(reason)`
when the peer refused the transfer: a refusal arrives as a receiver cancel
carrying only the resource hash (`RNS/Resource.py:155-165`), so the sender
never learns the peer's motive and `RejectReason::PeerCancelled` is the only
variant; the enum is `non_exhaustive`, so a future variant is not a breaking
change; it implements `Display`. `Cancelled` after
`Transport::cancel_resource`. Every terminal path goes through one exit, so
the outcome and the `ResourceEventKind` published for the hash cannot
disagree. Dropping the outcome cancels nothing: the transfer runs on and
`resource_events()` still carries its terminal event. A transport dropped
before a verdict resolves the outcome `Failed(LinkClosed)` rather than
leaving it pending; `Transport::stop()` does the same for every transfer on
a link its sweep locks within the 5 s stop budget, and a transfer on a link
it had to skip keeps its outcome until the transport is dropped. A link id
the transport does not hold is refused with `Err(InvalidArgument)` before
any advertisement exists, the answer the plain `send_*` entry points gave
before, and `Err(ConnectionError)` from the send means the advertisement
never left, so in neither case is there a transfer to follow. For a request
Resource, a destination whose `max_request_size` the advertisement exceeds
refuses it with a receiver cancel (`RNS/Link.py:1036-1042`), read as
`Rejected(PeerCancelled)`; the outcome says nothing about the response,
which arrives through `resource_events()` as before. For a response
Resource, a requester whose `max_response_size` it exceeds refuses it the
same way (`RNS/Link.py:1044-1051`), and one that no longer expects a
response under the request id drops the advertisement without answering
(`RNS/Link.py:1046-1047`), so that outcome ends in
`Failed(RetriesExhausted)`.

## Opening links and admitting resources

`Transport::link` returns `Result<Arc<Mutex<Link>>, RnsError>` and fails
fast with `RnsError::NoPath` when the path table has no next hop for the
destination and the transport's `broadcast` is off: the request would have
been dropped as `DroppedNoRoute` on the way out, and the caller learned of
it only when the pending link outlived its establishment timeout. That
timeout is sized from the path as `RNS.Link.__init__` sizes it, the first
hop's timeout plus `ESTABLISHMENT_TIMEOUT_PER_HOP` for every hop to the
destination, and a destination without a path entry counts as
`PATHFINDER_M` hops, so the wait ran to minutes rather than seconds.
`NoPath` sends nothing: no link request and no path request. A caller that
wants to discover the destination first asks with `Transport::request_path`
and waits with `Transport::await_path`, then calls `link`. With `broadcast`
on the request still goes out on every outgoing interface, as
`RNS/Transport.py:1439-1443` floods a packet without a path entry.
`RnsError` is `#[non_exhaustive]`, so a match on it keeps a wildcard arm.

`Transport::destination_desc` returns `Option<DestinationDesc>` for a
destination learned from an announce (the same map is also filled when a
cached path table is restored), `Some` exactly when
`Transport::destination_identity` is `Some`. The desc's `name` is the
announce's truncated name hash as `DestinationName::new_from_hash_slice`
stores it, not the full hash `DestinationName::new` computes, so compare
names with `as_name_hash_slice()` rather than against a freshly built
`DestinationName`. The entry outlives `Transport::expire_path`, which
touches only the path table, and is removed only by
`Transport::deregister_destination`. The desc is the argument
`Transport::link` takes, though the link still needs a path
as described above.

`Transport::resolve_destination` is `destination_desc` and
`Transport::await_path` in one call, taking the address and a `Duration`
timeout and returning the same `Option<DestinationDesc>`. A destination the
transport already knows answers at once and no path request goes out. A
zero timeout is a cached lookup alone and returns `None` for an unknown
destination without sending anything. Otherwise exactly one path request is
attempted, a `Broadcast` on every outgoing interface as `Transport::request_path`
sends it, and the call polls every 50 ms until the announce is learned or
the timeout passes, returning `None` after it. The
desc and the path table can part ways: a destination
`Transport::deregister_destination` removed while its path table entry stands
answers `None` at once with no request, since `await_path` takes the standing
path as its answer, and a destination whose desc is still held but whose path
`Transport::expire_path` or aging removed answers `Some` at once with no
request, so a caller that then needs a next hop for `Transport::link` still
calls `await_path` itself.
The convenience is fork-only: a Python application writes the same by hand
with `RNS.Transport.request_path` (`RNS/Transport.py:3279`), a
`RNS.Transport.has_path` poll (`RNS/Transport.py:3126`) and
`RNS.Identity.recall`.

Looking up a learned destination and opening a link with it, on the transport
that learned the announce and with `destination_hash` the `AddressHash` the
announce carried:

```rust
use rns_transport::destination::DestinationName;

let desc = transport
    .destination_desc(&destination_hash)
    .await
    .ok_or("the announce has not been learned yet")?;
assert_eq!(
    desc.name.as_name_hash_slice(),
    DestinationName::new("example", "echo").as_name_hash_slice(),
);
let link = transport.link(desc).await?;
```

`crates/libs/rns-transport/tests/destination_desc_api.rs` runs this snippet
verbatim on a transport that learned the announce over a loopback `TcpClient`
and reads the link's `Activated` event back on the announcing side.

`AnnounceEvent::is_path_response` is true for an announce the destination
sent in answer to a path request, which it files under
`PacketContext::PathResponse` (`RNS/Destination.py:308`), and false for an
unsolicited one; Python hands its announce handlers the same flag
(`RNS/Transport.py:2524-2528`). The flag repeats the context the announcing
peer put on the packet and is not matched against any path request this
transport sent, so it is information about the announce, not a check. Every
announce reaches `Transport::recv_announces()`; a consumer that wants
Python's default filters on the flag itself (see Known divergences).

`Transport::set_resource_accept_hook` installs a `ResourceAcceptHook`, an
`Arc<dyn Fn(&ResourceAdvertisement, &LinkId, Option<&Identity>) -> bool +
Send + Sync>`, consulted from then on for every inbound plain resource
advertisement, one that is neither a request nor a response, with the link
it arrived on and the peer's identity when it has identified itself on that
link. This is the `ACCEPT_APP` callback of `RNS/Link.py:1066-1074`; request
and response advertisements stay under `max_request_size` and the
requester's `max_response_size` (`RNS/Link.py:1036-1064`) and never reach
the hook. `false` refuses the transfer with a receiver cancel, exactly as the
size caps do, so the sender reads `Rejected(PeerCancelled)` on its outcome and
`ResourceEventKind::OutboundRejected` on its events, nothing is assembled,
and the link survives. A split resource consults the hook once per segment
advertisement, so a refusal is consulted once and later segments are never
offered. A hook that panics is logged at error and the advertisement is
dropped unanswered, neither accepted nor refused, as `RNS/Link.py:1073-1074`
logs the callback's exception; the transport's workers survive it. The
hook runs inside the inbound packet path with the link locked, so it must
return promptly and must not call back into the transport. A later call
replaces it; without one every plain advertisement is accepted, which is
not Python's default (see Known divergences).

## Attaching to a shared instance

`Transport::attach_shared_instance(SharedInstanceEndpoint, SharedInstanceOptions)`
spawns a `TcpClient` for a `SharedInstanceEndpoint::Tcp(SocketAddr)`, or on
unix targets a `LocalUnixClient` for `Unix`, marks that interface as the shared
instance, sets the transport's client posture, synthesizes a tunnel on it (at
attach and again on every reconnect) and returns the interface address: the
posture of a Python program
attaching to the instance that owns `shared_instance_port`
(`RNS/Reticulum.py:422-441`; the default port is `:295`), with the
client-side hop count of `RNS/Transport.py:1940` and the transport header
on one-hop paths of `:1416-1427`. One shared instance is held at a time:
`shared_instance()` reports the attached interface, a second call returns
`RnsError::InvalidArgument` until `detach_shared_instance()` removes the
first (it returns `bool`, `false` when nothing was attached), and
`Transport::stop()` removes it too. Only those two release the held slot:
stopping or removing the interface directly with `stop_interface` or
`remove_interface` leaves it held, and a later attach is refused until
detach; an attach after `stop()` has cancelled the transport is refused with
`RnsError::InvalidArgument` too. `reconnect_wait` (default 8 s,
`RNS/Interfaces/LocalInterface.py:54`) is honoured by both the unix and the
TCP client (`TcpClient::with_reconnect_wait`; the client's own default stays
5 s) and clamped to at least 1 ms by both: the unix client sleeps it before
every redial, a dropped stream's first included, as Python's reconnect loop
does (`RNS/Interfaces/LocalInterface.py:159-160`); the TCP client redials
at once after a drop and sleeps it from the second consecutive short-lived
connection on, one that lasted under `MIN_STABLE_CONNECTION` (2 s); a
connection that outlived the window resets the streak.
`SharedInstanceOptions` also carries `with_mtu`, `with_forced_bitrate` and
`with_interface_mode`, applied to the spawned client as the matching
`TcpClient`/`LocalUnixClient` constructors would, with the `mtu` and
`forced_bitrate_bps` getters reading the first two back; `LocalUnixClient`
exposes `addr`, `mtu_value`, `forced_bitrate_bps`, `reconnect_wait`,
`connect_events_enabled`, `reconnect_events_enabled` and
`disconnect_events_enabled` for inspection.
While
attached, a `SINGLE` destination is announced as a path
response 0.25 s after `register_destination` (`RNS/Transport.py:2911-2916`,
the context of `RNS/Destination.py:308-315`), and
`Transport::announce_destinations_to_shared_instance` sends that announce
for every registered destination on demand. On every reconnect the tunnel is
re-synthesized at once and every registered single destination is
re-announced as a path response `reconnect_wait` plus 2 s later, if the
transport is still attached then (`RNS/Transport.py:3705-3709`,
`RNS/Interfaces/LocalInterface.py:178-181`). A dropped connection, as
opposed to a detach or a stop, tears down every active and pending link,
sending a LINKCLOSE packet to the peer of each link past `Pending` on the
interface it rides, as Python's `Link.teardown` does for every such link
(`RNS/Link.py:657-668`), so a peer reached over another interface learns
the close from the wire, while a link that rode the dropped interface
itself sends none, its peer being gone with the connection (the flush
would inherit the `Handshake` LINKCLOSE too; no `in_links`/`out_links`
entry is observable in `Handshake`, because the responder proves, which
makes it `Active`, before the insertion, `transport/path.rs:372` vs
`:383`), and
clears the path, announce, link and tunnel tables, the held announces and
the packet cache's reverse routes, the received-packet entries that are
Python's `reverse_table`
(`RNS/Transport.py:3690-3702`, `RNS/Interfaces/LocalInterface.py:255-261`),
and with them the fork's own announce cache tier, which has no Python twin,
and its interface-tunnel index, whose twin is `interface.tunnel_id`
(`RNS/Transport.py:2827`, `:2834`, read at `:2467`): Python's
`shared_connection_disappeared` (`:3690-3702`) leaves it set, so Python's
`note_path` on a vanished tunnel logs the `:2469` warning where the fork's
does nothing; the entries of packets this node sent,
the receipt guard that is Python's `receipts` list (`RNS/Transport.py:174`),
stay;
a queued disconnect is served before the reconnect behind it, so the flush
never lands after that cycle's tunnel, and since the client emits each
cycle's disconnect before its reconnect into two equal-depth queues, a
disconnect can never be lost while its reconnect lands (the realistic storm
loss is a reconnect, healed by the next one that lands).
While attached, the duplicate filter
inserts nothing and consults nothing (`RNS/Transport.py:1618-1627`): the
owner deduplicates. The filter is skipped for packets arriving on the
shared-instance interface, or everywhere when no interface is flagged, since
a Python client brings up no other interface while attached
(`RNS/Reticulum.py:745`) and this transport keeps its own interfaces
filtering. The packet cache's other halves are still written: the
source notes `handle_data` records for received Data packets share the cache
and are seen as duplicates once the transport detaches, and the hash of each
packet sent is the receipt guard. A transport whose connected posture is set
directly with `set_connected_to_shared_instance` rather than through
`attach_shared_instance` gets the registration announce, whose trigger is the
posture alone, but not the reconnect re-announce and the drop flush, which
live in the attach's consumer task; reticulumd's shared-instance client
posture goes through the attach. `Transport::stop` joins the tasks the
client spawns (the delayed announces and the reconnect consumer) within its
budget, so none runs on a stopped transport.

`SharedInstanceOptions::with_state_events` takes a
`tokio::sync::mpsc::Sender<SharedInstanceEvent>` and delivers the client's
connection state on it.
`SharedInstanceEvent` is `#[non_exhaustive]`, so match it with a wildcard
arm; its variants are `Connecting`, `Connected` and `Disconnected`. The
attach itself sends the first `Connecting` once the interface is spawned and
the slot taken, the connected posture already set when `Connecting` arrives
while the slot still holds the interface (a refused attach sends nothing; in
the documented attach/stop race the attach returns `Ok` and `Connecting`
goes out with the posture off, the stream ending at once); the consumer task
sends `Connected` on every established stream, the first included, and after
every dropped connection `Disconnected`, once the flush above, which runs
only while the transport is still attached, has run, then `Connecting` at
once, so one cycle reads `Disconnected`, `Connecting`, `Connected`. Under a
reconnect storm the cycles can interleave: queued disconnects are served
before connects, so two `Disconnected` can precede their `Connected`s; the
per-cycle order holds, the cycles may nest. A client attached while nothing
listens stays in `Connecting` until the socket exists. Every event goes out
with `try_send`: a full channel drops the event at debug and never blocks
the consumer, so size the channel for the burst expected. The attach's clone
of the sender is dropped on `detach_shared_instance`, `Transport::stop` or
when the client worker exits; the stream ends once no other clone, a
retained `SharedInstanceOptions` clone included, is alive. A detach or a
stop sends no `Disconnected`, but a drop that was already queued when the
detach landed is still served and read as `Disconnected`, `Connecting`
before the stream ends (a connect already queued is read as `Connected`).
`connected_to_shared_instance()` is the posture flag and is unchanged by the
events: it stays set across a `Disconnected` until detach or stop. Python's
`LocalClientInterface` exposes no such stream; its reconnect loop is
`RNS/Interfaces/LocalInterface.py:155-181` and its drop path
`RNS/Interfaces/LocalInterface.py:255-261`. Underneath, `TcpClient` and
`LocalUnixClient` gained `with_connect_events`, which fires on every
established stream, the first included, where `with_reconnect_events` fires
from the second on, and a `connect_events_enabled` getter.

A `Tcp` endpoint whose address is not loopback is refused with
`RnsError::SharedInstanceEndpointNotLoopback` before any lock is taken, with
one warning, since Python's `LocalClientInterface` connects to `127.0.0.1`
only (`RNS/Interfaces/LocalInterface.py:109`). Passing
`SharedInstanceOptions::default().allow_remote_endpoint(true)` attaches it,
again with one warning per attach, and grants the remote host the trust a
local shared instance gets: it receives dedup delegation, since the duplicate
filter is off while attached (`RNS/Transport.py:1618-1627`), and its stream
drop flushes every table and tears down every link
(`RNS/Transport.py:3690-3702`). Python has no remote option at all.

A client that attaches, registers a destination with a request handler,
announces it to the owner on demand and detaches, with `endpoint` the owner's
`SocketAddr`:

```rust
use std::sync::Arc;
use rns_transport::destination::request::{InboundRequest, RequestAllow, RequestResponse};
use rns_transport::destination::DestinationName;
use rns_transport::identity::PrivateIdentity;
use rns_transport::rand_core::OsRng;
use rns_transport::transport::{SharedInstanceEndpoint, SharedInstanceOptions};

let iface = transport
    .attach_shared_instance(
        SharedInstanceEndpoint::Tcp(endpoint),
        SharedInstanceOptions::default(),
    )
    .await?;
let identity = PrivateIdentity::new_from_rand(OsRng);
let destination =
    transport.add_destination(identity, DestinationName::new("example", "echo")).await;
destination.lock().await.register_request_handler(
    "/echo",
    Arc::new(|request: InboundRequest| async move {
        Some(RequestResponse::Value(request.into_data()))
    }),
    RequestAllow::All,
    true,
)?;
let announced = transport.announce_destinations_to_shared_instance().await;
assert_eq!(announced, 1);
assert!(transport.detach_shared_instance().await);
```

`crates/libs/rns-transport/tests/shared_instance_api.rs` runs this snippet
verbatim against an in-process shared instance, a transport-enabled node with
a `TcpServer` on 127.0.0.1, and reads the announce back as a path response.
That plain server reads it at one hop; rnsd and reticulumd flag their
listener as the shared instance and read a local client's announce at zero
(`RNS/Transport.py:1938`). `crates/apps/reticulumd/tests/python_channel_interop_parts/python_shared_instance_live.rs`
attaches this crate to a Python `rnsd` (RNS 1.5.2) shared instance on a free
port next to a Python local client and proves the zero-hop path, announce and
`Link.request` both ways, over TCP everywhere and over an abstract unix socket on Linux.

A client that attaches with a state stream and logs every transition until
the stream ends, with `transport` the client transport and `endpoint` the
owner's `SocketAddr`:

```rust
use rns_transport::transport::{SharedInstanceEndpoint, SharedInstanceEvent, SharedInstanceOptions};
use tokio::sync::mpsc;

let (events_tx, mut events) = mpsc::channel(8);
transport
    .attach_shared_instance(
        SharedInstanceEndpoint::Tcp(endpoint),
        SharedInstanceOptions::default().with_state_events(events_tx),
    )
    .await?;
while let Some(event) = events.recv().await {
    match event {
        SharedInstanceEvent::Connecting => log::info!("shared instance: dialing"),
        SharedInstanceEvent::Connected => log::info!("shared instance: connected"),
        SharedInstanceEvent::Disconnected => log::warn!("shared instance: dropped, redialing"),
        _ => {}
    }
}
```

`crates/libs/rns-transport/tests/shared_instance_api.rs` runs this snippet
verbatim too: it reads `Connecting` then `Connected` against the in-process
shared instance and sees the stream end on the detach.

## Known divergences from Python RNS

Where this crate is known to behave differently from RNS 1.5.2. Each entry
names what the crate does, what Python does, and where in Python to look:
the `RNS/<file>.py:line` cites resolve against the Python Reticulum
repository (<https://github.com/markqvist/Reticulum>) at commit `ea98db4f`,
the ref the `verify.yml` interop workflow checks out.
The register covers the divergences the fork introduced; behaviour
inherited from upstream `reticulum-rs-transport` is not catalogued here,
except where a fork change made it visible, and such an entry says so.
For fork-only parity rows this register plus `docs/fork/PATCHSET.md` are the
parity ledger; the `docs/status/*` matrices are not updated per fork task.

- A request timeout fails a receipt in any status except `Receiving`, the
  packet form in `Sent` included, with `RequestError::Timeout`. Python only
  fails a receipt that reached `DELIVERED` (`RNS/Link.py:1398`
  `request_timed_out`, `:1408` `response_rejected`), so a never-answered
  packet-form request stays pending forever there.
- A packet-form response over the requester's `max_response_size` fails the
  receipt with `RequestError::Oversize`. Python rejects the response
  (`RNS/Link.py:874-876`) but the `DELIVERED` guard in `:1408` leaves that
  receipt pending.
- A response Resource that fails in transfer (a part failure, receiver retry
  exhaustion, a remote cancel) fails the receipt with
  `RequestError::Receive` in every status. Python routes the failed Resource
  through `request_timed_out` (`RNS/Link.py:914-918`), whose `DELIVERED`
  guard (`:1398`) fails only a receipt no progress callback has yet moved
  to `RECEIVING` (`:1420`); a receipt already `RECEIVING` stays there with
  the failure visible only through the Resource's own callback.
- After an over-limit response packet the receipt's `response_size()` and
  `response_transfer_size()` are left as they were. Python runs
  `update_sizes` before rejecting (`RNS/Link.py:868-871`).
- A packet-form `response_size()` is the exact packed body length. Python
  records `len(packb(body)) - 2` (`RNS/Link.py:1016-1017`).
- An in-limit response packet for a receipt already `Receiving` a response
  Resource resolves the receipt `Ready` from the packet, as Python does
  (`RNS/Link.py:857-866` `handle_response`, `:873`, `:1437-1443`), and
  cancels the response Resource still in flight with a receiver cancel on
  the link. Python lets that Resource run to completion and then discards
  it, since the pending request it would have answered is already gone
  (`:897-912`). The same holds for a response record no receipt owns: the
  packet that answers it cancels the Resource the record was receiving.
- A response record no receipt owns, for a request sent with
  `Link::request_packet*` or `Transport::send_request_resource*`, parks in
  `Receiving` between response segments with no clock; only the inbound
  Resource's own outcome clears it. Python's marker is the `RequestReceipt`,
  and a `RECEIVING` receipt is never timed out either (`RNS/Link.py:1389`),
  but Python has no receipt-less form.
- The next segment of a multi-segment response re-points the receipt, and
  the receipt-less record, at the new segment's hashes without cancelling
  an earlier segment's partial assembly. Python sets `receipt.resource` once
  in the constructor (`RNS/Link.py:1328`) and never re-points it (`:1055`).
- The receipt-less record follows the receipt clock. Python's solicitation
  marker is the `RequestReceipt`: a `DELIVERED` one past its timeout leaves
  `pending_requests` (`RNS/Link.py:1397-1405`), a `RECEIVING` one never does
  (`:1389`), and one that never reached `DELIVERED` stays, so a packet-form
  marker in Python can outlive the record here.
- A resource-form request whose Resource is proved while a response Resource
  is already being received stays `Receiving`. Python sets `DELIVERED`
  unconditionally (`RNS/Link.py:1372`).
- A legacy-length request id, one that is not 16 bytes, is not size-gated
  on the packet Response path. Python keys responses on the 16-byte receipt
  id alone (`RNS/Link.py:857-884`).
- An assembled response Resource is measured against the requester's
  `max_response_size` once more on completion and withheld if over. Python
  trusts the advertised size (`RNS/Link.py:1049`).
- `Transport::request` on a link that is not `Active`, `Stale` included, is
  refused with `RnsError::ConnectionError`. Python drops a packet only on a
  `CLOSED` link (`RNS/Packet.py:294-297`).
- `LinkEvent::PacketProven` is gated on the per-link record of sent plain
  Data packet hashes; `DeliveryReceipt` is not gated and fires for every
  validly signed link proof, replays and Channel frames included. Python
  consumes the one `SENT` receipt the proof validates
  (`RNS/Transport.py:2748-2761`).
- The sent-hash record keeps the latest 1024 hashes per link, oldest
  dropped, with no time-based cull. Python culls transport-wide at
  `MAX_RECEIPTS` (`RNS/Transport.py:159`, `:745-748`) and also times a link
  receipt out at `max(rtt * traffic_timeout_factor, TRAFFIC_TIMEOUT_MIN_MS /
  1000)` seconds (`RNS/Packet.py:428`).
- Only plain Data packets (`PacketContext::None`) are recorded and proved.
  Python records a `PacketReceipt` for every `DATA` packet outside the
  link-control and Resource contexts, request and response packets included
  (`RNS/Transport.py:1359-1377`).
- The link always proves plain Data packets. Python proves only under
  `PROVE_ALL` or `PROVE_APP` (`RNS/Link.py:961-968`) and its default is
  `PROVE_NONE` (`RNS/Destination.py:161`), so a stock Python peer never
  sends `PacketProven` here.
- A retried Resource advertisement re-sends the identical prepared packet,
  so the receiver's duplicate filter drops it and the
  `ResourceAcceptHook` runs once per advertisement. Python builds a fresh
  packet per retry (`RNS/Resource.py:597-598`) and re-runs `ACCEPT_APP`.
- Without a `ResourceAcceptHook` every plain advertisement is accepted,
  Python's `ACCEPT_ALL` (`RNS/Link.py:1075-1076`). Python's default is
  `ACCEPT_NONE` (`RNS/Link.py:239`, `:1065`), which has no equivalent here;
  a hook that always returns `false` is the closest.
- Every announce, path responses included, reaches
  `Transport::recv_announces()` flagged with `is_path_response`. Python's
  default announce handler drops path responses unless it sets
  `receive_path_responses` (`RNS/Transport.py:2500-2504`).
- Inherited from upstream, surfaced by `OutboundFailure::RetriesExhausted`:
  the sender counts its retry budget in `Transferring` too, one retry per
  retry interval without a part request, and fails with `RetriesExhausted`
  when it runs out. Python's sender waits one RTT-scaled `max_wait` there
  with no retries, then cancels (`RNS/Resource.py:643-650`); its
  `retries_left` budget applies while advertised (`:588-590`) and while
  awaiting the proof (`:659-661`).
- `attach_shared_instance` does not force transport, remote management or
  probes off as Python does (`RNS/Reticulum.py:438-440`): the caller keeps
  `transport_enabled` as configured. And the client synthesizes a tunnel at
  attach and again on every reconnect, which Python's `LocalClientInterface`
  never does (`RNS/Interfaces/LocalInterface.py` has no tunnel code;
  `wants_tunnel` is set by network-facing clients such as
  `RNS/Interfaces/TCPInterface.py:179`).
- `attach_shared_instance` refuses a `Tcp` endpoint whose address is not
  loopback by default and attaches it only under
  `SharedInstanceOptions::allow_remote_endpoint(true)`, a fork extension:
  Python's `LocalClientInterface` connects to `127.0.0.1` only and has no
  remote option at all (`RNS/Interfaces/LocalInterface.py:109`).
- While attached, the duplicate filter is skipped only for packets arriving
  on the shared-instance interface (or everywhere when no interface is
  flagged); the transport's other interfaces keep filtering, where a Python
  client brings up no other interface at all (`RNS/Reticulum.py:745`), so
  Python filters nothing in that mode (`RNS/Transport.py:1618-1627`).
- The reverse-routing source notes live in the same cache as the duplicate
  filter, so a Data packet received while attached to a shared instance is
  treated as seen after a detach. Python's `add_packet_hash` guard
  (`RNS/Transport.py:1618-1627`) keeps `packet_hashlist` empty of it, since
  `reverse_table` is a separate dict there, and the shared-instance drop
  flush keeps those notes' hashes in the cache too.
- The TCP shared-instance client redials at once after a drop that ended a
  connection of at least `MIN_STABLE_CONNECTION` (2 s), and after the first
  short-lived one; Python's `LocalClientInterface` sleeps `RECONNECT_WAIT`
  before every redial (`RNS/Interfaces/LocalInterface.py:159-160`). The
  unix client follows Python.
- Inherited from upstream, made visible by the `Handshake` LINKCLOSE
  widening: a responder-side link becomes `Active` when it proves
  (`destination/link_parts/link_sections/new.rs:137-138`, called from
  `transport/path.rs:372` before the `in_links` insertion at `:383`), so no
  link of this crate is ever observable in `Handshake` from the tables.
  Python's responder stays `HANDSHAKE` after `validate_request` runs
  `handshake()` and `prove()` (`RNS/Link.py:186-213`, the status set at
  `:350`) and `register_link` files it under `active_links` in that status
  (`RNS/Transport.py:2928-2937`) until the RTT packet makes it `ACTIVE`
  (`RNS/Link.py:516-523`). So Python's drop flush can tear down a
  `HANDSHAKE` responder link and send its LINKCLOSE (`RNS/Link.py:668`),
  where this crate's drop flush finds only `Active` ones; the widened
  `Link::teardown` sends the packet for a `Handshake` link a caller holds
  outside the tables.
- `AutoInterfaceConfig::unicast_discovery_port` steps down to 65534 when
  `discovery_port` is 65535, so the pair stays adjacent and distinct; Python
  computes `discovery_port + 1` (`RNS/Interfaces/AutoInterface.py:173`) and
  its unicast bind fails there.

Not divergences:

- `RejectReason::PeerCancelled` is the only reason produced: a receiver
  cancel carries only the resource hash (`RNS/Resource.py:155-165`), so the
  sender never learns the motive. API shape, not behaviour.
- `is_path_response` repeats the peer's packet context
  (`RNS/Transport.py:2524-2528`) and is not matched against any path request
  this transport sent: information, not a check.
- A Channel frame is proved (`RNS/Link.py:1147-1151`) but posts no
  `PacketProven`: its hash is never recorded. A fact of this crate's API.
- The 1024-slot request dispatch queue is a resource limit; Python has no
  bound (`RNS/Link.py:1004-1005`).
- The resource-form request clock anchored at `Delivered` and never
  expiring while `Receiving` is parity (`RNS/Link.py:1373`, `:1389`).

## Versioning and releases

The root `VERSION` file is the workspace number: the upstream baseline that
`lxmf --version` prints and that HIL evidence records. The fork publishes only
`coyote-rns-transport`, so this crate's version moves independently of it and
root `VERSION` stays at the upstream baseline until the next upstream rebase.

Release notes live in this crate's `CHANGELOG.md`. The per-release ledger of
every file changed since the previous release, with the commit that changed
it, is `docs/fork/PATCHSET.md`. A release is published by pushing the tag
`coyote-rns-transport-v<version>`: `tools/scripts/check-publish-tag-version.sh`
checks that the tag matches the crate version, taking the tag bare or as the
full `refs/tags/coyote-rns-transport-v<version>` ref, and
`.github/workflows/publish-coyote-rns-transport.yml` runs the publish.

## Patch list

`docs/fork/PATCHSET.md` is the single per-release ledger of every file changed
since the previous release, with the commit that changed it and the upstream
site; earlier sections carry the earlier patches. What `0.17.0` carries on top
of `0.16.0`:

- The destination lookup: `Transport::destination_desc` returns
  `Option<DestinationDesc>` for an announced destination, `Some` exactly when
  `destination_identity` is. The desc's name is the announce's truncated name
  hash as `DestinationName::new_from_hash_slice` stores it, so callers
  compare names with `as_name_hash_slice()`. The entry survives `expire_path`
  and is pruned only by `deregister_destination`; a restored path table fills
  it too.
- The responder outcome: `InboundRequest::track_outcome` returns a
  `ResponseOutcomeReceiver` that resolves with a `ResponseOutcome`, one of
  `Packet`, `Resource(OutboundResult)` or `NotSent`. `NotSent` covers every
  path that drops the sender: a silent handler, a link no longer able to
  exchange data, a stop or cancel under the response, a handler that panicked
  after tracking. Opt-in; a handler that never calls it sees no change.
- The shared-instance state events:
  `SharedInstanceOptions::with_state_events(Sender<SharedInstanceEvent>)`
  delivers the `#[non_exhaustive]` `Connecting`, `Connected` and
  `Disconnected` events with a non-blocking `try_send`, a full or closed
  channel dropping the event at debug; a drop or connect already queued when
  the attach ended is still drained and served. Underneath,
  `TcpClient::with_connect_events` and `LocalUnixClient::with_connect_events`
  with their `connect_events_enabled` getters fire on every established
  stream, the first included; `tests/connect_events_api.rs` drives both
  clients that way.
- For the consumer parsing hashes: `AddressHash::new_from_hex_string` has
  decoded its input with `hex::decode` and returned `RnsError::IncorrectHash`
  on any malformed string since 0.14.1 (that release's `### Fixed` in
  `CHANGELOG.md`); nothing new was needed here.
- Test-only hygiene: the loopback connector, the interop request-handler
  record and the hand-driven shared-instance consumer each share one helper
  in place of a copy per test file, three outcome probe tests that repeated a
  sibling's pin are gone, and the test-only `get_handler` accessor lives in
  `transport/tests.rs`.

What `0.16.0` carries on top of `0.15.1`:

- The loopback-only shared-instance default: `attach_shared_instance` refuses
  a non-loopback `Tcp` endpoint with
  `RnsError::SharedInstanceEndpointNotLoopback` unless
  `SharedInstanceOptions::allow_remote_endpoint(true)` opts in, which
  replaces the 0.15.1 once-per-attach warning (the opt-in keeps the warning,
  in key=value form).
- The removals: `RejectReason::{Oversize, Unknown}` and the storage-feature
  `transport::test_bridge` module, with the migrations under `### Breaking`
  in `CHANGELOG.md`.
- The per-interface duplicate-filter gate: the filter is skipped for packets
  arriving on the shared-instance interface (everywhere when no interface is
  flagged), replacing the 0.15.1 rule that it consulted nothing while
  attached.
- The LINKCLOSE a `Handshake` teardown now sends.
- The reticulumd migration: the daemon attaches through
  `attach_shared_instance` (a loopback-first host pick in the family
  `prefer_ipv6` selects, the ignored `connect_timeout` and
  `max_reconnect_tries` keys warned about and stripped from the reference TOML).
- `clippy::await_holding_invalid_type` live at its default level across the
  workspace; the remaining production holds carry fn-level allowances whose
  comments name the guard and the await, and the classify scripts gate them.
- Release tooling: the four `tools/scripts/*.sh` release checks with their
  contract probes, the `coyote-rns-transport-v*` tag arm of the xtask
  bundle's version check, and the removal of the Codex review workflow.
- Carried hygiene: in-limit Response packets cancel the overtaken Resource
  for receipt-less records too, the packet hashlist flush writes a unique
  temp file and sweeps orphans on load, `TcpClient` connect is cancel-aware,
  inbound resource lookups are link-scoped, and the request-layer send
  wrappers refuse a non-Active link.

What `0.15.1` carries on top of `0.15.0`:

- The shared-instance client: `Transport::attach_shared_instance` and
  `detach_shared_instance`, `SharedInstanceEndpoint` and
  `SharedInstanceOptions`, `announce_destinations_to_shared_instance`, the
  path-response announce of a destination registered while attached, the
  re-announce after a reconnect, and the LINKCLOSE teardown and table flush
  when the connection drops.
- `TcpClient::with_reconnect_wait` and the matching redial wait in
  `LocalUnixClient`, both honoured by `attach_shared_instance`.
- The duplicate filter inserts nothing and consults nothing while attached,
  as Python's `add_packet_hash` guard keeps `packet_hashlist` empty in that
  mode; the `## Known divergences from Python RNS` entry on the reverse-routing
  source notes describes what the cache still records.
- A warning, once per attach, when a `Tcp` shared-instance endpoint is not
  loopback.
- The live interop test that attaches to a Python `rnsd` shared instance and
  exchanges requests both ways.
- The usage example under `## Attaching to a shared instance`, pinned
  verbatim by `tests/shared_instance_api.rs`.
- Release hygiene: ci-full runs the fork contracts and the compile-fail
  harness as PR CI does, the publish tag guard takes the full `refs/tags/`
  form, and `ResourceManager::conclude_outbound` logs a dropped outcome
  receiver instead of discarding the send.

What `0.15.0` carries on top of `0.14.1`:

- The link request layer: the `RequestFrame` and `ResponseFrame` wire frames,
  request handlers with allow policies and size caps, `Transport::request`
  with its receipts, timeouts and response-size limits, the packet and
  Resource forms of a request and a response, and the dispatch worker that
  runs the handlers.
- The link proof event `LinkEvent::PacketProven` and
  `DeliveryReceipt::link_id`.
- Outbound Resource outcomes: `send_*_with_outcome`, `OutboundOutcome` and
  `RejectReason`.
- `Transport::link` returns `Result` and fails fast with `RnsError::NoPath`
  when no next hop is known and broadcast is off.
- `AnnounceEvent::is_path_response`.
- The resource accept hook, `Transport::set_resource_accept_hook`.
- The fuzz crate for the new decoders.
- The Python-in-the-loop interop tests.
- Release hygiene: the Codex review workflow gated on the repository
  variable `CODEX_REVIEW_ENABLED` and skipped for fork-originated pull
  requests, `fork_publish_contract` and `fork_release_docs_contract` run in
  PR CI, the old-name walk skipping dot-directories, and RNS citations on
  every public request-layer and outcome item. The Codex workflow was
  removed after 0.15.1; the crate CHANGELOG records it under `### Removed`
  in 0.16.0.

The `0.14.1` patches (link-close stdout, bound-interface segment dispatch,
reject outside the link lock, sender before advertisement, hex parsers,
non-fatal announce, advert cap, additive API, lint tables) are carried
unchanged and listed in the `0.14.1` section of `docs/fork/PATCHSET.md`.

## Rebase recipe

1. Add the upstream remote once:
   `git remote add upstream https://github.com/FreeTAKTeam/LXMF-rs.git`
2. `git fetch upstream --tags`
3. Merge or rebase `main` onto the upstream release tag.
4. Re-apply every entry in the patch list and re-run
   `cargo test -p coyote-rns-transport --lib no_stdout_prints_tests`.
5. Re-apply the rename wherever upstream added new sites:
   `grep -rn reticulum-rs-transport . --exclude-dir=target --exclude-dir=.git | grep -v Cargo.lock`
   shows the candidates, and the authoritative check is
   `cargo test -p reticulumd --test fork_publish_contract`, whose
   `old_package_name_survives_only_as_prose` scan fails on every runnable form
   of the old name (`-p`, `--package`, `package =`, `name =`, dependency and
   crates.io URL spellings). Prose that explains the relationship to upstream
   and the needle literals inside the test itself are allowed; the only docs
   that may keep the old commands are the historical subtrees the test skips
   (`docs/goals/`, `docs/migrations/`, `docs/adr/`, `docs/release-notes-*`).
   `tools/scripts/old-name-tracked-scan.sh` is the tracked-files-only form of
   that scan (same needles, extension and doc-prefix filters; the test also
   scans committable untracked files and skips dot-directories other than
   `.github`).
6. Bump the version at every site: `version` in this crate's `Cargo.toml`,
   the workspace `[workspace.dependencies] rns-transport` entry,
   `crates/apps/rns-tools/Cargo.toml` and `crates/apps/reticulumd/Cargo.toml`
   (`package = "coyote-rns-transport", version = ...`), the consumer line in
   this README and in `docs/getting-started.md`, and the version pins in
   `crates/apps/reticulumd/tests/fork_publish_contract.rs`. Regenerate
   `Cargo.lock` (`cargo update -p coyote-rns-transport --offline`) and the
   fuzz crate's own `crates/libs/rns-transport/fuzz/Cargo.lock` (the same
   command with `--manifest-path crates/libs/rns-transport/fuzz/Cargo.toml`),
   and add the release headings to `CHANGELOG.md` and `docs/fork/PATCHSET.md`.
7. `cargo check --workspace && cargo test -p coyote-rns-transport && cargo test -p reticulumd --test fork_publish_contract --test fork_release_docs_contract && tools/scripts/check-boundaries.sh && tools/scripts/workspace-clippy-classify.sh && cargo publish -p coyote-rns-transport --locked --dry-run`
   `workspace-clippy-classify.sh` (needs `python3`) refuses any tracked
   manifest that mentions `await_holding_invalid_type`, replays CI's clippy
   invocation and passes only when every error-level record is, on macOS
   only, the `dead_code` of the interop test target whose linux-only cases
   are cfg'd out there.
   `tools/scripts/semver-major-findings-check.sh` records the
   `cargo-semver-checks` verdict: `... <previous-tag> 0` for a patch bump,
   `... <previous-tag> <expected-major-count> [lint ...]` for a minor bump.
8. Commit; cite the bump commit's hash in a ledger-only follow-up commit and
   run `tools/scripts/patchset-ledger-check.sh <previous-tag> <version> <previous-version>`
   (usage `<base-sha> <label> [<previous-version> [<head>]]`), which checks
   that the section's file table equals `git diff --name-only` from that tag
   (deletions and `plans/` excluded) and that every non-ledger commit in the
   range is cited in the section; PR CI runs the zero-argument form. Then
   either tag `coyote-rns-transport-vX.Y.Z` and push the tag, or
   run the `publish-coyote-rns-transport` workflow by hand from the Actions
   tab, setting the `ref` input to the tag; the dropdown choice alone does not
   select what is published. The repository secret `CARGO_REGISTRY_TOKEN`
   must exist for the publish step. A manual run from a ref that is not a
   release tag skips the tag-matches-version check and publishes whatever
   version that ref carries; publish from the tag when you can. The workflow
   runs without a GitHub deployment environment; if you want required
   reviewers or tag-only deployment branches, create an environment named
   `crates-io` in the repository settings and add `environment: crates-io`
   to the `publish` job.

## Licence

Same as upstream: `EPL-2.0 OR GPL-2.0-or-later`.
