# coyote-rns-transport patch set

One section per published fork release of `coyote-rns-transport`. Each
section names the upstream `reticulum-rs-transport` release the fork build is
based on, opens with a table of every file changed since the previous release
and the patch each change belongs to, and then lists every patch applied on
top of upstream, with the upstream site of the change and the fork commits
that introduced it. The package rename (`reticulum-rs-transport` to
`coyote-rns-transport`) and the pure rename edits to workspace manifests,
scripts, CI workflows and docs are not patches in this sense and are not
listed; lint-table changes that bind contributors (`clippy.toml` and the
manifest `[lints]` tables) are. Each patch header lists every commit a rebase
must re-apply: every commit that touches a file other than this ledger and
the crate `CHANGELOG.md` (code, tests, fixtures, scripts, doc comments).
Commits that only update this ledger are omitted. The crate README
(`crates/libs/rns-transport/README.md`) carries the versioning policy, the
rebase recipe and a prose summary of the current patch list; this file is
the per-release ledger of the same patches, with commit hashes and file
sites, so a rebase can re-apply them one commit at a time.

## Unreleased (upstream 0.14.0)

Every file changed since the `0.14.1` release and the patch each change
belongs to. Patch ids: F1 is request wire frames and fuzz. The
`request wire frames and fuzz` patch is fork commits `8e9970ac`,
`afc56836`, `71365159`, `f875aff7` and `30d8dfde`; the request handlers
and dispatch patch carries no other id and is fork commits `08a5cff6`,
`21ab1981`, `c1a98ff1`, `12d2080c`, `7bbb0197`, `73f177b4`, `1fd1192a`,
`febec3db`, `7e9e5904`, `5544c49c`, `816032c5` and `c82dcd0d`; the
request receipts and timeouts patch carries no other id and is fork
commits `1ddd5273`, `a2a40de7`, `3805fe7f`, `3ed78582`, `415d67c8`,
`afdf0f18`, `fe55cb71`, `10dc355e`, `8909d956`, `642dfc2b`, `7ce1601c`,
`40409738`, `2f2c11c7` and `140a4fd2`.

| File | Patch |
| --- | --- |
| `crates/apps/reticulumd/tests/fuzz_campaign_contract.rs` | request wire frames and fuzz |
| `crates/libs/rns-transport/CHANGELOG.md` | request wire frames and fuzz, request handlers and dispatch, request receipts and timeouts |
| `crates/libs/rns-transport/README.md` | request handlers and dispatch, request receipts and timeouts |
| `crates/libs/rns-transport/fuzz/.gitignore` | request wire frames and fuzz |
| `crates/libs/rns-transport/fuzz/Cargo.lock` | request wire frames and fuzz |
| `crates/libs/rns-transport/fuzz/Cargo.toml` | request wire frames and fuzz |
| `crates/libs/rns-transport/fuzz/fuzz_targets/request_frame_decode.rs` | request wire frames and fuzz |
| `crates/libs/rns-transport/fuzz/fuzz_targets/resource_advertisement_decode.rs` | request wire frames and fuzz |
| `crates/libs/rns-transport/fuzz/fuzz_targets/response_frame_decode.rs` | request wire frames and fuzz |
| `crates/libs/rns-transport/src/destination.rs` | request wire frames and fuzz |
| `crates/libs/rns-transport/src/destination/link_parts/link.rs` | request handlers and dispatch, request receipts and timeouts |
| `crates/libs/rns-transport/src/destination/link_parts/link_sections/identify_packet_tests.rs` | request handlers and dispatch |
| `crates/libs/rns-transport/src/destination/link_parts/link_sections/new.rs` | request handlers and dispatch, request receipts and timeouts |
| `crates/libs/rns-transport/src/destination/link_parts/link_sections/new_from_request.rs` | request handlers and dispatch, request receipts and timeouts |
| `crates/libs/rns-transport/src/destination/link_parts/link_sections/next_watchdog_deadline.rs` | request receipts and timeouts |
| `crates/libs/rns-transport/src/destination/link_parts/link_sections/pending_requests.rs` | request receipts and timeouts |
| `crates/libs/rns-transport/src/destination/link_parts/link_sections/pending_requests_tests.rs` | request receipts and timeouts |
| `crates/libs/rns-transport/src/destination/link_parts/link_sections/request_dispatch.rs` | request handlers and dispatch |
| `crates/libs/rns-transport/src/destination/link_parts/link_sections/request_dispatch_tests.rs` | request handlers and dispatch |
| `crates/libs/rns-transport/src/destination/link_parts/link_sections/request_payload.rs` | request wire frames and fuzz |
| `crates/libs/rns-transport/src/destination/link_parts/linkhandleresult.rs` | request handlers and dispatch, request receipts and timeouts |
| `crates/libs/rns-transport/src/destination/link_parts/module_prelude.rs` | request handlers and dispatch, request receipts and timeouts |
| `crates/libs/rns-transport/src/destination/request/allow.rs` | request handlers and dispatch |
| `crates/libs/rns-transport/src/destination/request/dispatch.rs` | request handlers and dispatch |
| `crates/libs/rns-transport/src/destination/request/frame.rs` | request wire frames and fuzz |
| `crates/libs/rns-transport/src/destination/request/handler.rs` | request handlers and dispatch |
| `crates/libs/rns-transport/src/destination/request/id.rs` | request wire frames and fuzz |
| `crates/libs/rns-transport/src/destination/request/inbound.rs` | request handlers and dispatch |
| `crates/libs/rns-transport/src/destination/request/mod.rs` | request wire frames and fuzz, request handlers and dispatch, request receipts and timeouts |
| `crates/libs/rns-transport/src/destination/request/receipt.rs` | request receipts and timeouts |
| `crates/libs/rns-transport/src/destination/request/receipt_pending.rs` | request receipts and timeouts |
| `crates/libs/rns-transport/src/destination/request/receipt_tests.rs` | request receipts and timeouts |
| `crates/libs/rns-transport/src/destination/request/tests.rs` | request wire frames and fuzz |
| `crates/libs/rns-transport/src/destination/tests.rs` | request handlers and dispatch |
| `crates/libs/rns-transport/src/destination_parts/destination.rs` | request handlers and dispatch |
| `crates/libs/rns-transport/src/destination_parts/module_core.rs` | request handlers and dispatch |
| `crates/libs/rns-transport/src/destination_parts/module_prelude.rs` | request handlers and dispatch |
| `crates/libs/rns-transport/src/resource/manager_cleanup.rs` | request receipts and timeouts |
| `crates/libs/rns-transport/src/resource/tests.rs` | request wire frames and fuzz |
| `crates/libs/rns-transport/src/resource/tests_advert_kat.rs` | request wire frames and fuzz |
| `crates/libs/rns-transport/src/resource/tests_timeouts_lifecycle.rs` | request receipts and timeouts |
| `crates/libs/rns-transport/src/transport/core.rs` | request handlers and dispatch |
| `crates/libs/rns-transport/src/transport/jobs.rs` | request handlers and dispatch, request receipts and timeouts |
| `crates/libs/rns-transport/src/transport/jobs_parts/link_maintenance.rs` | request receipts and timeouts |
| `crates/libs/rns-transport/src/transport/lifecycle.rs` | request receipts and timeouts |
| `crates/libs/rns-transport/src/transport/links_parts/transport.rs` | request receipts and timeouts |
| `crates/libs/rns-transport/src/transport/links_parts/transport_sections/request.rs` | request receipts and timeouts |
| `crates/libs/rns-transport/src/transport/links_parts/transport_sections/reset_out_link.rs` | request handlers and dispatch |
| `crates/libs/rns-transport/src/transport/mod.rs` | request handlers and dispatch |
| `crates/libs/rns-transport/src/transport/path.rs` | request handlers and dispatch |
| `crates/libs/rns-transport/src/transport/request_dispatch.rs` | request handlers and dispatch |
| `crates/libs/rns-transport/src/transport/resource_wire.rs` | request handlers and dispatch, request receipts and timeouts |
| `crates/libs/rns-transport/src/transport/test_bridge.rs` | request handlers and dispatch |
| `crates/libs/rns-transport/src/transport/tests.rs` | request handlers and dispatch, request receipts and timeouts |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_lifecycle.rs` | request handlers and dispatch, request receipts and timeouts |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests.rs` | request handlers and dispatch, request receipts and timeouts |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_isolation.rs` | request handlers and dispatch |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_malformed.rs` | request handlers and dispatch |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_probe.rs` | request handlers and dispatch |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_receipt.rs` | request receipts and timeouts |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_receipt_probe.rs` | request receipts and timeouts |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_receipt_probe2.rs` | request receipts and timeouts |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_receipt_probe3.rs` | request receipts and timeouts |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_receipt_stop.rs` | request receipts and timeouts |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_receipt_timeouts.rs` | request receipts and timeouts |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_resource_form.rs` | request handlers and dispatch |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_resource_form_probe.rs` | request handlers and dispatch |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_resource_form_probe2.rs` | request handlers and dispatch |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_resource_form_probe3.rs` | request handlers and dispatch |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_routing.rs` | request handlers and dispatch |
| `crates/libs/rns-transport/tests/request_frame_wire.rs` | request wire frames and fuzz |
| `crates/libs/rns-transport/tests/request_handler_api.rs` | request handlers and dispatch |
| `crates/libs/rns-transport/tests/request_receipt_api.rs` | request receipts and timeouts |
| `crates/libs/rns-transport/tests/request_receipt_readme.rs` | request receipts and timeouts |
| `docs/fork/PATCHSET.md` | request wire frames and fuzz, request handlers and dispatch, request receipts and timeouts |
| `docs/runbooks/fuzzing-campaign.md` | request wire frames and fuzz |
| `tools/scripts/fuzz-campaign.sh` | request wire frames and fuzz |
| `xtask/src/main_parts/run_key_management_check.rs` | request wire frames and fuzz |

