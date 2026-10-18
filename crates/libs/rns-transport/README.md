# coyote-rns-transport

## What this is

Coyote's published fork of the upstream `reticulum-rs-transport` crate from
[LXMF-rs](https://github.com/FreeTAKTeam/LXMF-rs): the transport, link,
interface, receipt, and resource layers of the Reticulum network stack.

Apart from the patch list below, only the package name changes: the library
target stays `rns_transport`, so every `use rns_transport::...` path written
against upstream compiles unchanged against this crate.

The version follows cargo semver relative to the upstream baseline. `0.14.0`
is upstream `0.14.0` renamed, with the link-close patch applied. Fork patches
that do not break the public API (additive API, internal fixes) bump the patch
digit; a fork change that breaks the public API bumps the minor digit. When
upstream ships its next release the fork is rebased onto it and takes that
version; if upstream itself ships a `0.14.x` patch release, the fork rebases
onto it and takes the next unused patch number.

## Consumer

```toml
[dependencies]
coyote-rns-transport = "0.14.1"
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

`RequestAllow::List` answers only a peer that identified itself over the link
with one of the listed identity hashes; an unidentified peer is denied
silently. `None` from the handler sends no response at all. A response that
fits the link MDU goes out as one packet; a larger one travels as a response
Resource and reaches the requester through `Transport::resource_events()` and
resolves its `RequestReceipt`.
Each handler runs on a task of its own with no concurrency cap, so a slow
handler holds up only its own request; a handler that needs a bound holds its
own `Semaphore`. A request larger than the destination's `max_request_size`
is ignored before any handler runs; a request packet over the limit is dropped
silently.

A request too large for one packet arrives as a request Resource; once it is
assembled the same handler runs, with the request id recomputed from the
assembled bytes. A `max_request_size` is enforced against the advertised data
size before any part is accepted: an oversize request Resource is rejected at
its advertisement, which the requester sees as
`ResourceEventKind::OutboundRejected` and as `RequestError::Rejected` on its
receipt. On the response side, a `Value` or
`Bytes` that outgrows the link MDU travels as a Resource whose data is the
whole `[request_id, body]` envelope; a
`RequestResponse::Resource { data, metadata }` always travels as a Resource
whose data is the raw `data`, with the packed metadata ahead of it. A Python
requester reads a response Resource as a file response only when it carries
metadata (`RNS/Link.py:902-912`), so a `Resource { metadata: None }` answer is
unpacked as an envelope there and is only for a peer that expects raw bytes.
Both advertise the request id in `q` with the response flag `p` set, and both
honour the registration's `auto_compress`; a response packet is never
compressed.

`LinkEvent::Data` with `PacketContext::Request` keeps posting for every
inbound request packet, and `ResourceEventKind::Complete` with `is_request`
for every assembled request Resource; a request whose path has a registered
handler is answered by that handler, and broadcast consumers must not answer it.

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

The receipt is `Clone`; `response()` resolves on any clone once the request
is `Ready` or `Failed`, and `subscribe()` reports the status as it moves. A
packet-form request goes `Sent -> Ready`, or `Sent -> Receiving -> Ready`
when the response is a Resource; a resource-form request reaches `Delivered`
when its Resource completes. The timeout defaults to `rtt * 6 + 11.25 s` and
`RequestOptions::with_timeout` overrides it; the clock runs from the send for
a packet-form request and from `Delivered` for a resource-form one, and a
receipt still waiting when it runs out fails with `RequestError::Timeout`.
`RequestOptions::with_max_response_size` is measured against
the uncompressed size of the response, the packed body of a response packet
or the advertised data size of a response Resource; a response Resource over
the limit is refused at its advertisement, a response packet over it is
dropped, and a response Resource that assembles over it (the advertised size
was the peer's claim) is failed after assembly; each of the three fails the
receipt with `RequestError::Oversize`. Every request packet the link sends,
however it was built, and every Resource sent through `send_request_resource*`
solicits its response; a response Resource advertised for a request nothing on
the link sent is dropped without a reject. The solicitation lasts the default
request timeout, counted from the send for a request packet and from the
request Resource's delivery for `send_request_resource*`, unless a response
Resource is being received, in which case that Resource's own outcome ends it.
An id a `Transport::request` receipt owns is solicited for the receipt's own
timeout instead, default or not.
No clock runs while a request Resource is in flight or a response Resource
is being received: a request Resource that is refused fails `Rejected`, one
that cannot be sent, exhausts its retries or is cancelled through
`Transport::cancel_resource` fails `Send`, and a response Resource that
fails in transfer fails `Receive`. A link that closes, from either side,
fails every pending receipt with `RequestError::LinkClosed`, as does
`Transport::stop()` within its budget; a request on a link that is not
`Active` (`Stale` included) is refused. A response Resource with metadata is
a file response, read through `RequestResponseData::data()` and `metadata()`;
every other response is the envelope body, read through `value()`. Dropping
every clone of a receipt does not cancel the request: the responder still
serves it and the response still reaches the link's broadcasts, unless the
receipt's `max_response_size` refuses it: an over-limit packet Response is
not posted as `LinkEvent::Data`, and a response Resource that assembles over
it has its `Complete` withheld.

## Delivery proofs on a link

A plain Data packet sent on a link is proved by the peer (`RNS/Link.py:950-962`),
and the proof posts `LinkEvent::PacketProven { packet_hash }` on the link's
event stream, on whichever side sent the packet, once per packet: the hash is
recorded when the packet is sent and the first valid proof naming it consumes
the record, the way Python's proof removes the one outstanding receipt it
validates (`RNS/Transport.py:2748-2761`). A link remembers at most 1024
unproved sent packets, oldest dropped first (`Transport.MAX_RECEIPTS`,
`RNS/Transport.py:159`), so a proof for a dropped record posts nothing. A
Python peer proves link packets
only when its destination's proof strategy is `PROVE_ALL` or `PROVE_APP`
(`RNS/Link.py:961-968`); its default is `PROVE_NONE`, while this crate always
proves. Requests, responses and Resource packets are never proved, and a
Channel frame, though proved, posts no `PacketProven`. Every validly signed
link proof, those included, reaches the transport's `ReceiptHandler` as a
`DeliveryReceipt` whose `link_id()` names the link; the receipt is not
filtered by the record. The request layer's own `RequestReceipt` is the delivery
signal for a request, and only a resource-form request ever reports
`Delivered`.

## Versioning and releases

The root `VERSION` file is the workspace number: the upstream baseline that
`lxmf --version` prints and that HIL evidence records. The fork publishes only
`coyote-rns-transport`, so this crate's version moves independently of it and
root `VERSION` stays at the upstream baseline until the next upstream rebase.

Release notes live in this crate's `CHANGELOG.md`. The per-release ledger of
every file changed since the previous release, with the commit that changed
it, is `docs/fork/PATCHSET.md`. A release is published by pushing the tag
`coyote-rns-transport-v<version>`: `tools/scripts/check-publish-tag-version.sh`
checks that the tag matches the crate version and
`.github/workflows/publish-coyote-rns-transport.yml` runs the publish.

## Patch list

`docs/fork/PATCHSET.md` is the single per-release ledger of every file changed
since the previous release, with the commit that changed it and the upstream
site; earlier sections carry the earlier patches. What `0.14.1` carries on top
of upstream `0.14.0`:

- Link close no longer prints to stdout: the `println!` in
  `Link::finalize_local_close` is removed and the `log::warn!` on the next line
  carries the information. `tools/scripts/python-lxmd-rust-lxmd-smoke.sh`
  waits for the warn's own message, `close /<32 hex link id>/`. When
  re-applying keep that `warn!` and keep its message starting with `close `
  followed by the link id.
- Later Resource segments and Resource retries are dispatched on the link's
  bound interface instead of through the path table, so a `Transport` built
  with broadcast off no longer drops segments 2 and later as `DroppedNoRoute`.
- A Resource advertisement reject is sent after the link lock is released;
  sending it under the lock deadlocked the transport on the first oversize
  request or response advertisement.
- An outbound Resource sender is registered before its advertisement is
  written, so a fast peer's first part request is served; the
  `pending_outgoing` staging map is gone.
- The hex parsers (`AddressHash`, `Identity`, `PrivateIdentity`) return
  `IncorrectHash` on multibyte input instead of panicking.
- `Transport::send_announce` logs and returns instead of panicking when the
  announce cannot be built; `try_send_announce` returns the error.
- An inbound Resource advertisement above the reference cap of
  `3 * MAX_EFFICIENT_SIZE` is dropped before any receiver state is created
  (`RNS/Resource.py:1374`).
- Additive API: `Transport::stop`, `Link::rtt`,
  `destination_hash::destination_address`, `AddressHash::short`, and the
  `rmpv` and `rand_core` re-exports from the crate root.
- Workspace clippy configuration names the tokio guard types under
  `await-holding-invalid-types` (`clippy.toml`, the workspace and
  `reticulumd` `[lints]` tables); see the CHANGELOG `### Changed` bullet.

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
   `old_package_name_survives_only_as_prose` walk fails on every runnable form
   of the old name (`-p`, `--package`, `package =`, `name =`, dependency and
   crates.io URL spellings). Prose that explains the relationship to upstream
   and the needle literals inside the test itself are allowed; the only docs
   that may keep the old commands are the historical subtrees the test skips
   (`docs/goals/`, `docs/migrations/`, `docs/adr/`, `docs/release-notes-*`).
6. Bump the version at every site: `version` in this crate's `Cargo.toml`,
   the workspace `[workspace.dependencies] rns-transport` entry,
   `crates/apps/rns-tools/Cargo.toml` and `crates/apps/reticulumd/Cargo.toml`
   (`package = "coyote-rns-transport", version = ...`), the consumer line in
   this README and in `docs/getting-started.md`, and the version pins in
   `crates/apps/reticulumd/tests/fork_publish_contract.rs`. Regenerate
   `Cargo.lock` (`cargo update -p coyote-rns-transport --offline`) and add the
   release headings to `CHANGELOG.md` and `docs/fork/PATCHSET.md`.
7. `cargo check --workspace && cargo test -p coyote-rns-transport && cargo test -p reticulumd --test fork_publish_contract && tools/scripts/check-boundaries.sh && cargo publish -p coyote-rns-transport --locked --dry-run`
8. Commit, then either tag `coyote-rns-transport-vX.Y.Z` and push the tag, or
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