Fork commits `8e9970ac`, `afc56836`, `71365159`, `f875aff7` and
`30d8dfde`: F1, request wire frames and fuzz. The first lands the frames,
the fuzz crate and the campaign entries; the second corrects the
`request_payload` doc comment; the third makes the campaign script
runnable (`--fuzz-dir` on the nightly toolchain, one `run_target` helper,
a crash-artifact gate), pins the 16-byte `str` request id refusal, makes
`FrameDecodeError::Malformed` a panic in the fuzz targets and the smoke
test, moves the advertisement vector include into `resource/tests.rs`,
adds the split-response vector and the black-box
`tests/request_frame_wire.rs`; the fourth corrects two fixture comments
in that file; the fifth adds two more tests to it and the contract test
`crates/apps/reticulumd/tests/fuzz_campaign_contract.rs`, which pins the
campaign script to the `--fuzz-dir` invocation, the toolchain override and
the artifact gate, and checks every target it runs against the fuzz crate
manifests and the runbook. The file sites below describe the combined
result.
Upstream sites:
`crates/libs/rns-transport/src/destination.rs` registers the new
`destination::request` module; `Link::request_payload` and
`unpack_response_envelope` in `link_sections/request_payload.rs` delegate
to `RequestFrame` and `ResponseFrame` with unchanged signatures;
`resource/tests.rs` includes the advertisement vector test inside its
`mod tests`; `run_key_management_check.rs`, `fuzzing-campaign.md` and
`fuzz-campaign.sh` add the third fuzz crate to the gate, the runbook and
the campaign. New files: `src/destination/request/{mod,frame,id,tests}.rs`,
`src/resource/tests_advert_kat.rs`, `tests/request_frame_wire.rs`,
`crates/apps/reticulumd/tests/fuzz_campaign_contract.rs` and the `fuzz/`
crate with its three targets. The crate `CHANGELOG.md` carries the
release notes.

Fork commits `08a5cff6` and `21ab1981`: request handlers and dispatch.
The first moves the `test_bridge` module out of `transport/mod.rs` into
`transport/test_bridge.rs` unchanged, to make room in that file; the
second lands the responder half of the link request layer in packet
form. The file sites below describe the combined result.
Upstream sites:
`destination/request/{allow,handler,inbound,dispatch}.rs` (new) carry
`RequestAllow`, `RequestHandler` with its closure blanket impl,
`RequestResponse`, `InboundRequest`, the crate-private `RegisteredHandler`
row, the `DispatchItem` message and the queue bound; `request/mod.rs`
declares and re-exports them. `destination_parts/module_core.rs` adds the
`request_handlers` table to the `Destination` struct,
`destination_parts/destination.rs` initialises it in the three
constructors and adds `register_request_handler`,
`deregister_request_handler` and the crate-private lookup, and
`destination_parts/module_prelude.rs` imports the request types and
`HashMap`. `link_parts/linkhandleresult.rs` adds the `request_dispatch`
sender seat to `Link`, `link_sections/{new,new_from_request}.rs`
initialise it to `None`, `link_sections/request_dispatch.rs` (new,
listed in `link_parts/link.rs`) carries `set_request_dispatch` and the
non-blocking `dispatch_request`, which the `Request` arm of
`handle_data_packet` in `link_sections/new.rs` calls while still posting
the `Data` event; `link_parts/module_prelude.rs` imports the message
types. `transport/mod.rs` adds the sender to `TransportHandler` and
declares `transport/request_dispatch.rs` (new): the `request-dispatch`
worker, the hoisted `send_on_iface`, `find_any_link`,
`resource_mtu_for_iface` and `send_response_resource` free functions and
the packet-response path; `transport/core.rs` creates the channel and
passes the receiver to `manage_transport`, which spawns the worker in
`transport/jobs.rs`; `transport/path.rs` sets the sender on every
accepted link beside its ingress interface;
`transport_sections/reset_out_link.rs` and `Transport::send_direct` in
`transport/core.rs` delegate to the hoisted functions. Tests:
`destination/tests.rs` pins the registry;
`tests_parts/two_transport_loopback_requests.rs` and
`two_transport_loopback_requests_isolation.rs` (new, two `include!` lines
in `transport/tests.rs`) pin the allow modes, the silent `None`, the slow
and panicking handler, the full queue, the `Data` event contract and the
oversize response; `two_transport_loopback_lifecycle.rs` keeps the
destination in its fixture and matches plain data only in its round
trip. The crate `README.md` gains the request handler section and the
crate `CHANGELOG.md` carries the release notes.

Fork commit `c1a98ff1`: the fixes for request handlers and dispatch.
Upstream sites: `dispatch_one` in `transport/request_dispatch.rs`
reads the destination's `max_request_size` and the handler row under one
guard and ignores a packet-form request over the limit before any handler
runs (`RNS/Link.py:999-1001`); `find_any_link` in the same file checks
`in_links` under the guard and collects `out_links` only on a miss.
`dispatch_request` in `link_sections/request_dispatch.rs` hands a request
on only while the link is active (`RNS/Link.py:805`). The handler table in
`destination_parts/module_core.rs` holds `Arc<RegisteredHandler>` rows,
`destination_parts/destination.rs` wraps and returns them, and
`request/handler.rs` loses the `Clone` derive. Tests:
`two_transport_loopback_requests.rs` pins the size gate and fails on a
lagged event receiver; `two_transport_loopback_requests_isolation.rs`
sends one more round trip after the panicked handler is joined;
`two_transport_loopback_requests_probe.rs` (new, a third `include!` line
in `transport/tests.rs`) and `tests/request_handler_api.rs` (new) pin the
remaining registry and response-form promises; `destination/tests.rs`
asserts the second registration's `Arc` replaced the first. The crate
`README.md` gains the concurrency note and the `max_request_size`
sentence.

Fork commit `12d2080c`: tests for request handlers and dispatch.
`two_transport_loopback_requests_probe.rs` measures the `max_request_size`
gate against the decrypted request envelope and shows
`clear_max_request_size` lifting it. `link_sections/request_dispatch_tests.rs`
(new, an `include!` line in `link_parts/link.rs`) pins the active-link gate
in `dispatch_request`; `identify_packet_tests.rs` opens its `linked_pair`
harness to that sibling module.

Fork commit `7bbb0197`: routing tests for request handlers and dispatch.
`two_transport_loopback_requests_routing.rs` (new, a fourth `include!`
line in `transport/tests.rs`) pins the allow list reading the link's
identity when each request arrives (`RNS/Link.py:818-824`), a second
`register_request_handler` for a path replacing the whole row including
its allow mode (`RNS/Destination.py:398`), and the handler table
belonging to the destination that owns the link (`RNS/Link.py:810`), so
a sibling destination on the same transport answers only once it has
registered the path itself. Fork commit `73f177b4` corrects the comment
on that file's announce spacing: the two-sample announce rate is the
fork's (`transport/announce_limits.rs:91-114`); Python measures nothing
until a third sample (`RNS/Interfaces/Interface.py:346-348`).

Fork commit `1fd1192a`: malformed-request test for request handlers and
dispatch. `two_transport_loopback_requests_malformed.rs` (new, a fifth
`include!` line in `transport/tests.rs`) sends request bodies that are not
a `[requested_at, path_hash, data]` frame and pins what Python does when
`unpackb` or `handle_request` cannot read one (`RNS/Link.py:1003-1007`,
`:804-807`): the `Data(Request)` event still posts, no handler runs, no
response leaves, and the next well-formed request on the link is
answered. The crate `CHANGELOG.md` names the file in the worker bullet.

Fork commit `febec3db`: the resource form of request handlers and
dispatch. Upstream sites: `handle_link_resource_packet` in
`transport/resource_wire.rs` hands every drained
`ResourceEventKind::Complete(ResourceComplete { is_request: true, .. })` for
the link to `Link::dispatch_request` under the existing link guard, before
the events are published, and compares a request advertisement's data size `d`
rather than its transfer size `t` against `max_request_size`
(`RNS/Link.py:1036-1046`, `RNS/Resource.py:1285-1287`).
`destination/request/dispatch.rs` reshapes `DispatchForm` into
`Packet { request_id }` and `Resource`, drops the `request_id` field of
`DispatchItem` and gives the form a `Display` for the drop logs;
`link_sections/request_dispatch.rs` makes `dispatch_request` crate-visible
and takes the form, `link_sections/new.rs` passes the packet form, and
`dispatch_one` in `transport/request_dispatch.rs` recomputes the
resource form's id with `RequestId::of_packed_request` over the assembled
bytes (`RNS/Link.py:885-889`). `request/handler.rs` and the two
resource-sending functions in `transport/request_dispatch.rs` document
the response forms, their `q`/`p` flags and metadata layout. Tests:
`two_transport_loopback_requests_resource_form.rs` (new, a sixth
`include!` line in `transport/tests.rs`) and
`link_sections/request_dispatch_tests.rs` reads the id from the form. The
crate `CHANGELOG.md` and `README.md` carry the release notes and the
user-facing paragraph.

Fork commit `7e9e5904`: the resource-form review probe adopted, and the
request limit doc in `two_transport_loopback_requests_resource_form.rs`
reworded to describe the behaviour it guards.
`two_transport_loopback_requests_resource_form_probe.rs` (new, a seventh
`include!` line in `transport/tests.rs`) pins the edges of the resource
form: `Complete(ResourceComplete { is_request: true, .. })` still posts
for an answered request Resource; `max_request_size` is inclusive of the
advertised data size `d` (`RNS/Link.py:1039`) and the link a rejection
used goes on carrying request Resources; a `Resource { metadata: None }`
response is raw data without metadata (`RNS/Link.py:839-846`); the
response Resource honours the registration's `auto_compress`
(`RNS/Link.py:846`, `:852`); an unregistered path, an allow list that
denies the unidentified peer (`RNS/Link.py:818-824`) and an assembled body
that is not a request frame all stay silent while the link keeps
answering.

Fork commit `5544c49c`: release notes for the resource form. The crate
`CHANGELOG.md` and `README.md` name
`two_transport_loopback_requests_resource_form_probe.rs`, write the
`Complete` variant in its tuple form, say that a `metadata: None`
resource is read as an envelope by a Python requester
(`RNS/Link.py:902-912`) and that an oversize request Resource surfaces to
the requester as `ResourceEventKind::OutboundRejected`.

Fork commit `816032c5`: the second resource-form review probe adopted,
and the size-constant comment in
`two_transport_loopback_resources_reject.rs` brought in line with the
gate reading the advertised `d`.
`two_transport_loopback_requests_resource_form_probe2.rs` (new, an eighth
`include!` line in `transport/tests.rs`) pins the response-size boundary
from the far side: an envelope one byte over the link MDU goes out as a
response Resource with `p` set and no response packet beside it
(`RNS/Link.py:849-852`); two request Resources in flight on one link are
each answered under their own `q`; and a handler that answers `None` for
a request Resource sends nothing (`RNS/Link.py:844`) while
`Complete(ResourceComplete { is_request: true, .. })` still posts and the
link keeps answering. The crate `CHANGELOG.md` names the file in both
pin clauses.

Fork commit `c82dcd0d`: the third resource-form review probe adopted.
`two_transport_loopback_requests_resource_form_probe3.rs` (new, a ninth
`include!` line in `transport/tests.rs`) pins the response form from the
requester's side of a request Resource: a small `Value` answer to a
request that arrived as a Resource goes out as exactly one `Response`
packet with no response Resource beside it (`RNS/Link.py:849`); a
registration with `RequestAllow::None` runs no handler and answers
nothing on the resource path (`RNS/Link.py:818-824`) while
`Complete(ResourceComplete { is_request: true, .. })` still posts, and
re-registering the path with `All` answers the next request Resource on
the same link; and a request packet, a request Resource and another
request packet interleaved on one link are each answered under their own
id by one handler. The crate `CHANGELOG.md` names the file in both pin
clauses.

Fork commit `1ddd5273`: request receipts and timeouts, the requester half
of the link request layer. `Transport::request` sends a request as
`RNS.Link.request` does (`RNS/Link.py:473-510`) and returns a
`RequestReceipt` modelled on `RNS.RequestReceipt` (`RNS/Link.py:1311-1500`)
without callbacks; the response is matched to the receipt in-path, under
the link lock, and the default timeout is `rtt * 6 + 11.25 s`
(`RNS/Link.py:491`). A timeout fails the receipt in every state, where
Python (`RNS/Link.py:1398`, `:1408`) fails only a `DELIVERED` receipt
(superseded by `3ed78582`: the clock runs from the send or from
`Delivered` and never while `Receiving`).
Upstream sites:
`destination/request/receipt.rs` (new, declared and re-exported by
`request/mod.rs`) carries `RequestOptions`, `RequestStatus`,
`RequestError`, `RequestResponseData`, the `Clone` handle
`RequestReceipt` over a shared outcome cell and status `watch`, and the
crate-private `PendingRequest` entry and `ResponseArrival` input.
`link_parts/linkhandleresult.rs` adds the `pending_requests` table to
`Link`, `link_sections/{new,new_from_request}.rs` initialise it, and
`link_parts/module_prelude.rs` imports the receipt and resource event
types. `link_sections/pending_requests.rs` (new, listed in
`link_parts/link.rs`) carries the synchronous table methods: insert,
remove, resolve, fail, fail-all, receiving, progress, the request
Resource outcome, expiry and the earliest deadline. The `Response` arm of
`handle_data_packet` in `link_sections/new.rs` resolves the receipt an
accepted response names and fails one over its size limit with
`Oversize`, before the `Data` event posts; the link-request reset there
fails every pending receipt with `LinkClosed`, as do `finalize_local_close`
and `restart` in `link_sections/next_watchdog_deadline.rs`, whose
`check_watchdog` expires deadlines first for an `Active` or `Stale` link
and whose `next_watchdog_deadline` folds the earliest pending deadline into
the link check wake-up. `transport/resource_wire.rs` moves a receipt to
`Receiving` with the advertised `d` and `t` when its response
advertisement is admitted and fails it with `Oversize` when it is not,
resolves it from `Complete { is_response }`, records `Progress`, and
hands the request Resource's `OutboundComplete`, `OutboundRejected` and
`OutboundFailed` to the table from both `handle_resource_proof` and
`handle_link_resource_packet`, all under the existing link guard.
`transport_sections/request.rs` (new, listed in `links_parts/transport.rs`)
carries `Transport::request`: the packet form through
`request_packet_with_max_response_size`, the resource form through a
mirror of `send_request_resource_with_max_response_size` that registers
the pending entry with the request Resource hash before the
advertisement leaves; the older helpers are unchanged. Tests:
`two_transport_loopback_requests_receipt.rs` and
`two_transport_loopback_requests_receipt_timeouts.rs` (new, two
`include!` lines in `transport/tests.rs`) pin the round trips in every
form on two clones of one receipt, the status orders, the file response,
`Oversize`, `LinkClosed` from either side, the unknown link, the default
timeout against `Link::rtt()`, the override timeout for both forms and
`Delivered` before the resource-form timeout. The crate `CHANGELOG.md`
and `README.md` carry the release notes and the `Sending requests`
paragraph.

Fork commit `a2a40de7`: corrects the `RNS/Link.py` line cites for the
receipt's progress bookkeeping in `destination/request/receipt.rs` and
`link_sections/pending_requests.rs` (`:1429` and `:1439`); doc comments only.

Fork commits `3805fe7f`, `3ed78582`, `415d67c8`, `afdf0f18`, `fe55cb71`
and `8909d956`:
request receipts and timeouts, the request clock as `RNS.RequestReceipt`
keeps it. The first moves `PendingRequest` and `ReceiptState` out of
`receipt.rs` with no behaviour change. The second anchors the clock: it
runs from the send for a request packet (`RNS/Link.py:1331-1334`) and from
`Delivered` for a request Resource (`RNS/Link.py:1370-1373`), so
`RequestReceipt::started_at` is an `Option` that is `None` until the
request Resource completes or a response is advertised
(`RNS/Link.py:1062-1063`); the watchdog never expires a `Receiving`
receipt (`RNS/Link.py:1389`), whose response Resource failing in transfer
fails it with the new `RequestError::Receive`; a receipt that fails while
a response Resource is inbound cancels that Resource (`RNS/Link.py:1435`)
and sends the receiver cancel only on an `Active` link
(`RNS/Resource.py:1111-1116`); `Transport::stop` fails every pending
receipt `LinkClosed`; a packet response sets both response sizes to the
packed body length (`RNS/Link.py:1016-1017`) and a split response sets
`response_size` once and sums `response_transfer_size` over its segments
(`RNS/Link.py:1057-1061`); response metadata gets the frame depth check.
It also adds the unit-test files `request/receipt_tests.rs`,
`link_sections/pending_requests_tests.rs` and
`resource/tests_timeouts_lifecycle.rs`.
The third routes the Resource outcomes the periodic workers drain to the
owning link's pending table, so a request Resource that exhausts its
retries fails `Send` and a response Resource whose receiver gives up fails
`Receive`. The fourth and fifth add the loopback and black-box tests; the
sixth corrects a helper's doc comment in the loopback tests. The file
sites below describe the combined result.
Upstream sites:
`destination/request/receipt_pending.rs` (new, declared by
`request/mod.rs`) carries `PendingRequest`, whose `deadline` is `None`
without a running clock or when the timeout does not fit an `Instant`,
and `ReceiptState`; `receipt.rs` keeps the public types, adds `Receive`,
makes `started_at` an `Option`, has `status` agree with a concluded
`response`, has `subscribe` report only the moves after the call and
walks response metadata with `frame::validate` before decoding it.
`link_sections/pending_requests.rs` hands back the response Resource
segment a failed entry was receiving from `fail_pending_request`,
`fail_all_pending_requests`, `insert_pending_request` (which fails a
displaced entry `Send`) and `note_request_resource_outcome` (which fails
a receipt `Receive` on its response Resource's `InboundFailed`), and
drops `pending_response_limits` entries with every failed entry, while
its expiry and earliest-deadline methods skip entries with no running
clock; `next_watchdog_deadline.rs` clears that table in `restart` and
`finalize_local_close`; the `Response` arm in `link_sections/new.rs`
hands the packed body length to `ResponseArrival::Packet`.
`resource/manager_cleanup.rs` adds `ResourceManager::cancel_incoming`.
`transport/resource_wire.rs` folds
`advertisement_reject` into `receiver_cancel`, cancels the inbound
response of every receipt it fails once the link guard is gone, and adds
`note_resource_events_on_links`, which `transport/jobs.rs` and
`jobs_parts/link_maintenance.rs` call before publishing the events their
drains produce. `transport/lifecycle.rs` adds
`fail_pending_requests_on_stop`; `transport_sections/request.rs` re-checks
the link is `Active` where the resource form registers its receipt and
cancels a displaced receipt's inbound response. Tests:
`request/receipt_tests.rs`, `link_sections/pending_requests_tests.rs` and
`resource/tests_timeouts_lifecycle.rs` (unit), the extended
`two_transport_loopback_requests_receipt.rs` and
`two_transport_loopback_requests_receipt_timeouts.rs`, the new
`two_transport_loopback_requests_receipt_probe.rs` (one more `include!` in
`transport/tests.rs`) and the black-box `tests/request_receipt_api.rs`.
The release notes for these commits are fork commit `10dc355e`, below.

Fork commit `10dc355e`: release notes for the request clock. The crate
`CHANGELOG.md` `Transport::request` bullet states what the receipt
exposes after the clock amendment (`started_at()` as an `Option` anchored
per request form, both response sizes for a packet response and summed
across a split one, cumulative `progress()`, the full `RequestError` set
with `Receive`, the `Stale` link refusal) and its cites move to
`RNS/Link.py:473-510`, `:1311-1500` and `:874-876`; the `Changed` bullet
on the request timeout is rewritten around the divergence from Python's
`DELIVERED` guard. The `README.md` `Sending requests` paragraph carries
the same clock rule and the `Rejected`, `Send`, `Receive` and `Stale`
outcomes, its snippet uses the crate `rmpv` re-export, and the responder
section says a response Resource resolves the requester's
`RequestReceipt` and an oversize request Resource reads
`RequestError::Rejected` on it.

Fork commit `642dfc2b`: a receipt is marked `Receiving` only for an
admitted response advertisement. The transport marked it as soon as the
advertisement passed the size check, before the ResourceManager had seen
it, so an advertisement the manager drops (inbound caps, an out-of-order
or duplicate segment, a receiver build error) left the receipt
`Receiving` with no deadline and no Resource whose failure could fail
it, and a routine advertisement retry over-counted
`response_transfer_size`.
Upstream sites:
`resource/manager_cleanup.rs` adds `ResourceManager::has_active_receiver`.
`transport/resource_wire.rs` marks the receipt `Receiving` after
`handle_packet_into_with_mtu` returns, gated on that method
(`RNS/Link.py:1055-1064` touches the receipt only once `Resource.accept`
returned one), re-drains the manager's events after the cancels it queues
in both `handle_link_resource_packet` and `handle_resource_proof`, so a
cancelled receiver's `InboundFailed` is published with the batch that
caused it, and logs a response advertisement whose `request_id` has the
wrong length. `request/receipt_pending.rs` has
`PendingRequest::mark_receiving` ignore a repeat of the segment in flight
(`RNS/Resource.py:594-598`, `:238-240`). `request/receipt.rs` has
`status` and `concluded_at` conclude `LinkClosed` through the new private
`outcome` when the entry is gone, makes `RESPONSE_GRACE` `pub(crate)`,
and widens the rustdoc on `RequestError::Oversize` (`RNS/Link.py:1017`,
`:876`, `:1408`), `RequestError::Send` and `ResponseArrival::Resource`
(`RNS/Link.py:897-912`). `link_sections/new.rs` and
`link_sections/pending_requests.rs` `debug_assert!` that the
`fail_pending_request` result they discard is `None`;
`link_sections/next_watchdog_deadline.rs` gives `restart` the comment
`finalize_local_close` had; `transport_sections/request.rs` removes the
pending entry after a failed send only when `registered.is_some()`;
`transport/lifecycle.rs` describes the receipt sweep of `stop` as
outside its 5 s budget. Tests: `request/receipt_tests.rs`, whose two
async tests carry the `await_holding_invalid_type` deny, pins a repeated
advertisement counting `t` once and `status()` and `concluded_at()`
concluding a vanished entry on their own;
`resource/tests_timeouts_lifecycle.rs` pins `has_active_receiver` for a
dropped and an admitted advertisement.

Fork commit `7ce1601c`: the `Receiving` loopback tests stall
mid-transfer, and the advertised sizes are pinned exactly.
`two_transport_loopback_requests_receipt_timeouts.rs`: the tests that
stall or drop a response Resource while the receipt reads `Receiving`
run their pair over `LEGACY_MTU`, where the 512 KiB body is about 1100
parts rather than three, so the stall lands with most of the transfer
still to come; the outlasts-the-timeout test is back on the
`current_thread` runtime; `assert_times_out` measures from a
`std::time::Instant` and the resource-form test hands it the receipt's
own `started_at()` rather than the observed `Delivered` instant; the
fixture reads `RESPONSE_GRACE` in place of the `11_250` literal.
`two_transport_loopback_requests_receipt.rs` pins `response_size()` to
the length of the `ResponseFrame` envelope and `response_transfer_size()`
to the `total_bytes` of the receiver's `Progress` events
(`RNS/Link.py:1057-1061`), in place of the `is_some()` and
`> expected.len()` checks.
`two_transport_loopback_requests_receipt_probe.rs` drops its
`DEFAULT_TIMEOUT_BASE` copy for `RESPONSE_GRACE`, which `request/mod.rs`
re-exports under `cfg(test)`.
`two_transport_loopback_requests_receipt_probe2.rs` (new, the fourth
`include!` line this patch adds to `transport/tests.rs`, the
sixty-fourth in the file) pins the usage patterns: a `Duration::MAX`
timeout never expires across two link checks and leaves the link
answering until a close ends the receipt `LinkClosed`; a timed-out
receipt leaves the request beside it and the one after it `Ready` and
the link `Active`, and re-reads `Timeout`; a link that closes from the
requester's side while a response Resource is inbound, over `LEGACY_MTU`
with the responder stalled, fails the receipt `LinkClosed`, not `Receive`
(`RNS/Link.py:1435`); and a response that resolved a receipt in-path
still posts `LinkEvent::Data(Response)` and `ResourceEventKind::Complete`
with `is_response` and the request id.

Fork commit `40409738`: the receipt sweep of `Transport::stop` shares the
stop budget, and `RequestOptions` keeps its getters in the crate. The
sweep ran after the 5 s budget with unbounded handler and link locks, so
a link whose lock was held elsewhere held `stop` open indefinitely.
Upstream sites: `transport/lifecycle.rs` passes the `deadline` of `stop`
into `fail_pending_requests_on_stop`, where every lock waits with
`time::timeout_at` on it: a handler lock that does not come skips the
sweep, or the cancels and the publish, a link lock that does not come
skips that link, each with a `[tp]` warning; `STOP_BUDGET` is
`pub(super)`, and the `stop` rustdoc says the interface wait, the
management wait and the sweep share one budget. tokio's `Timeout` polls
the lock before the deadline, so an uncontended lock is still taken once
the budget is spent. `request/receipt.rs` makes `max_response_size()`
`pub(crate)` and removes the unused `timeout()` getter, so the private
fields and the `with_*` builders are the public surface. Tests:
`two_transport_loopback_requests_receipt_stop.rs` (new, one more
`include!` line in `transport/tests.rs`) holds the connector's link lock
on another thread across `stop` and pins that `stop` returns within the
budget plus slack, leaves the held link's receipt `Sent`, and that the
receipt concludes `LinkClosed` once the lock is released and the
transport dropped; `tests/request_receipt_api.rs` pins the builder chain
and the `Clone + Default + Debug` bounds in place of the getters;
`request/receipt_tests.rs` reads the override through
`timeout_for_rtt`.

Fork commit `2f2c11c7`: a locally cancelled request Resource fails its
receipt `Send`, and the admitted-advertisement gate is keyed by request.
`Link::note_request_resource_outcome` treated `OutboundCancelled` as no
outcome, so a request Resource cancelled through
`Transport::cancel_resource` left its receipt `Sent` with no clock until
the link closed; `ResourceManager::has_active_receiver` was keyed by
segment hash alone, so a responder re-advertising an active segment
under another request id would have marked that other receipt
`Receiving` with nothing to ever fail it. Upstream sites:
`link_sections/pending_requests.rs` maps `OutboundCancelled` to
`RequestError::Send` (`RNS/Link.py:1375-1385` fails the receipt for
every Resource status but complete) and logs at debug the cause
`PendingRequest::resolve` hands back for a response that did not decode;
`transport_sections/reset_out_link.rs` has `cancel_resource` route the
manager's events through the link under the handler-then-link lock order
and re-drain the manager before publishing; `resource/manager_cleanup.rs`
gives `has_active_receiver` the link id and request id and compares them
with the receiver's, and makes it and `cancel_incoming` `pub(crate)`;
`transport/resource_wire.rs` passes `link.id()` and the request id bytes.
`request/receipt.rs` adds the `pub(crate)` `ResponseDecodeFailure` enum
(`Frame`, `Metadata`, `MetadataRead`, `MetadataTrailing`) that
`ResponseArrival::decode` returns and widens the `RequestError::Send`
rustdoc; `request/receipt_pending.rs` has `resolve` conclude `Decode` and
return the cause, and its `mark_delivered` rustdoc records that a receipt
already `Receiving` keeps its state where `RNS/Link.py:1372` sets
`DELIVERED`. `link_sections/next_watchdog_deadline.rs` and
`link_sections/new.rs` say a receiver left inbound fails on its own retry
limit (`RNS/Resource.py:1111`); the oversize response gate cite in
`two_transport_loopback_requests_receipt_probe.rs` is
`RNS/Link.py:1048-1053`. Tests: `link_sections/pending_requests_tests.rs`
pins the cancel failing `Send` and leaving an unrelated entry `Sent`;
`request/receipt_tests.rs` pins `mark_delivered` after `mark_receiving`
changing nothing, a non-envelope response failing `Decode` with a
`Frame` cause, and the depth refusal as a `Metadata` cause;
`resource/tests_timeouts_lifecycle.rs` pins `has_active_receiver` false
for another request id and for another link.

Fork commit `140a4fd2`: the dropped-advertisement probe and the README
usage tests. `two_transport_loopback_requests_receipt_probe3.rs` (new,
one more `include!` line in `transport/tests.rs`) injects a response
advertisement the ResourceManager refuses, a part count over the inbound
cap, for a pending receipt and pins that the receipt stays `Sent` with
no sizes and its clock running, then fails `Timeout`
(`RNS/Link.py:1055-1064`), and that the link keeps answering.
`tests/request_receipt_readme.rs` (new) drives the README's handler and
request snippets verbatim from outside the crate over a loopback TCP
pair: a packet-form request goes `Sent` to `Ready` without `Delivered`,
a response over the link MDU arrives as a Resource through `Receiving`,
a closed link fails its receipt `LinkClosed` and refuses the next
request with `RnsError::ConnectionError`, and `Transport::stop` fails
every pending receipt on every link whatever state it is in.

## 0.14.1 (upstream 0.14.0)

Every file changed between the `0.14.0` tag (`b56452e6`) and this release,
and the patch each change belongs to. Patch ids: B1 is bound-interface
segment dispatch, B2 is reject outside link lock, B3 is sender before
advertisement, B4 is hex parsers, B5 is non-fatal announce; the advert cap,
additive API, fixtures and lint tables, Python-in-the-loop proof, publish
contract, link-close smoke leg and release 0.14.1 patches carry no other id.
The `release 0.14.1` patch is fork commits `fd83a9f1`, `256936ca`,
`e79b6026`, `a2e37568` and `5243a01d`.

| File | Patch |
| --- | --- |
| `.github/workflows/publish-coyote-rns-transport.yml` | publish contract |
| `Cargo.lock` | release 0.14.1 |
| `Cargo.toml` | fixtures and lint tables, release 0.14.1 |
| `clippy.toml` | fixtures and lint tables |
| `crates/apps/reticulumd/Cargo.toml` | fixtures and lint tables, release 0.14.1 |
| `crates/apps/reticulumd/tests/fork_publish_contract.rs` | publish contract, link-close smoke leg, release 0.14.1 |
| `crates/apps/reticulumd/tests/python_channel_interop.rs` | Python-in-the-loop proof |
| `crates/apps/reticulumd/tests/python_channel_interop_parts/multi_segment_python_interop.rs` | Python-in-the-loop proof, release 0.14.1 |
| `crates/apps/reticulumd/tests/support/python_channel_endpoint.py` | Python-in-the-loop proof, release 0.14.1 |
| `crates/apps/reticulumd/tests/support/python_channel_process.rs` | Python-in-the-loop proof |
| `crates/apps/rns-tools/Cargo.toml` | release 0.14.1 |
| `crates/libs/rns-transport/CHANGELOG.md` | bound-interface segment dispatch, reject outside link lock, sender before advertisement, hex parsers, non-fatal announce, advert cap, additive API, fixtures and lint tables, Python-in-the-loop proof, release 0.14.1 |
| `crates/libs/rns-transport/Cargo.toml` | release 0.14.1 |
| `crates/libs/rns-transport/README.md` | link-close smoke leg, publish contract, release 0.14.1 |
| `crates/libs/rns-transport/src/destination/link_parts/link_sections/remove_channel_handler.rs` | additive API |
| `crates/libs/rns-transport/src/destination_hash.rs` | additive API |
| `crates/libs/rns-transport/src/hash.rs` | hex parsers, additive API |
| `crates/libs/rns-transport/src/identity.rs` | hex parsers |
| `crates/libs/rns-transport/src/identity/tests.rs` | hex parsers |
| `crates/libs/rns-transport/src/lib.rs` | additive API |
| `crates/libs/rns-transport/src/resource/advertisement_limits.rs` | advert cap |
| `crates/libs/rns-transport/src/resource/manager.rs` | sender before advertisement |
| `crates/libs/rns-transport/src/resource/manager_cleanup.rs` | sender before advertisement |
| `crates/libs/rns-transport/src/resource/manager_segments.rs` | sender before advertisement |
| `crates/libs/rns-transport/src/resource/manager_start.rs` | sender before advertisement |
| `crates/libs/rns-transport/src/resource/sender.rs` | sender before advertisement |
| `crates/libs/rns-transport/src/resource/tests.rs` | sender before advertisement, advert cap |
| `crates/libs/rns-transport/src/resource/tests_advert_limits.rs` | advert cap |
| `crates/libs/rns-transport/src/resource/tests_split_lazy.rs` | sender before advertisement |
| `crates/libs/rns-transport/src/resource/tests_split_unconfirmed.rs` | sender before advertisement |
| `crates/libs/rns-transport/src/resource/tests_split_unconfirmed_served.rs` | sender before advertisement |
| `crates/libs/rns-transport/src/resource/tests_timeouts.rs` | sender before advertisement |
| `crates/libs/rns-transport/src/resource/tests_timeouts_cleanup.rs` | sender before advertisement |
| `crates/libs/rns-transport/src/transport/core.rs` | non-fatal announce, additive API |
| `crates/libs/rns-transport/src/transport/jobs.rs` | bound-interface segment dispatch |
| `crates/libs/rns-transport/src/transport/lifecycle.rs` | additive API |
| `crates/libs/rns-transport/src/transport/mod.rs` | additive API |
| `crates/libs/rns-transport/src/transport/resource_wire.rs` | bound-interface segment dispatch, reject outside link lock |
| `crates/libs/rns-transport/src/transport/tests.rs` | bound-interface segment dispatch, reject outside link lock, additive API, fixtures and lint tables |
| `crates/libs/rns-transport/src/transport/tests_parts/resource_retry_link_lookup.rs` | bound-interface segment dispatch |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback.rs` | fixtures and lint tables |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_lifecycle.rs` | non-fatal announce, advert cap, additive API |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_lifecycle_probe.rs` | non-fatal announce, additive API |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_resources.rs` | bound-interface segment dispatch |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_resources_directions.rs` | bound-interface segment dispatch |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_resources_gone_link.rs` | bound-interface segment dispatch, release 0.14.1 |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_resources_reject.rs` | reject outside link lock |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_resources_reject_response.rs` | reject outside link lock |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_resources_retries.rs` | bound-interface segment dispatch |
| `crates/libs/rns-transport/tests/crate_root_reexports.rs` | additive API |
| `crates/libs/rns-transport/tests/destination_address_kat.rs` | additive API |
| `docs/fork/PATCHSET.md` | bound-interface segment dispatch, reject outside link lock, sender before advertisement, hex parsers, non-fatal announce, advert cap, additive API, fixtures and lint tables, Python-in-the-loop proof, release 0.14.1 |
| `docs/getting-started.md` | release 0.14.1 |
| `docs/lxmf-rs-api.md` | publish contract |
| `docs/performance/tcp-connection-memory.md` | publish contract |
| `docs/runbooks/crates-io-publish-plan.md` | publish contract |
| `docs/runbooks/release-readiness.md` | publish contract |
| `docs/status/python-surface-mapping.json` | publish contract |
| `docs/status/python-surface-parity.json` | publish contract |
| `docs/status/reticulum-parity-matrix.md` | release 0.14.1 |
| `docs/status/rns-1.5-delta.md` | publish contract |
| `docs/status/software-parity-ledger.md` | publish contract |
| `docs/status/v0.9.6-hardening-audit.md` | publish contract |
| `tests/hil/cases/core.toml` | publish contract |
| `tools/scripts/check-publish-tag-version.sh` | publish contract |
| `tools/scripts/python-lxmd-rust-lxmd-smoke.sh` | link-close smoke leg |

Fork commits `7c98216e`, `42825a54` and `2079a74e`: fixtures and lint
tables. The first rolls out the guard-across-await lint and seeds the release
docs; the second adds the loopback fixture; the third brings the fixture
under the lint.

- `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback.rs`
  (new): in-crate `two_transport_loopback` fixture, two `Transport`s joined
  over a real `TcpServer` / `TcpClient` pair on 127.0.0.1 with broadcast
  disabled on both sides, plus a smoke test that opens a link and exchanges
  one `Data` packet in each direction. Every loopback test under this
  section builds on it. Its async fns carry
  `#[deny(clippy::await_holding_invalid_type)]`, the per-fn opt-in that
  overrides the `allow` tables below. One `include!` line in
  `crates/libs/rns-transport/src/transport/tests.rs`.
- `clippy.toml`: the `await-holding-invalid-types` list names the tokio
  `MutexGuard`, `RwLockReadGuard` and `RwLockWriteGuard` types.
- `Cargo.toml` (`[workspace.lints.clippy]`) and
  `crates/apps/reticulumd/Cargo.toml` (`[lints.clippy]`):
  `await_holding_invalid_type = "allow"` until the existing hits are cleared.
- `crates/libs/rns-transport/CHANGELOG.md`: the `### Changed` lint bullet
  under `## 0.14.1`; the fixture is named in the trailing clause of the B1
  `### Fixed` bullet.

Fork commits `7f8b4c96`, `b64cb536` and `73e04b76` (B1): send later
Resource segments and Resource retries on the link's bound interface. The second
commit reshapes the link lookup and its unit test; the third adopts the
gone-link retry test. The file sites below describe the combined result.

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
  (new): the connector forgets its `out_links` as soon as `send_resource`
  returns, before the retry worker can be due (the retry clock starts under
  the handler lock the dispatch releases last, and the worker needs that
  lock); exactly one advertisement may reach the listener, the sender must
  still report `OutboundFailed` once its retries are spent, and a fresh link
  over the same transports must still carry a Resource.
- `crates/libs/rns-transport/src/transport/tests.rs`: one `include!` line
  for each of the five test files above.
- `crates/libs/rns-transport/CHANGELOG.md`: `### Fixed` bullet under
  `## 0.14.1`.

Fork commits `e239a239`, `7cfe7e73`, `40b392b7`, `8eaf1ee7`, `915c214f`
and `21b6dcb3` (B2, B3): send Resource advertisement rejects outside the
link lock, and register an
outbound Resource sender before its advertisement is written. The second
commit guards the dispatch confirm and moves the unconfirmed-sender tests;
the third only touches the `track_prepared` doc comment in
`manager_start.rs` (it cites `RNS/Resource.py:545` and `:552` for the
reference ordering); the fourth carries parts served before the confirm
into the sender status; the fifth adds the response-size reject loopback
test; the sixth adopts the served-parts confirm tests. The file sites below
describe the combined result.

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
  `ResourceReceiverCancel` arm), `manager_cleanup.rs` (`remove_link_state`),
  `sender.rs` (`confirm_advertised`, `settle_transfer_status`):
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
  `## 0.14.1`, including the `track_prepared` /
  `confirm_outbound_dispatch` semantics note.

Fork commits `e69eff43` and `49923b1a` (B4, B5, advert cap, additive API):
harden the hex parsers, make announce building non-fatal, drop over-cap Resource advertisements, and add
the additive lifecycle and helper API. The second commit adopts the
destination-address known-answer test and the advert-cap boundary tests.
Every public change is additive; no existing signature, enum or public field
changes.

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
  per segment (`d` at 64 times that) is admitted; a well-formed
  advertisement after a dropped over-cap one is still admitted on the same
  link; and the cap sits exactly at `3 * MAX_EFFICIENT_SIZE` (a single
  segment with `t` at the cap is admitted and requests parts, `t` one byte
  above it gets no reply and no receiver).
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
- `crates/libs/rns-transport/tests/destination_address_kat.rs` (new
  integration test target): three known-answer vectors computed
  independently with Python `hashlib` per `RNS/Destination.py:116-130`
  (`sha256(sha256(expanded_name)[:10] ++ identity_hash)[:16]`) pin the
  name hash, the 32-hex address and its `short()` prefix for
  `lxmf.delivery`, `rnstransport.probe` and `example_utilities.linkexample`;
  the first vector's address also round-trips through
  `AddressHash::new_from_hex_string`. This is the only check of
  `destination_address` against the reference algorithm rather than against
  `SingleInputDestination`, which shares its implementation.
- `crates/libs/rns-transport/CHANGELOG.md`: six `### Added` and three
  `### Fixed` bullets under `## 0.14.1`.

Fork commit `0fc93dd4`: Python-in-the-loop proof of multi-segment transfer
under broadcast off. Test-only; it pins the bound-interface dispatch patch
above against the reference implementation on either end.

- `crates/apps/reticulumd/tests/python_channel_interop_parts/multi_segment_python_interop.rs`
  (new): two ignored tests (they need a local Python Reticulum checkout).
  Both build the Rust `Transport` with broadcast off and move a `4 MiB + 1`
  byte response Resource, which spans five segments; the body is a run of
  SHA-256 blocks so it does not compress, and both ends compare SHA-256
  digests. In `rust_to_python_multi_segment_response_roundtrip` Rust is the
  requester: `Link::request_payload` plus `send_request_resource`; the
  receiver asserts five `SegmentComplete` events for the response, and the
  Python server reports what it served through a stdout pipe the test drains
  for the whole transfer. In
  `python_to_rust_multi_segment_response_roundtrip` Rust is the responder:
  the request arrives through `in_link_events` as a `Data` event with the
  `Request` context and is answered with `send_response_resource`. On
  `2079a74e` the Rust-serves case fails with `OutboundFailed` (`outbound
  resource failed before completion`) while the Rust-requests case passes;
  both pass on `0fc93dd4` in about 6 s.
- `crates/apps/reticulumd/tests/python_channel_interop.rs`: one `include!`
  line for the file above.
- `crates/apps/reticulumd/tests/support/python_channel_process.rs`: new
  `spawn_multi_segment_client`, which starts the Python client with the
  `multi-segment-response` payload kind and a 180 s client timeout.
- `crates/apps/reticulumd/tests/support/python_channel_endpoint.py`: new
  `multi-segment-response` payload kind. As a server, `_on_request` returns
  the hash-block body and prints `served_response_size` and
  `served_response_sha256`; as a client it sends `Link.request(data=None)`
  and prints `response_size`, `response_sha256` and `negotiated_mdu`.
- `crates/libs/rns-transport/CHANGELOG.md`: trailing clause of the
  B1 `### Fixed` bullet under `## 0.14.1`, added by the first release commit.

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

Fork commits `0197768d` and `a3e0262c`: publish contract. Not a patch to the
crate, and the rename edits in these commits are not listed; the files are
named here because they appear in the table above. `0197768d` moves the
tag-matches-version guard into `tools/scripts/check-publish-tag-version.sh`,
called from `.github/workflows/publish-coyote-rns-transport.yml`, pins the
manifest and workflow tokens in
`crates/apps/reticulumd/tests/fork_publish_contract.rs`, and points
`tests/hil/cases/core.toml` at the real link-close test name; `a3e0262c` pins
the crate version and the `rns-core` dependency in the same test and extends
its old-name walk to `docs/status/`. The remaining files in that table under
`publish contract` (`docs/lxmf-rs-api.md`,
`docs/performance/tcp-connection-memory.md`, the two `docs/runbooks/` files,
the `docs/status/` JSON and Markdown files) only have the crate name
updated.

Fork commits `fd83a9f1`, `256936ca`, `e79b6026`, `a2e37568` and `5243a01d`:
release 0.14.1. The first bumps the crate version at every manifest,
consumer-snippet and contract-test site, regenerates `Cargo.lock`, promotes
`## Unreleased` in the crate `CHANGELOG.md`, adds two contract tests to
`crates/apps/reticulumd/tests/fork_publish_contract.rs` (every version site,
the dated `CHANGELOG.md` heading and the README policy tokens track the
crate version; the file table above names only existing files and the files
every release touches), and amends the `#610` row in
`docs/status/reticulum-parity-matrix.md`. The second adds two more contract
tests (root `VERSION` stays on the upstream baseline; every table row names
a patch the legend defines), sharpens the fixture comments and the lag report
in `multi_segment_python_interop.rs` and `python_channel_endpoint.py`, and
aligns the README's description of this ledger. The third adds a contract
test that the README rebase recipe runs the publish workflow's own dry-run
command with no `--registry` override, makes the row check match every
phrase of a patch cell against the legend, and names the lint-table patch in
the README patch list; it also hardens `multi_segment_python_interop.rs`,
whose child-stdout drain now skips an `InvalidData` line instead of stopping
and whose `wait_for_link_request` counts lagged events into its timeout
report. The fourth adopts the `release_notes_cite_files_that_exist_in_the_tree`
contract test. The fifth makes `two_transport_loopback_resources_gone_link.rs`
independent of the retry pump timing. The ledger-only commits that complete
this section are omitted here by convention. The link-close patch from the
`## 0.14.0` section below (`3f20c991`) is still carried; its crate sources
are unchanged since the tag and its smoke-script leg is superseded by
`0197768d` above.

## 0.14.0 (upstream 0.14.0)

The published tag `coyote-rns-transport-v0.14.0` is `b56452e6`.

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
  `main` by `0197768d`; see `## 0.14.1`).
