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
belongs to. Patch ids: F1 is request wire frames and fuzz, F2 is link
proof event and receipt link id, F3 is outbound outcomes with link fast
fail and accept hook. The
`request wire frames and fuzz` patch is fork commits `8e9970ac`,
`afc56836`, `71365159`, `f875aff7` and `30d8dfde`; the request handlers
and dispatch patch carries no other id and is fork commits `08a5cff6`,
`21ab1981`, `c1a98ff1`, `12d2080c`, `7bbb0197`, `73f177b4`, `1fd1192a`,
`febec3db`, `7e9e5904`, `5544c49c`, `816032c5` and `c82dcd0d`; the
request receipts and timeouts patch carries no other id and is fork
commits `1ddd5273`, `a2a40de7`, `3805fe7f`, `3ed78582`, `415d67c8`,
`afdf0f18`, `fe55cb71`, `10dc355e`, `8909d956`, `642dfc2b`, `7ce1601c`,
`40409738`, `2f2c11c7`, `140a4fd2`, `3035e470`, `8a8c7a4e`, `6a0fc9cb`,
`bbd34e72`, `114dbe1c`, `6f8cba34`, `1ef4cddd`, `f732e7d4`, `abd9187d`,
`938d2852`, `6b5e9a4e`, `9521e2c0`, `b8ec835a`, `8b7557bd`, `f2ff9922`,
`60cb3755`, `e28c5a03`, `1da7243b`, `7aaf7a50`, `f575a790`, `ad9f6b8a`,
`0d4c9342`, `181c4a27`, `8eebdfab`, `ca4d1aa4`, `6b3ab18e`, `bb8a94ad`,
`d042612c`, `e5183c64`, `64d49999`, `08794d16`, `f7922574`, `8549dd6f`,
`b836157a`, `30f98996`, `55736b0b`, `f10a7843`, `09f6c98b`, `21fc1c3f`,
`f7d8c60d`, `f68479d5`, `c13491a7`, `28d7e69b`, `6ce0e712`, `4309e2ee`,
`62cd6799`, `64ab1317`, `20019b5f`, `1dd36956`, `03f63892`, `032e9142`,
`8b512044`, `9ebd348b`, `9842695a`, `10f67e30`, `c18098c9`, `3568f1ab`,
`f1985d01`, `348cf74d`, `a93e3127`, `c7d5f533` and `d9585ef0`; the
`link proof event and receipt link id` patch is fork commits `6499621f`,
`d4127c6d`, `04de5d71`, `875edf92`, `8c262c9f` and `631203b5`; the
`outbound outcomes with link fast fail and accept hook` patch is fork commits
`50793515`, `4a4a91ae`, `3a5cd726`, `a0a88883`, `9eaafcc1`, `98718b4d`,
`97bd6a91`, `85c20ef5`, `1bd387b4`, `5df575ec`, `873ef34f`, `211d84b8`,
`9a4e8735`, `a4cf4f4b`, `65fb8646`, `01e93fc3` and `17348ca6`; the
`Python-in-the-loop request and proof interop` patch is fork commits
`60e74ffb`, `6e583b50`, `ea6edfaa`, `9223ef43`, `6e9eedf2` and
`3f615a3b`.

| File | Patch |
| --- | --- |
| `crates/apps/reticulumd/src/bin/reticulumd/bridge.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/apps/reticulumd/src/bin/reticulumd/bridge_delivery_task_parts/deliverytask_sections/resolve_or_create_propagation_link.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/apps/reticulumd/src/bin/reticulumd/bridge_delivery_task_tests_parts/module_prelude.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/apps/reticulumd/src/bin/reticulumd/bridge_delivery_task_tests_parts/oversized_opportunistic_delivery_fal.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/apps/reticulumd/src/bin/reticulumd/bridge_link_send.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/apps/reticulumd/src/bin/reticulumd/bridge_propagation.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/apps/reticulumd/src/bin/reticulumd/bridge_remote_control_link.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/apps/reticulumd/src/bin/reticulumd/bridge_remote_control_link_tests.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/apps/reticulumd/src/bin/reticulumd/direct_backchannel.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/apps/reticulumd/src/bin/reticulumd/inbound_routing.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/apps/reticulumd/src/bin/reticulumd/inbound_worker_link_identification.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/apps/reticulumd/src/bin/reticulumd/inbound_worker_parts/inbound_propagation_payload_is_inges_sections/partial_inbound_resource_teardown.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/apps/reticulumd/src/bin/reticulumd/inbound_worker_parts/inbound_propagation_payload_is_inges_sections/remote_inbound_resource_cancel.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/apps/reticulumd/src/bin/reticulumd/inbound_worker_parts/inbound_propagation_payload_is_inges_sections/resource_terminal_consumer.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/apps/reticulumd/src/bin/reticulumd/inbound_worker_parts/module_core.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/apps/reticulumd/tests/direct_link_delivery.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/apps/reticulumd/tests/fuzz_campaign_contract.rs` | request wire frames and fuzz |
| `crates/apps/reticulumd/tests/python_channel_interop.rs` | Python-in-the-loop request and proof interop |
| `crates/apps/reticulumd/tests/python_channel_interop_parts/ax25_kiss_pty.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/apps/reticulumd/tests/python_channel_interop_parts/ifac_kiss_pty.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/apps/reticulumd/tests/python_channel_interop_parts/ifac_python_interop.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/apps/reticulumd/tests/python_channel_interop_parts/module_prelude.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/apps/reticulumd/tests/python_channel_interop_parts/multi_segment_python_interop.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/apps/reticulumd/tests/python_channel_interop_parts/python_request_forms_matrix.rs` | Python-in-the-loop request and proof interop |
| `crates/apps/reticulumd/tests/python_channel_interop_parts/python_resource_fault_reverse.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/apps/reticulumd/tests/python_channel_interop_parts/python_to_rust_request_handler.rs` | Python-in-the-loop request and proof interop |
| `crates/apps/reticulumd/tests/python_channel_interop_parts/reader_resource_interop.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/apps/reticulumd/tests/python_channel_interop_parts/resource_faults.rs` | outbound outcomes with link fast fail and accept hook, Python-in-the-loop request and proof interop |
| `crates/apps/reticulumd/tests/python_channel_interop_parts/resource_memory_profile.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/apps/reticulumd/tests/python_channel_interop_parts/resource_size_matrix.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/apps/reticulumd/tests/python_channel_interop_parts/rust_to_python_raw_resource_roundtri.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/apps/reticulumd/tests/python_channel_interop_parts/rust_to_python_request_receipt.rs` | Python-in-the-loop request and proof interop |
| `crates/apps/reticulumd/tests/support/python_channel_endpoint.py` | Python-in-the-loop request and proof interop |
| `crates/apps/reticulumd/tests/support/python_channel_events.rs` | outbound outcomes with link fast fail and accept hook, Python-in-the-loop request and proof interop |
| `crates/apps/reticulumd/tests/support/python_channel_process.rs` | Python-in-the-loop request and proof interop |
| `crates/apps/reticulumd/tests/support/python_channel_request_forms.py` | Python-in-the-loop request and proof interop |
| `crates/apps/rns-tools/src/bin/independent_interop_node/events.rs` | link proof event and receipt link id, outbound outcomes with link fast fail and accept hook |
| `crates/apps/rns-tools/src/bin/independent_interop_node/node.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/apps/rns-tools/src/bin/rncp_parts/client.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/apps/rns-tools/src/bin/rncp_parts/server.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/apps/rns-tools/src/bin/rngit_parts/native_client.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/apps/rns-tools/src/bin/rnsh_parts/network.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/libs/lxmf-runtime/src/link_delivery.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/libs/lxmf-runtime/src/tests.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/libs/lxmf-runtime/src/tests_link_delivery.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/CHANGELOG.md` | request wire frames and fuzz, request handlers and dispatch, request receipts and timeouts, link proof event and receipt link id, outbound outcomes with link fast fail and accept hook, Python-in-the-loop request and proof interop |
| `crates/libs/rns-transport/README.md` | request handlers and dispatch, request receipts and timeouts, link proof event and receipt link id, outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/fuzz/.gitignore` | request wire frames and fuzz |
| `crates/libs/rns-transport/fuzz/Cargo.lock` | request wire frames and fuzz |
| `crates/libs/rns-transport/fuzz/Cargo.toml` | request wire frames and fuzz |
| `crates/libs/rns-transport/fuzz/fuzz_targets/request_frame_decode.rs` | request wire frames and fuzz |
| `crates/libs/rns-transport/fuzz/fuzz_targets/resource_advertisement_decode.rs` | request wire frames and fuzz |
| `crates/libs/rns-transport/fuzz/fuzz_targets/response_frame_decode.rs` | request wire frames and fuzz |
| `crates/libs/rns-transport/src/channel_buffer_parts/stream_data_message_roundtrips_compr.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/delivery.rs` | request receipts and timeouts, outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/destination.rs` | request wire frames and fuzz |
| `crates/libs/rns-transport/src/destination/link_parts/link.rs` | request handlers and dispatch, request receipts and timeouts, link proof event and receipt link id |
| `crates/libs/rns-transport/src/destination/link_parts/link_sections/data_proof_tests.rs` | link proof event and receipt link id |
| `crates/libs/rns-transport/src/destination/link_parts/link_sections/handle_proof_packet.rs` | link proof event and receipt link id |
| `crates/libs/rns-transport/src/destination/link_parts/link_sections/identify_packet_tests.rs` | request handlers and dispatch |
| `crates/libs/rns-transport/src/destination/link_parts/link_sections/new.rs` | request handlers and dispatch, request receipts and timeouts, link proof event and receipt link id |
| `crates/libs/rns-transport/src/destination/link_parts/link_sections/new_from_request.rs` | request handlers and dispatch, request receipts and timeouts, link proof event and receipt link id |
| `crates/libs/rns-transport/src/destination/link_parts/link_sections/next_watchdog_deadline.rs` | request receipts and timeouts, link proof event and receipt link id |
| `crates/libs/rns-transport/src/destination/link_parts/link_sections/pending_requests.rs` | request receipts and timeouts, outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/destination/link_parts/link_sections/pending_requests_tests.rs` | request receipts and timeouts, outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/destination/link_parts/link_sections/request_dispatch.rs` | request handlers and dispatch |
| `crates/libs/rns-transport/src/destination/link_parts/link_sections/request_dispatch_tests.rs` | request handlers and dispatch |
| `crates/libs/rns-transport/src/destination/link_parts/link_sections/request_payload.rs` | request wire frames and fuzz |
| `crates/libs/rns-transport/src/destination/link_parts/link_sections/response_packet.rs` | request receipts and timeouts |
| `crates/libs/rns-transport/src/destination/link_parts/link_sections/response_packet_tests.rs` | request receipts and timeouts |
| `crates/libs/rns-transport/src/destination/link_parts/link_sections/response_record_tests.rs` | request receipts and timeouts, link proof event and receipt link id, outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/destination/link_parts/linkhandleresult.rs` | request handlers and dispatch, request receipts and timeouts, link proof event and receipt link id |
| `crates/libs/rns-transport/src/destination/link_parts/module_prelude.rs` | request handlers and dispatch, request receipts and timeouts, link proof event and receipt link id |
| `crates/libs/rns-transport/src/destination/request/allow.rs` | request handlers and dispatch |
| `crates/libs/rns-transport/src/destination/request/dispatch.rs` | request handlers and dispatch |
| `crates/libs/rns-transport/src/destination/request/frame.rs` | request wire frames and fuzz |
| `crates/libs/rns-transport/src/destination/request/handler.rs` | request handlers and dispatch |
| `crates/libs/rns-transport/src/destination/request/id.rs` | request wire frames and fuzz |
| `crates/libs/rns-transport/src/destination/request/inbound.rs` | request handlers and dispatch |
| `crates/libs/rns-transport/src/destination/request/mod.rs` | request wire frames and fuzz, request handlers and dispatch, request receipts and timeouts, link proof event and receipt link id |
| `crates/libs/rns-transport/src/destination/request/receipt.rs` | request receipts and timeouts |
| `crates/libs/rns-transport/src/destination/request/receipt_order_tests.rs` | link proof event and receipt link id |
| `crates/libs/rns-transport/src/destination/request/receipt_pending.rs` | request receipts and timeouts |
| `crates/libs/rns-transport/src/destination/request/receipt_tests.rs` | request receipts and timeouts, link proof event and receipt link id |
| `crates/libs/rns-transport/src/destination/request/tests.rs` | request wire frames and fuzz |
| `crates/libs/rns-transport/src/destination/tests.rs` | request handlers and dispatch |
| `crates/libs/rns-transport/src/destination_parts/destination.rs` | request handlers and dispatch |
| `crates/libs/rns-transport/src/destination_parts/module_core.rs` | request handlers and dispatch |
| `crates/libs/rns-transport/src/destination_parts/module_prelude.rs` | request handlers and dispatch |
| `crates/libs/rns-transport/src/error.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/resource.rs` | request receipts and timeouts, outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/resource/manager.rs` | request receipts and timeouts, outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/resource/manager_cleanup.rs` | request receipts and timeouts, outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/resource/manager_polling.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/resource/manager_proof.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/resource/manager_segments.rs` | request receipts and timeouts, outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/resource/manager_start.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/resource/outcome.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/resource/sender.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/resource/tests.rs` | request wire frames and fuzz, outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/resource/tests_advert_kat.rs` | request wire frames and fuzz |
| `crates/libs/rns-transport/src/resource/tests_fault_injection.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/resource/tests_outcome.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/resource/tests_split_lazy.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/resource/tests_split_unconfirmed_served.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/resource/tests_timeouts.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/resource/tests_timeouts_lifecycle.rs` | request receipts and timeouts, outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/transport/announce.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/transport/core.rs` | request handlers and dispatch, link proof event and receipt link id, outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/transport/handler.rs` | request receipts and timeouts, link proof event and receipt link id |
| `crates/libs/rns-transport/src/transport/jobs.rs` | request handlers and dispatch, request receipts and timeouts |
| `crates/libs/rns-transport/src/transport/jobs_parts/link_maintenance.rs` | request receipts and timeouts |
| `crates/libs/rns-transport/src/transport/lifecycle.rs` | request receipts and timeouts, outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/transport/links_parts/transport.rs` | request receipts and timeouts, outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/transport/links_parts/transport_sections/close_channel.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/transport/links_parts/transport_sections/link_fanout.rs` | request receipts and timeouts |
| `crates/libs/rns-transport/src/transport/links_parts/transport_sections/link_open.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/transport/links_parts/transport_sections/request.rs` | request receipts and timeouts |
| `crates/libs/rns-transport/src/transport/links_parts/transport_sections/reset_out_link.rs` | request handlers and dispatch, request receipts and timeouts, outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/transport/links_parts/transport_sections/resource_reader.rs` | request receipts and timeouts |
| `crates/libs/rns-transport/src/transport/links_parts/transport_sections/send_outcome.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/transport/mod.rs` | request handlers and dispatch, request receipts and timeouts, link proof event and receipt link id, outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/transport/path.rs` | request handlers and dispatch |
| `crates/libs/rns-transport/src/transport/request_dispatch.rs` | request handlers and dispatch, outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/transport/resource_accept_hook.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/transport/resource_wire.rs` | request handlers and dispatch, request receipts and timeouts, outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/transport/resource_wire_parts/accept_hook.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/transport/resource_wire_parts/cancel.rs` | request receipts and timeouts |
| `crates/libs/rns-transport/src/transport/resource_wire_parts/proof.rs` | request receipts and timeouts |
| `crates/libs/rns-transport/src/transport/test_bridge.rs` | request handlers and dispatch |
| `crates/libs/rns-transport/src/transport/tests.rs` | request handlers and dispatch, request receipts and timeouts, link proof event and receipt link id, outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/transport/tests_parts/link_no_path_fast_fail.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/transport/tests_parts/pending_out_link_establishment_timeout.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/transport/tests_parts/pending_out_link_rediscovery.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/transport/tests_parts/rns_1_5_ingress_admission.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_announce_path_response.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_lifecycle.rs` | request handlers and dispatch, request receipts and timeouts |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_link_packet_proven.rs` | link proof event and receipt link id |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_link_packet_proven_probe.rs` | link proof event and receipt link id, outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_link_packet_proven_probe2.rs` | link proof event and receipt link id |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_link_packet_proven_probe3.rs` | link proof event and receipt link id |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests.rs` | request handlers and dispatch, request receipts and timeouts |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_errors.rs` | request receipts and timeouts, outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_isolation.rs` | request handlers and dispatch |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_malformed.rs` | request handlers and dispatch |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_probe.rs` | request handlers and dispatch, request receipts and timeouts |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_receipt.rs` | request receipts and timeouts |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_receipt_orders.rs` | link proof event and receipt link id |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_receipt_probe.rs` | request receipts and timeouts |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_receipt_probe2.rs` | request receipts and timeouts |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_receipt_probe3.rs` | request receipts and timeouts |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_receipt_probe4.rs` | request receipts and timeouts |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_receipt_probe5.rs` | request receipts and timeouts |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_receipt_probe6.rs` | request receipts and timeouts |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_receipt_probe7.rs` | request receipts and timeouts |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_receipt_stop.rs` | request receipts and timeouts |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_receipt_timeouts.rs` | request receipts and timeouts |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_resource_form.rs` | request handlers and dispatch, outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_resource_form_probe.rs` | request handlers and dispatch, outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_resource_form_probe2.rs` | request handlers and dispatch |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_resource_form_probe3.rs` | request handlers and dispatch, outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_resource_form_sizes.rs` | request receipts and timeouts, outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_resource_form_sizes_cancel.rs` | request receipts and timeouts |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_resource_form_sizes_cancel2.rs` | request receipts and timeouts |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_resource_form_sizes_cancel_probe.rs` | request receipts and timeouts |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_resource_form_sizes_probe.rs` | request receipts and timeouts, outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_resource_form_sizes_probe2.rs` | request receipts and timeouts, outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_resource_form_sizes_probe3.rs` | request receipts and timeouts, outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_resource_form_sizes_probe4.rs` | request receipts and timeouts |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_resource_form_sizes_probe5.rs` | request receipts and timeouts, outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_resource_form_sizes_probe6.rs` | request receipts and timeouts, outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_resource_form_sizes_probe7.rs` | request receipts and timeouts, outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_resource_form_sizes_probe8.rs` | request receipts and timeouts |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_resource_form_sizes_probe9.rs` | request receipts and timeouts, outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_resource_form_sizes_probe10.rs` | request receipts and timeouts |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_resource_form_sizes_probe11.rs` | request receipts and timeouts |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_resource_form_sizes_probe12.rs` | request receipts and timeouts |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_resource_form_sizes_probe13.rs` | request receipts and timeouts |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_resource_form_sizes_probe14.rs` | request receipts and timeouts |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_resource_form_sizes_probe15.rs` | request receipts and timeouts |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_resource_form_sizes_probe16.rs` | request receipts and timeouts |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_routing.rs` | request handlers and dispatch, request receipts and timeouts |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_resources.rs` | request receipts and timeouts, outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_resources_accept_hook.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_resources_directions.rs` | request receipts and timeouts, outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_resources_gone_link.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_resources_outcome.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_resources_outcome_probe.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_resources_outcome_probe2.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_resources_outcome_probe3.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_resources_outcome_stop.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_resources_receipt_less_receiving.rs` | request receipts and timeouts, outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_resources_reject.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_resources_reject_response.rs` | request receipts and timeouts, outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_resources_retries.rs` | request receipts and timeouts, outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/transport/wire.rs` | request receipts and timeouts, link proof event and receipt link id |
| `crates/libs/rns-transport/tests/link_event_packet_proven_api.rs` | link proof event and receipt link id |
| `crates/libs/rns-transport/tests/outbound_outcome_api.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/tests/request_frame_wire.rs` | request wire frames and fuzz |
| `crates/libs/rns-transport/tests/request_handler_api.rs` | request handlers and dispatch |
| `crates/libs/rns-transport/tests/request_receipt_api.rs` | request receipts and timeouts |
| `crates/libs/rns-transport/tests/request_receipt_readme.rs` | request receipts and timeouts, outbound outcomes with link fast fail and accept hook |
| `docs/fork/PATCHSET.md` | request wire frames and fuzz, request handlers and dispatch, request receipts and timeouts, link proof event and receipt link id, outbound outcomes with link fast fail and accept hook, Python-in-the-loop request and proof interop |
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
`resource/tests_timeouts_lifecycle.rs`, and the fixture constructors
`loopback_link_handles_with_mtu` in
`tests_parts/two_transport_loopback_lifecycle.rs` and
`loopback_request_pair_with_mtu` in
`tests_parts/two_transport_loopback_requests.rs`, which build a loopback
pair over interfaces of a given MTU; the MTU-less helpers delegate to them.
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
`RequestError::Send` (`RNS/Link.py:1377-1385` fails the receipt for
every Resource status but complete) and logs at debug the cause
`PendingRequest::resolve` hands back for a response that did not decode;
`transport_sections/reset_out_link.rs` has `cancel_resource` route the
manager's events through the link under the handler-then-link lock order
before publishing; `resource/manager_cleanup.rs`
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

Fork commit `3035e470`: release notes for the stop budget, the cancel
outcome and the keyed gate. The crate `CHANGELOG.md` says `started_at()`
of a request Resource starts when a response advertisement is admitted,
adds `RNS/Link.py:1048-1053` to the `Oversize` cite, names the local
cancel under `Send`, the shared 5 s budget and the held-link skip under
`Transport::stop`, and the request id and link in the admitted gate. The
`README.md` `Sending requests` paragraph says a request Resource cancelled
through `Transport::cancel_resource` fails `Send` and that
`Transport::stop()` fails every pending receipt within its budget.

Fork commit `8a8c7a4e`: a request Resource cancelled while its receipt is
already `Receiving` cancels the response inbound for it. The request
Resource stays in the manager's outgoing table until its proof arrives,
and a response advertisement admitted ahead of a lost or late proof
already moves the receipt to `Receiving`, so `Transport::cancel_resource`
could fail a receipt whose response receiver was running and leave that
receiver in place. Upstream site: `transport_sections/reset_out_link.rs`
has `cancel_resource` collect the segments
`Link::note_request_resource_outcome` hands back under the lock scope and
pass them to `cancel_inbound_responses` once both guards are gone, so the
receiver is dropped and, on an active link, the peer is sent a receiver
cancel (`RNS/Link.py:1435`, `RNS/Resource.py:1111-1116`). Tests:
`link_sections/pending_requests_tests.rs` pins `OutboundCancelled` for an
entry already `Receiving` failing it `Send` and naming the segment in
flight.

Fork commit `6a0fc9cb`: the cancelled-request probe adopted and the receipt
fixtures tidied. `two_transport_loopback_requests_receipt_probe4.rs` (new,
one more `include!` line in `transport/tests.rs`) cancels a request
Resource through `Transport::cancel_resource` while the responder is
stalled and pins the receipt failing `Send` on every clone with no clock
started and no sizes, the `OutboundCancelled` event for the hash, a cancel
naming nothing in flight leaving the receipt as it was, a second cancel of
the same hash finding nothing, and the link answering afterwards.
`resource.rs` makes `FLAG_RESPONSE` `pub(crate)` and
`two_transport_loopback_requests_receipt_probe3.rs` reads it instead of
its own copy; `two_transport_loopback_requests_receipt_stop.rs` spells its
link-drop bound as `LOOPBACK_TIMEOUT`, says what the holder's `Err` means
and pins `stop` returning no earlier than the budget;
`two_transport_loopback_requests_receipt_probe.rs` drops two asserts the
surrounding `timeout` and the `expected` bound already carry;
`resource/tests_timeouts_lifecycle.rs` sizes its request ids by
`ADDRESS_HASH_SIZE`; the `tests/request_receipt_api.rs` header claims
only what it pins. The `RNS/Link.py` failure-branch cite is `:1377-1385`
in `link_sections/pending_requests_tests.rs` and the crate `CHANGELOG.md`.

Fork commit `bbd34e72`: the receiver-drop comments say which close path
they belong to, and `resolve` is `#[must_use]`.
`link_sections/next_watchdog_deadline.rs` had the two sentences crossed:
`finalize_local_close` now says the transport drops the link's receivers
with its state on the next link check, and `restart()` that the link
stays so a receiver left inbound fails on its own retry limit;
`link_sections/new.rs` keeps only the sentence about the id changing
below. `request/receipt_pending.rs` marks `PendingRequest::resolve`
`#[must_use]`, the cause being for the caller to log, and
`request/receipt_tests.rs` asserts the `Ready` path hands back `None`;
`request/receipt.rs` records that `MetadataRead` and `MetadataTrailing`
are defence in depth behind the walker, kept without a synthetic pin.

Fork commit `114dbe1c`: the receipt re-read probe adopted, and what a
dropped sender does to a subscriber.
`two_transport_loopback_requests_receipt_probe5.rs` (new, one more
`include!` line in `transport/tests.rs`) pins `response()` as a read that
returns the same body or the same failure on every repeat, on the same
clone or on one taken after the conclusion, with `concluded_at()`
unmoved; a subscriber taken after the conclusion reading the final status
and being reported no change; and a receipt every clone of which is
dropped before the response arrives leaving the link answering the next
request in both forms: in packet form its response still reaches the
link's broadcast, in resource form the responder still serves the request
under the id the receipt reported (the response Resource's own arrival on
the broadcast is pinned by `6f8cba34` below). Upstream site:
`request/receipt.rs` has `subscribe()` say that a link which goes away
without concluding the request drops the sender, so `changed()` returns
an error rather than a `Failed` change and `borrow()` keeps the last
status, with `status()` and `response()` concluding the receipt
`LinkClosed`, and the `ResponseDecodeFailure` doc drops its
review-process clause; `request/receipt_tests.rs` binds its shallow metadata
vector mutable in one step. The crate `CHANGELOG.md` names the fixture
in the `### Added` pin clause and `probe4` and `pending_requests_tests.rs`
in the `### Changed` one.

Fork commit `6f8cba34`: the vanished-link subscriber probe adopted, and
dropping a receipt said to cancel nothing.
`two_transport_loopback_requests_receipt_probe6.rs` (new, one more
`include!` line in `transport/tests.rs`) pins what a `subscribe()`
receiver sees when the link goes away without concluding the request,
held through `Transport::stop` the way the stop fixture does and dropped
afterwards: `changed()` errors out rather than reporting a `Failed`
change, `borrow()` keeps the last status the link published, `Sent` for
a packet-form receipt and `Delivered` for a resource-form one, and
`status()`, `concluded_at()` and `response()` conclude the receipt
`LinkClosed` on either clone without moving any subscriber.
`two_transport_loopback_requests_receipt_probe5.rs` has its dropped
resource-form receipt's response observed on `resource_events()` as a
`Complete` with `is_response` and the dropped id, carrying the echoed
envelope; its late subscriber asserts that the conclusion dropped the
sender (`changed()` is `Err` at once); its one-tick bound is
`one_tick_bound()`. Upstream site: `request/receipt.rs` has `subscribe()`
say the sender is dropped once the request concludes or the link goes
away unconcluded, and `RequestReceipt` say that dropping every clone
cancels nothing. The crate README says the same after its
`Sending requests` paragraph, and the crate `CHANGELOG.md` names the new
fixture in the `### Added` pin clause and the `*_with_mtu` constructors
in the `### Changed` one.

Fork commit `1ef4cddd`: the early-subscriber probe for a normal conclusion
adopted. `two_transport_loopback_requests_receipt_probe7.rs` (new, one
more `include!` line in `transport/tests.rs`) pins what a `subscribe()`
receiver taken before the request concludes sees once it does, as the
`subscribe()` doc says: the terminal status is reported as the last
change, then `changed()` returns an error rather than a further change,
`borrow()` keeps the terminal status, `has_changed()` keeps erroring, and
reading the receipt afterwards (`status()`, `response()`,
`concluded_at()`) on two clones moves nothing the receiver sees. Covered
for a packet-form `Ready`, a resource-form `Ready` that passed through
`Delivered`, and a `Timeout` failure, where a subscriber taken after the
failure reads `Failed` and no change. The crate `CHANGELOG.md` names the
fixture in the `### Added` pin clause.

Fork commit `f732e7d4`: the receiver-cancel helpers of
`transport/resource_wire.rs` (`receiver_cancel_packet`, `receiver_cancel`,
`cancel_inbound_response`, `note_resource_events_on_links`) move verbatim
to `transport/resource_wire_parts/cancel.rs`, an `include!`d part spliced
in at the same position, so `resource_wire.rs` stays under the 500-line
module cap once the response-size gate grows. No behaviour change.

Fork commit `abd9187d`: the `max_response_size` gate on a response
Resource reads the advertised data size `d` (`RNS/Link.py:1049`,
`RNS/Resource.py:1285-1287`) instead of the compressed transfer size
`t`, so a compressible response over the limit is refused however small
its transfer, and reads it at every segment advertisement without
spending it: `Link.pending_response_limits` becomes
`HashMap<Vec<u8>, Option<usize>>` (`Some` the limit the request went out
with, `None` a request sent without one), `Link::response_size_allowed`
reads an entry without removing it, and the entry goes when the response
resolves, fails or is refused; `take_response_limit_if_allowed` keeps its
one-shot shape for the packet-response path. A response advertisement
whose `q` names neither a pending receipt nor an entry recorded by the
new `Link::expect_response` is dropped before the ResourceManager, logged
at debug and not rejected, as Python's pending-request loop
(`RNS/Link.py:1046-1047`) falls through it;
`request_packet_with_max_response_size` and `send_request_resource*`
record their id so the responses they solicit are admitted, and the
record is dropped when the advertisement is not sent. Failing a receipt
that was receiving a response hands back
`InboundResponseHashes { segment, original }` (`request/receipt_pending.rs`)
instead of the bare segment hash, and
`ResourceManager::cancel_incoming_response` (`resource/manager_cleanup.rs`)
cancels whichever form the transfer is in: the live receiver under the
segment hash, or, between one segment's completion and the next admitted
advertisement, only the partial assembly under the original hash; the
peer is owed a receiver cancel only when a segment was in flight
(`resource_wire_parts/cancel.rs`, `transport_sections/request.rs`,
`transport/lifecycle.rs`). `SendPacketOutcome::is_sent` (`transport/mod.rs`)
replaces the same `matches!` in `delivery.rs`, `link_fanout.rs`,
`reset_out_link.rs`, `resource_reader.rs` and `request.rs`. New fixture
`two_transport_loopback_requests_resource_form_sizes.rs` (one `include!`
line in `transport/tests.rs`) pins the status order in all four
request/response form pairs, a three-segment response's sizes and
monotone progress, the zeros-body `Oversize` refusal by `d`, admission
within the limit by `d`, the silent unsolicited drop and a split response
admitted whole. `pending_requests_tests.rs` and `receipt_tests.rs` follow
the `InboundResponseHashes` shape and pin the per-advertisement gate; the
`two_transport_loopback_resources*.rs` fixtures that sent a bare response
now solicit it with a request Resource first. The crate README says what
`max_response_size` is measured against; the crate `CHANGELOG.md` gains
a `### Changed` clause naming every fixture above.

Fork commit `938d2852`: `pending_requests_tests.rs` pins `Failed(Decode)`
through `Link::resolve_pending_request`: a response that is not an
envelope fails its receipt `Decode`, the entry and its response size
limit go with it, and a second copy of the response finds nothing
waiting.

Fork commit `6b5e9a4e`: `OutboundRequestForm::Packet` in
`transport_sections/request.rs` holds its `Packet` behind a `Box`, as
`delivery.rs` and `resource/sender.rs` hold theirs, since carrying
`InboundResponseHashes` in `displaced` pushed the variant past clippy's
`large_enum_variant` threshold. No behaviour change.

Fork commit `9521e2c0`: a response Resource without metadata carries the
`[request_id, body]` envelope, and its advertisement's `q` chose the
receipt; Python reads the id out of the envelope and keys
`handle_response` on it (`RNS/Link.py:908-912`). `ResponseArrival::decode`
(`request/receipt.rs`) now takes the receipt's id from
`PendingRequest::resolve` (`request/receipt_pending.rs`) and, for that
form only, fails a mismatching envelope with the new
`ResponseDecodeFailure::RequestIdMismatch { expected, actual }`, which the
receipt reports as `Failed(Decode)` and `Link::resolve_pending_request`
logs with both ids. The packet form is keyed on its own envelope and the
file form has no envelope, so neither is checked. `receipt_tests.rs` pins
the mismatch and the match; the crate `CHANGELOG.md` names the mismatch
under `Decode`.

Fork commit `b8ec835a`: `RequestResponseData` (`request/receipt.rs`)
wraps its body in an `Arc`, so the clone `RequestReceipt::response` hands
every caller, and any clone of the result, shares one body instead of
copying a file response; `into_value` moves the body out of the last
handle and clones it out of a shared one. The three `#[allow(dead_code)]`
on `MIN_LINKS_CHECK_DELAY`, `link_check_delay_from_deadline` and
`next_link_check_delay` in `transport/jobs.rs` go: all three are live,
`jobs_parts/tests.rs` pins `link_check_delay_from_deadline` with
`MIN_LINKS_CHECK_DELAY` as one expected value, and `next_link_check_delay`
is reached only through the link check job. `receipt_tests.rs` pins the
sharing and both `into_value` paths; the crate `CHANGELOG.md` says the
handle is cheap.

Fork commit `8b7557bd`: new fixture
`two_transport_loopback_requests_errors.rs` (one `include!` line in
`transport/tests.rs`) pins `Transport::request` on a link the connector
knows but that is not `Active`: `RnsError::ConnectionError`, staged with
a `Pending` out-link toward a destination nobody serves, which stays in
the table for its establishment timeout where a `close()`d link is
removed by the link maintenance job within a second.
`two_transport_loopback_requests_receipt_stop.rs` gains a second connector
link to a sibling destination: through `Transport::stop` the held link's
receipt stays `Sent` while the other link's receipt concludes
`LinkClosed` inside `stop`. The displaced same-id case is not stageable
through `Transport::request` (the packet-form id is the hash of a freshly
encrypted packet and both forms pack the send time into the request) and
stays pinned at the link level in `pending_requests_tests.rs`. The crate
`CHANGELOG.md` names the new fixture in the receipts pin clause and says
the sweep skips only the held link.

Fork commit `f2ff9922`: every request packet the link sends solicits its
response, however it was built. The unsolicited-advertisement drop of
`abd9187d` admitted a response Resource only when its `q` named a pending
receipt or an entry written by `Link::expect_response`, and only
`request_packet_with_max_response_size`, `set_response_size_limit` and
`send_request_resource*` wrote one, so a response too large for a packet
to a request built with the public `Link::request_packet` was dropped.
`TransportHandler::note_link_packet_sent` (`transport/handler.rs`), which
already locks the link for every outbound link packet, records the
truncated packet hash of a `Request`-context packet through the new
`Link::note_request_sent` (`link_sections/response_packet.rs`), an entry
that never overwrites a limit recorded before the send
(`RNS/Link.py:473-510`, `:1365`). The `requests`, `requests_probe` and
`requests_routing` fixtures go back to the bare `request_packet` builder,
leaving them as they were before `abd9187d`, and
`two_transport_loopback_requests_resource_form_sizes.rs` gains
`..._sizes_bare_request_packet_solicits_its_response`. The crate README
and `CHANGELOG.md` say what solicits a response.

Fork commit `60cb3755`: `send_request_as_resource`
(`transport_sections/request.rs`) calls `Link::expect_response`
unconditionally, so a request sent without a limit under an id a
displaced entry recorded one for does not inherit it;
`Link::fail_pending_request` (`link_sections/pending_requests.rs`) drops
the response size limit before the receipt lookup, so a limit with no
receipt under the id goes too, and the `clear_response_size_limit` in
the `Oversize` branch of `transport/resource_wire.rs` stays for a
`send_request_resource*` id of another length.
`ResourceManager::cancel_incoming_response` (`resource/manager_cleanup.rs`)
takes the link id the hashes came from and fails a receiver or a partial
assembly only when it belongs to that link; `resource_wire_parts/cancel.rs`,
`transport_sections/request.rs` and the receipt sweep in
`transport/lifecycle.rs` pass it. `PendingRequest::fail`
(`request/receipt_pending.rs`) and `Link::fail_pending_request`,
`fail_all_pending_requests` and `note_request_resource_outcome` are
`#[must_use]`; the close and restart paths of `link_sections/new.rs` and
`next_watchdog_deadline.rs`, which hold the link alone, go through the
new `Link::abandon_pending_requests`, which logs the response Resources
left inbound. `PendingRequest::resolve` records that the packet-form
sizes diverge from Python's `len(packb(response)) - 2`
(`RNS/Link.py:1016-1017`) by two bytes. `resource/tests_timeouts_lifecycle.rs`
pins the link-scoped cancel; `pending_requests_tests.rs` and
`receipt_tests.rs` assert the results they discarded.

Fork commit `e28c5a03`: new fixture
`two_transport_loopback_requests_resource_form_sizes_probe.rs` (one
`include!` line in `transport/tests.rs`) pins, on the response Resource
path, a hand-built envelope naming another request failing the receipt
`Decode` with no reject on the wire and one naming the receipt resolving
it, every `response()` handle sharing one body, a response advertised
under the id of a resolved receipt dropped as unsolicited, and a
packet-form request whose Resource response is over `max_response_size`
failing `Oversize` while the responder sees `OutboundRejected`. The crate
`CHANGELOG.md` names it in the receipts and the `max_response_size` gate
pin clauses.

Fork commit `1da7243b`: formatting only, `rustfmt` run over
`two_transport_loopback_requests_resource_form_sizes.rs`, which is
`include!`d into `transport/tests.rs` and so is never seen by `cargo fmt`;
six hunks: four lines ran past the 100-column width and two took
rustfmt's preferred shape.

Fork commit `7aaf7a50`: an over-limit response packet for a receipt
already receiving a response Resource fails it `Oversize` and cancels
that Resource with a receiver cancel to the peer (`RNS/Link.py:1435`);
the Response branch of `Link::handle_packet` (`link_sections/new.rs`)
had asserted that nothing was inbound, a premise the non-consuming gate
of `abd9187d` broke. `handle_packet` holds the link alone and its
public `LinkHandleResult` is unchanged, so the hashes queue on a new
private `Link::owed_response_cancels` (`linkhandleresult.rs`,
initialised in `new.rs` and `new_from_request.rs`), drained by
`Link::take_owed_response_cancels` (`pending_requests.rs`);
`transport/wire.rs` drains it after each `handle_packet` call and, once
the proofs are sent, re-locks each link to run
`resource_wire::cancel_inbound_response` (`resource_wire_parts/cancel.rs`,
now `pub(super)`), publishes the resource events and sends the cancels.
`pending_requests_tests.rs` drives the over-limit packet from the peer
link through `handle_packet`; the new fixture
`two_transport_loopback_requests_resource_form_sizes_cancel.rs` (one
`include!` line in `transport/tests.rs`) admits an unbacked response
advertisement, sends the oversize packet from the listener's in-link
and counts one receiver cancel on the listener tap. Also:
`SendPacketOutcome::is_sent` (`transport/mod.rs`) and
`Link::insert_pending_request` are `#[must_use]`;
`Link::note_request_sent` (`link_sections/response_packet.rs`) is
`pub(crate)`; the rustdoc of `request_packet_with_max_response_size` no
longer says a `request_packet` requester's response is dropped; the
send seam in `transport/handler.rs` cites `RNS/Link.py:473-510`; the
gate test pins that `note_request_sent` keeps a prior limit. The crate
`CHANGELOG.md` gains the cancel clause and an `is_sent` bullet.

Fork commit `f575a790`: new fixture
`two_transport_loopback_requests_resource_form_sizes_probe2.rs` (one
`include!` line in `transport/tests.rs`) pins the solicited-response
marker from the consumer's side: a bare request packet sent through
`send_packet_broadcast_with_trace` is answered by a Resource response,
`request_packet_with_max_response_size` refuses `d` over its limit and
admits `d` at the limit and any `d` without one, and a receipt and a
bare request packet interleaved on one link each get their own response.
The header of `two_transport_loopback_requests_resource_form_sizes_probe.rs`
names the divergence its first test pins: Python resolves the receipt
the envelope's id names and leaves `q`'s receipt pending
(`RNS/Link.py:908-912`); here the two must agree or `q`'s receipt fails
`Decode`. The crate `CHANGELOG.md` names the fixture in the
`max_response_size` gate pin clause.

Fork commit `ad9f6b8a`: new fixture
`two_transport_loopback_requests_resource_form_sizes_cancel_probe.rs`
(one `include!` line in `transport/tests.rs`) pins the packet-form
Response arm of a receipt already receiving a response Resource from
three sides: an in-limit packet resolves it `Ready` with the packed
length as its size and no receiver cancel leaves; an oversize packet
fails only the receipt it names, leaves a second receiving receipt on
the same link untouched and sends exactly one receiver cancel; an
oversize packet for a receipt still `Sent` fails it `Oversize` and owes
no cancel. The second test panicked on the assertion `7aaf7a50`
replaced. The crate `CHANGELOG.md` names the fixture in the receipts
`Changed` pin clause.

Fork commits `0d4c9342` and `8eebdfab`: the solicited-response record
gets a lifetime. `Link::pending_response_limits`
(`link_parts/linkhandleresult.rs`) is keyed as before but its value is
the new crate-private `PendingResponseLimit { limit, expires_at }`;
`expect_response` and `note_request_sent`
(`link_sections/response_packet.rs`) stamp `expires_at` with the default
request timeout (`RequestOptions::default().timeout_for_rtt(Link::rtt())`,
`RNS/Link.py:491`) from now, the send refreshing the stamp of a limit
recorded before it, and the three readers read `.limit`.
`Link::expire_pending_requests` (`link_sections/pending_requests.rs`)
prunes an entry past its stamp unless a live receipt owns the id, and
`earliest_pending_request_deadline` folds the unowned stamps in so the
watchdog wakes for them; a receipt's entry still goes with the receipt.
Python's record is the `RequestReceipt`, pruned by its own timeout job
when `DELIVERED` (`RNS/Link.py:1397-1405`). `module_prelude.rs` imports
`RequestOptions`.
`pending_requests_tests.rs` pins a bare-send record and a
`send_request_resource*` record gone after the timeout while a `Sent`
and a `Receiving` receipt keep theirs. `8eebdfab` rewords the field doc.
The crate `CHANGELOG.md` gains the lifetime sentence in the
`max_response_size` gate bullet.

Fork commit `181c4a27`: `Link::resolve_pending_request`
(`link_sections/pending_requests.rs`) measures an assembled response
Resource against the limit the request went out with once more, metadata
excluded, and fails the receipt `Oversize` when it is over, returning the
segment it was receiving like the sibling failers (`#[must_use]`); the
advertised `d` the gate read was the peer's claim, and an uncompressed
response carries up to `t` bytes. Python trusts the advertisement
(`RNS/Link.py:1049`). `transport/resource_wire.rs` routes the returned
hashes through `cancel_inbound_response`; `link_sections/new.rs` extends
`owed_response_cancels` from the Response packet arm's result.
`pending_requests_tests.rs` pins a `d = 10` advertisement whose assembled
envelope is 2000 bytes against a 1024 limit, and an envelope of exactly
1024 bytes admitted. The crate `CHANGELOG.md` gains the sentence in the
`max_response_size` gate bullet.

Fork commit `ca4d1aa4`: new fixture
`two_transport_loopback_requests_resource_form_sizes_probe3.rs` (one
`include!` line in `transport/tests.rs`) pins the packet-form limit as
inclusive of the `response_size()` an unlimited request reports, three
Resource responses in flight on one link each under their own limit all
resolving `Ready`, and one refused response leaving its two neighbours
and the link intact with the same body admitted again under a covering
limit. The crate `CHANGELOG.md` names the fixture in the
`max_response_size` gate pin clause.

Fork commits `6b3ab18e` and `bb8a94ad`: `RequestResponseData::into_value`
(`destination/request/receipt.rs`) returns `None` for a file response
before touching the `Arc`; the two `[resource-diag]` lines
`drop_unsolicited_response_advertisement` and
`response_advertisement_request_id_len` (`transport/resource_wire.rs`)
carry `node=`; `transport/handler.rs` and
`link_sections/response_packet.rs` derive the request id through
`RequestId::from_packet_hash`; `resource/tests_timeouts_lifecycle.rs`
pins `cancel_incoming_response` for a live receiver (another link's
cancel is a no-op, its own link's drops it and owes one receiver cancel);
`two_transport_loopback_resources_reject_response.rs` sends its bounded
request under a 19-byte id so the non-`RequestId` clear arm in
`resource_wire.rs` is reached; the
`two_transport_loopback_requests_resource_form_sizes_probe2.rs` header
names `send_prepared_packet_broadcast_with_trace` as the seam the marker
does not see. `bb8a94ad` rewords the crate README on the `Oversize`
paths and the not-`Active` refusal and adds the crate `CHANGELOG.md`
sentence on an in-limit response packet for a receipt already receiving
a response Resource (`RNS/Link.py:873`, `:1437-1443`).

Fork commit `d042612c`: the rustdoc of
`request_packet_with_max_response_size` names the crate-private
`Link::note_request_sent` in a code span instead of an intra-doc link.

Fork commit `e5183c64`: the receipt-less solicited-response record
follows a receipt's lifetime. `PendingResponseLimit`
(`link_parts/linkhandleresult.rs`) carries `clock: RecordClock` and
`request_resource: Option<Hash>` in place of `expires_at`; the new
crate-private `enum RecordClock { Expires(Instant),
Receiving(InboundResponseHashes), Unarmed }` with `has_run_out(now)`
runs from the send for a request packet and from the request Resource's
`OutboundComplete` for the resource form (`RNS/Link.py:1370-1373`),
stops while a response Resource is inbound (`:1389` never times out
`RECEIVING`), and only a run-out `Expires` entry is pruned
(`:1397-1405`, a `DELIVERED` receipt past its timeout). In
`link_sections/response_packet.rs`, `expect_response` arms `Expires`,
the new crate-private `expect_response_to_resource(request_id, limit,
request_resource)` leaves `Unarmed`, `note_request_sent` re-arms an
existing entry and keeps its limit, `pending_response_expiry` returns
the `RecordClock`, and `response_size_allowed` returns `None` for an
entry whose clock ran out ahead of the watchdog. In
`link_sections/pending_requests.rs`,
`mark_pending_request_receiving(&[u8], &ResourceAdvertisement)` takes
the receipt when one exists and otherwise moves the entry to
`Receiving`; `note_request_resource_outcome` clears a `Receiving` entry
on `InboundFailed` by its original hash and arms (`OutboundComplete`)
or drops (`OutboundRejected`, `OutboundFailed`, `OutboundCancelled`) an
entry by its request Resource hash; `expire_pending_requests` prunes
only run-out `Expires` entries and `earliest_pending_request_deadline`
folds only unowned `Expires` stamps. `transport/resource_wire.rs`
carries `(Vec<u8>, ResourceAdvertisement)` for any admitted id into the
receiving hook, not only a 16-byte one;
`transport_sections/reset_out_link.rs` has
`send_request_resource_with_max_response_size` record through
`expect_response_to_resource` with the tracked hash. The four parity
glosses that cited `RNS/Link.py:1398-1411` now state the rule against
`:1397-1405`, `:1389` and `:1370-1373`. `pending_requests_tests.rs`
pins an entry without a receipt held while its response is inbound, a
resource-form entry armed by its request Resource, a run-out clock
admitting nothing until the send restarts it, and the reworded expiry
on the default request timeout. The crate `CHANGELOG.md` restates the
record's lifetime in the `max_response_size` gate bullet.

Fork commit `64d49999`: new fixture
`two_transport_loopback_resources_receipt_less_receiving.rs` (one
`include!` line in `transport/tests.rs`) solicits a response under a
17-byte id over `LEGACY_MTU`, answers with the two-segment body
`two_transport_loopback_resources_two_segments` sends, and stalls the
listener for the default request timeout plus margin once the first
`Progress` has landed, so the second segment's advertisement follows
the deadline a running clock would have set at delivery; both segments
must still assemble. On 32a35552 the receiver timed out waiting for
`Complete`.
`two_transport_loopback_resources_retries.rs` gains
`stall_transport_for(transport, Duration)`, with `stall_transport`
delegating under `RETRY_STALL`. The crate `CHANGELOG.md` names the
fixture in the `max_response_size` gate pin clause.

Fork commit `08794d16`: the legacy `Err(_)` arm of the response
`Complete` in `transport/resource_wire.rs` measures an assembled
response to a `send_request_resource*` id of another length against the
limit the request went out with, as `resolve_pending_request` does for
a receipt: it reads `response_size_limit`, calls
`take_response_limit_if_allowed(request_id, complete.data.len())`,
which drops the entry either way, warns when over and records the event
index in `suppressed`, and those indices are removed from `events`
before `publish_resource_events`. The Resource is already assembled and
gone from the ResourceManager, so the withheld event is the failure and
no cancel is owed. `pending_requests_tests.rs` pins the verdict and the
removal for a 17-byte id over its limit. The crate `CHANGELOG.md` gains
the sentence in the `max_response_size` gate bullet.

Fork commit `f7922574`: new fixture
`two_transport_loopback_requests_resource_form_sizes_probe4.rs` (one
`include!` line in `transport/tests.rs`) pins the receipt-less record's
lifetime as a slow responder sees it: a bare request packet answered
halfway through the default request timeout is admitted while one
answered past it is dropped without a reject, a receipt-owned record
outlives the default timeout, and `into_value` of a file response is
`None` on every handle. The expiry test anchors its wait at the legacy
request Resource's `OutboundComplete` on the connector, so it covers
the packet record's clock from the send and the Resource record's from
delivery. The crate `CHANGELOG.md` names the fixture in the
`max_response_size` gate pin clause.

Fork commit `8549dd6f`: the reject test in
`two_transport_loopback_resources_reject_response.rs` sends a second
response under the rejected id before its recovery block and pins that
the reject cleared the requester's record: no `Complete` reaches the
requester within `RESOURCE_SETTLE` and no receiver cancel lands on the
listener's interface tap. The
`two_transport_loopback_requests_resource_form_sizes_probe3.rs` header
drops the claim that its retry observes a leftover limit (the retry is
keyed by a fresh id) and its two helpers carry the `sizes_probe3_`
prefix. `request/receipt_tests.rs` renames
`into_value_moves_the_body_out_of_a_shared_handle_too` to
`into_value_clones_out_of_a_shared_handle_and_moves_out_of_the_last`.
The crate `CHANGELOG.md` names the second-response drop in the
`max_response_size` gate pin clause.

Fork commit `b836157a`: the crate README names the three `Oversize`
paths (a response Resource refused at its advertisement, a response
packet dropped, a response Resource failed after it assembles over the
limit) and states how long a solicitation lasts. The
`two_transport_loopback_requests_resource_form_sizes_probe2.rs` header
names the mechanism that keeps
`send_prepared_packet_broadcast_with_trace` out of the record: it hands
the packet to the interface manager and never passes
`TransportHandler::send`. Comments: the receipt arm in
`transport/resource_wire.rs` says why its cancel call cancels nothing,
the Response arm in `link_sections/new.rs` says why only its `Oversize`
branch can owe one, and the initiator retry test in
`two_transport_loopback_resources_retries.rs` says why `RETRY_STALL`
leaves the record armed. `rustfmt` also reflowed one pre-existing line
in `new.rs`.

Fork commit `30f98996`: new fixture
`two_transport_loopback_requests_resource_form_sizes_probe5.rs` (one
`include!` line in `transport/tests.rs`) pins the end of a request
record's life and a response Resource whose advertised `d` was a lie: a
response admitted on an advertised size under the receipt's bound and
assembled over it fails the receipt `Oversize`; a completed and a
remotely cancelled response each end a receipt-less
`send_request_resource` record, so a second response under the id is
dropped without a reject while a fresh request on the link is still
answered; a bare `request_packet` record ends the same way on
completion; and the legacy-id limit is inclusive, a response of exactly
the limit admitted at its advertisement and delivered after assembly
while one byte more is refused at the advertisement with
`OutboundRejected` on the responder. The crate `CHANGELOG.md` names the
fixture in the `max_response_size` gate pin clause.

Fork commit `55736b0b`: the response `Complete` arm in
`transport/resource_wire.rs` measures the assembled response against the
record's limit before the id-length split, so the `Complete` for a
receipt-owned or a receipt-less 16-byte id over its limit is withheld
from `resource_events()` as the legacy-length id's already was; the warn
moves with the measurement, the `Err(_)` arm keeps only its
`take_response_limit_if_allowed` call, and
`Link::resolve_pending_request` (`link_sections/pending_requests.rs`)
logs the same case at debug. The probe5 `Oversize` test now also asserts
that no `Complete` for the receipt's id reaches the connector's
`resource_events()`. New `response_packet_tests.rs` (one `include!` line
in `link_parts/link.rs`, since `pending_requests_tests.rs` is at the
module-size cap) pins a receipt-less 16-byte entry reporting its limit
through `response_size_limit` and going when
`take_response_limit_if_allowed` refuses the assembled size. The crate
`CHANGELOG.md` `max_response_size` gate bullet now states the recheck
for every id form.

Fork commit `f10a7843`: `Link::set_response_size_limit`
(`link_sections/response_packet.rs`) edits only the limit of a record
already under the id (`entry().and_modify().or_insert()`), leaving its
clock and request Resource as they were, and `expect_response` and
`expect_response_to_resource` keep a limit already recorded when given
`None`, so the limit and the record a request makes compose in either
order. New crate-private `Link::replace_response_record` does the
unconditional insert, and both forms of `Transport::request`
(`transport_sections/request.rs`) use it so a request sent without a
limit under a displaced id does not inherit the stale one; the packet
form builds with `request_packet` and then writes the record itself.
`response_packet_tests.rs` gains rows for both orderings and for the
writer replacing the record whole. The crate `CHANGELOG.md` gains a
bullet for the composition.

Fork commit `09f6c98b`:
`two_transport_loopback_resources_receipt_less_receiving.rs` stalls for
a 2 s margin instead of 3 s, so the stall still outlasts the record's
deadline by an `INTERVAL_LINKS_CHECK` tick but ends inside the connector
link's stale window (the header works out the ceiling from `stale_time`,
`STALE_GRACE_SECS` and `KEEPALIVE_TIMEOUT_FACTOR`). The record timeout
comes from `RequestOptions::default().timeout_for_rtt`, the `Progress`
predicate is keyed on the first segment's hash rather than the link id,
and `assert_ne!(status, LinkStatus::Closed)` after the stall pins that
the link survived it.

Fork commit `21fc1c3f`: `Link::note_request_sent` re-anchors only an
`Expires` clock, so a request packet re-sent under the id of a record
receiving its response, or of a request Resource still in flight, leaves
that record as it is. New crate-private `Link::take_response_record`
removes a record and returns the inbound hashes it was receiving, and the
rejected-advertisement path in `transport/resource_wire.rs` cancels that
partial assembly instead of leaving it in the ResourceManager. New
`response_record_tests.rs` (one `include!` line in `link_parts/link.rs`)
pins the re-send, the take, and `note_request_resource_outcome` handing
back the response a rejected request Resource's entry was receiving; the
`hash`, `hashes` and `response_segment` helpers in
`pending_requests_tests.rs` become `pub(super)` for it. The
`two_transport_loopback_requests_resource_form_sizes_probe4.rs` helpers
carry the `sizes_probe4_` prefix and its silence check delegates to
`sizes_assert_no_response_activity`; the
`two_transport_loopback_requests_resource_form_sizes_probe3.rs` doc drops
the clause about a retry under a covering limit. The crate README says
an id a `Transport::request` receipt owns is solicited for the receipt's
own timeout. Comments: `InboundResponseHashes`
(`request/receipt_pending.rs`) says a receipt-less record carries the
same pair in `RecordClock::Receiving`, and
`resource/tests_timeouts_lifecycle.rs` says a first segment's original
hash is its own. The crate `CHANGELOG.md` gains a bullet for the clock
and the cancel.

Fork commit `f7d8c60d`: new fixtures
`two_transport_loopback_requests_resource_form_sizes_probe6.rs` and
`two_transport_loopback_requests_resource_form_sizes_probe7.rs` (two
`include!` lines in `transport/tests.rs`). probe6 pins
`Link::set_response_size_limit` composed with the public request builders:
a limit set before or after a `send_request_resource` call, or before a
bare `request_packet` is sent, is kept when the send seam re-arms the
record's clock, one byte over it refused with `OutboundRejected` on the
responder and exactly the limit delivered; and a response whose advertised
`d` lied under the limit of a receipt-less 16-byte or a legacy-length
record is caught at the post-assembly measurement, its `Complete`
withheld, the record cleared.
probe7 pins the public `expect_response` keeping a prior limit when given
`None` and replacing it when given `Some`, in both directions. The probe5
header splits the Python cite (`d` measured and trusted at the
advertisement) from this crate's assembled-bytes recheck, its size
constants say what each length buys under the 256 KiB loopback MTU, and
the lying-`d` re-advertisement notes that the payload has to be
incompressible because the receiver caps decompression by the advertised
`d`. The crate `CHANGELOG.md` names probe6 in the `max_response_size`
gate pin clause and probe7 under the `set_response_size_limit` bullet.

Fork commit `f68479d5`: every writer in
`link_sections/response_packet.rs` that removes or replaces a record
hands back the `InboundResponseHashes` a `RecordClock::Receiving` record
held, so a receipt-less record displaced while its response Resource is
inbound has that transfer cancelled as a receipt's would be. New
crate-private `Link::take_response_limit` returns `(allowed, hashes)`,
the hashes only on refusal; the public `take_response_limit_if_allowed`
keeps its signature as a wrapper that owes them on the link. `replace_response_record`
and `expect_response_to_resource` return the displaced hashes under
`#[must_use]`, and the public `expect_response` owes them itself.
`PendingResponseLimit::into_receiving` (`link_parts/linkhandleresult.rs`)
is the shared projection. The Response arm in `link_sections/new.rs`
calls the crate-private writer and owes the hashes, with its comment
covering both sources of the one cancel and the legacy-length id's
exemption from the packet-path gate; both forms of `Transport::request`
(`transport_sections/request.rs`, `displaced` now a `Vec`) fold the
record's hashes into the cancel they already send, and
`send_request_resource_with_max_response_size`
(`transport_sections/reset_out_link.rs`) cancels them after the link
guard is released. `response_record_tests.rs` gains rows for each writer
and for the packet handled under a receipt-less receiving record owing
exactly one cancel; `response_packet_tests.rs` and
`pending_requests_tests.rs` assert `None` where the new returns were
ignored. New fixture
`two_transport_loopback_requests_resource_form_sizes_cancel2.rs` (one
`include!` line in `transport/tests.rs`) pins one
`ResourceReceiverCancel` reaching the responder and the transfer
reported `InboundFailed` on the connector. `rustfmt` also reflowed one
pre-existing line in `request.rs`. The crate `CHANGELOG.md` gains a
bullet for the hand-back.

Fork commit `c13491a7`: new
`sizes_assert_no_response_activity_under(events, link_id, ids)` in
`two_transport_loopback_requests_resource_form_sizes.rs` watches every
id in one silence window, since an event read off the receiver in a
window for one id is gone for the next; `sizes_assert_no_response_activity`
wraps it for one id, and a closed event channel panics as in
`sizes_sample_progress` while the timeout stays the clean return.
`two_transport_loopback_requests_resource_form_sizes_probe4.rs` passes
both late ids in one call and its doc says why; the local copy in
`two_transport_loopback_requests_resource_form_sizes_probe5.rs` is gone
and its call sites there and in probe6 use the shared helper. The sizes
header names `two_transport_loopback_requests_receipt_probe3.rs` as the
fixture behind its not-spent clause, and the `windows(2)` closure in
`two_transport_loopback_requests_resource_form_sizes_probe3.rs` binds
`window`.

Fork commit `28d7e69b`: the legacy-length arm of the response `Complete`
handling in `transport/resource_wire.rs` takes its record through the
crate-private `take_response_limit` and consumes the hashes through
`cancel_inbound_response` as the 16-byte arm does, instead of owing a
cancel for a Resource already assembled; and the manager events a
receiver cancel produces are drained and published with the packet that
caused them on the rejected branch too, not left for the next resource
packet or the periodic worker.

Fork commit `6ce0e712`: the crate `README.md`, the crate `CHANGELOG.md`
receipt bullet and the `RequestReceipt` rustdoc
(`destination/request/receipt.rs`) qualify the dropped-receipt sentence:
a response assembling over the receipt's `max_response_size` has its
`Complete` withheld. The refused-segment cancel bullet in the
`CHANGELOG.md` claims only the Link half its unit rows pin. The
`note_request_sent` rustdoc (`link_sections/response_packet.rs`) says an
`Expires` clock is re-armed whether or not it has run out.

Fork commit `4309e2ee`: new fixture
`two_transport_loopback_requests_resource_form_sizes_probe8.rs` (one
`include!` line in `transport/tests.rs`) pins the public
`expect_response` over a receipt-less record receiving its response: the
displaced transfer is cancelled with one receiver cancel on the link's
next inbound packet and the replacing record keeps or sets the limit; a
later advertisement over the limit of a record receiving again cancels
that partial assembly too. Of the two `CHANGELOG.md` pin clauses it
touches, the one that named the reject path as unpinned now cites it and
the other gained it as a pin.

Fork commit `62cd6799`: `handle_link_resource_packet`
(`transport/resource_wire.rs`) drains the link's owed response cancels
itself, since the Resource packets it routes never reach `handle_data`'s
drain: the main path through `drain_owed_response_cancels` next to the
cancels of its own, and both early-return arms through
`owed_response_cancels` and `send_owed_response_cancels`
(`resource_wire_parts/cancel.rs`), which take the cancels and the events
they raised under the link guard and send them once it is gone.
`handle_resource_proof` drains them the same way and moves verbatim to
`resource_wire_parts/proof.rs`, an `include!`d part, to keep
`resource_wire.rs` under the 500-line cap. `Link::fail_pending_request`
(`link_sections/pending_requests.rs`) hands back the `Receiving` hashes
of a record no receipt owns, so the 16-byte and legacy-length `Complete`
arms cancel the same transfer, and `abandon_pending_requests` clears the
owed cancels with the rest of the link state. `response_record_tests.rs`
gains rows for the receipt-less hand-back, the over-limit resolve of a
receipt-less record and the cleared owed cancels. Both probe8 tests make
the next inbound packet a response advertisement and count the keepalive
and Response packets that reached the connector's link at zero on its
interface tap, pinning the cancel to the Resource path. `wire.rs` notes
the Resource path drains for itself. The whole-queue `drain_events` that
`owed_response_cancels` returned alongside the cancels is gone since
`10f67e30`: the cancel's event comes back from the manager, and the
queue is left for the next routed drain.

Fork commit `64ab1317`: the crate `README.md`, the crate `CHANGELOG.md`
receipt bullet and the `RequestReceipt` rustdoc
(`destination/request/receipt.rs`) carry one exception clause, word for
word: an over-limit packet Response is not posted as `LinkEvent::Data`,
and a response Resource assembling over the limit has its `Complete`
withheld. The probe6 bare `request_packet` test doc and this ledger's
`f7d8c60d` paragraph say the limit is kept when the send seam re-arms
the record's clock. The `cancel_inbound_responses` rustdoc
(`transport_sections/request.rs`) names displaced records alongside
failed receipts.

Fork commit `20019b5f`: `..._sizes_cancel2.rs` asserts the
`InboundFailed` reason is `cancelled` and the event hash is the
advertisement's original hash; `pending_requests_tests.rs` exports its
`id` helper and `response_record_tests.rs` and `response_packet_tests.rs`
import it; the `OutboundRejected` filters in `..._sizes_probe.rs`,
`..._sizes_probe2.rs` and `..._sizes_probe3.rs` admit a completion or
failure so the assert after them discriminates; the lying-advertisement
test in `..._sizes_probe5.rs` calls `sizes_probe6_readvertise_under_a_lie`
instead of its own copy of the decoy-then-lie block.

Fork commit `1dd36956`: `Link::abandon_pending_requests`
(`link_sections/pending_requests.rs`) clears the whole
`pending_response_limits` table itself, so the records no receipt owns
go with the link state where the receipts and the owed cancels already
went; the follow-up clears after its three callers
(`request_with_mtu_limit` in `link_sections/new.rs`,
`finalize_local_close` and `restart` in
`link_sections/next_watchdog_deadline.rs`) are gone.
`fail_pending_request` asserts in debug builds that a receipt and its
record never both hold the inbound response. The
`fail_all_pending_requests` rustdoc names the stopping transport as a
caller; the `fail_pending_requests_on_stop` rustdoc
(`transport/lifecycle.rs`) says the stop sweep drops what receipts were
receiving and leaves records and owed cancels with the link state.
`response_record_tests.rs` gains a row seeding a bare record, a
receiving record and an owed cancel before a fresh link request, a
restart and a close, and finds nothing of them after.

Fork commit `03f63892`: `handle_link_resource_packet`
(`transport/resource_wire.rs`) drains the link's owed response cancels
before the ResourceManager admits the packet, so a cancel owed to a
displaced assembly fails it before the same packet's advertisement is
admitted under the fresh record, and the `InboundFailed` it raises is
published with this packet's events. It was also routed through the
hash-keyed outcome step; a later fix commit below stopped that.

Fork commit `032e9142`: new fixture
`two_transport_loopback_requests_resource_form_sizes_probe9.rs` (one
`include!` line in `transport/tests.rs`) pins where the owed cancel
leaves the link: an over-limit packet response sends it on that packet,
a `send_request_resource*` re-sent under a receiving id cancels it in
the call before the request Resource is advertised and the replacing
record keeps the limit `None` leaves alone or takes the one
`Some` sets, and the cancel `expect_response` owes leaves on the link's
next inbound packet, here the responder's own receiver cancel refusing
an over-`max_request_size` request Resource. The owed-cancel bullet in
the `CHANGELOG.md` names the fixture and says the drain precedes the
packet's own admission.

Fork commit `8b512044`: the `owed_response_cancels` field doc
(`link_parts/linkhandleresult.rs`) and the `take_owed_response_cancels`
rustdoc (`link_sections/pending_requests.rs`) name receipts and records,
failed and displaced, the three producers and the drain on the link's
next inbound packet; the `OutboundRejected` filter in
`..._sizes_probe3.rs` no longer admits a completion, which an earlier
neighbour's transfer could supply first; `..._sizes_cancel.rs` and
`..._sizes_cancel2.rs` build their over-limit envelope with
`sizes_cancel_probe_oversize_envelope`; `response_packet_tests.rs`
imports `hash` from `pending_requests_tests.rs` instead of keeping a
copy; `..._sizes_probe8.rs` spells `Hash` bare; `..._sizes_probe9.rs`
and `..._sizes_cancel.rs` still qualify it.

Fork commit `9ebd348b`: `handle_link_resource_packet`
(`transport/resource_wire.rs`) collects the owed cancel's own events
right after the drain and routes only the admitted packet's: the
cancel's `InboundFailed` carries the hash of the transfer it took down,
and a retry of that very advertisement admitted by the same packet is
received under the fresh record with the same hash, so routing the old
failure through the hash-keyed `note_request_resource_outcome` cleared
the fresh record and left the admitted receiver recordless.
`note_request_resource_outcome` (`link_sections/pending_requests.rs`)
gains the `debug_assert!` that a request Resource is owned by a receipt
or a record, never both, mirroring the one in `fail_pending_request`.
New fixture `two_transport_loopback_requests_resource_form_sizes_probe10.rs`
(one `include!` line in `transport/tests.rs`) pins the retry-admitted
case: the retry is asked for parts again under the fresh record, which
keeps its limit and refuses a later over-limit advertisement with a
receiver cancel; an advertisement for another id is admitted and
assembled. `..._sizes_probe9.rs` renames its helper to
`sizes_probe9_assert_displaced_transfer_cancelled` and says that
`send_request_resource*` cancels the displaced transfer in the call,
before the request Resource is advertised, and uses it for its own
advertisements; `..._sizes_cancel.rs` adds `SIZES_CANCEL_ADMITTED_D` and
reuses `sizes_cancel_probe_receiving_receipt`.
The owed-cancel bullet in the `CHANGELOG.md` says the cancel's failure
is published and not routed and names the new fixture. The pre-admission
`drain_events` and the `admitted_from` skip this commit added are gone
since `10f67e30`: the cancel's event never enters the queue, so the
single post-admission drain is routed in full again.

Fork commit `9842695a`: `..._sizes_cancel_probe.rs`, `..._sizes_cancel2.rs`
and `..._sizes_probe8.rs` build their admitted advertisement with
`SIZES_CANCEL_ADMITTED_D`; the `..._sizes_cancel2.rs` hash assert says a
single-segment advertisement's original hash is its own;
`sizes_assert_link_still_answers` (`..._sizes.rs`) documents the ordering
barrier its round trip provides; the `expect_response` rustdoc
(`link_sections/response_packet.rs`) covers the record waiting on a
request Resource.

Fork commit `10f67e30`: `ResourceManager::cancel_incoming_response`
(`resource/manager_cleanup.rs`) returns `CancelledResponse`, whether a
receiver was in flight and the transfer's `InboundFailed("cancelled")`,
instead of pushing that event onto the manager's queue; `cancel_incoming`
is folded into the private `take_cancelled_incoming`, which builds the
event, and `fail_inbound_segments` (`resource/manager_segments.rs`)
splits the assembly removal into `take_inbound_segments`, which both
share. The cancel's event had to be published and not routed, and every
site that kept it out of the routing loop drained the whole queue
publish-only, swallowing any failure the manager had queued across a lock
release before the packet, so the receipt parked `Receiving` with no
clock; the receipt-timeouts fixture `..._failed_response_resource_fails_receive`
races exactly that. `cancel_inbound_response` and
`drain_owed_response_cancels` (`resource_wire_parts/cancel.rs`) take
an `events` sink, `owed_response_cancels` returns only the returned
cancel events and `note_resource_events_on_links` returns the ones it
raised; `handle_link_resource_packet` (`transport/resource_wire.rs`)
collects every cancel event in `cancel_events` and appends it after the
`suppressed` removal, with one post-admission drain routed in full;
`handle_resource_proof` (`resource_wire_parts/proof.rs`), `handle_data`
(`transport/wire.rs`) and `cancel_inbound_responses`
(`links_parts/transport_sections/request.rs`) publish the returned
events instead of draining the queue; the retry worker
(`transport/jobs.rs`) and `handle_check_links`
(`jobs_parts/link_maintenance.rs`) drop their second drain; the stop
sweep (`transport/lifecycle.rs`) publishes the returned events together
with the whole-queue drain it already had, since no routed drain follows
a stop. The manager unit tests
(`resource/tests_timeouts_lifecycle.rs`) read the returned struct and
assert the queue is untouched. The owed-cancel bullet in the
`CHANGELOG.md` says the publish-not-route property now holds by
construction.

Fork commit `c18098c9`: new fixtures
`two_transport_loopback_requests_resource_form_sizes_probe11.rs`,
`..._probe12.rs` and `..._probe13.rs` (three `include!` lines in
`transport/tests.rs`): probe11 runs the displace-and-retry cycle twice
on one id and widens the limit while displacing, probe12 answers a
receipt-less `Receiving` record with an in-limit packet response and
lets a real many-part response over the legacy MTU complete, probe13
makes the race of `..._failed_response_resource_fails_receive`
deterministic by dropping the connector's Resource state and sending an
unsolicited advertisement before the retry worker's drain, which failed
at `10f67e30`'s parent and passes after it. `..._sizes_cancel_probe.rs`
reads the admitted `d` from `SIZES_CANCEL_ADMITTED_D`;
`resolve_pending_request` (`link_sections/pending_requests.rs`) drops
its over-limit `debug!`, leaving the wire-level `warn!` as the one log
per over-limit assembly. The owed-cancel bullet in the `CHANGELOG.md`
names the three fixtures.

Fork commit `3568f1ab`: `ResourceManager::drain_events_for_link`
(`resource/manager.rs`) partitions the manager's queue by link and hands
back only the events queued for the given link, leaving the rest in
order. `handle_link_resource_packet` (`transport/resource_wire.rs`) and
`handle_resource_proof` (`resource_wire_parts/proof.rs`) drain with it
after the manager has admitted the packet (a refused or unsolicited
advertisement, like an undecryptable Resource packet, skips the drain and
leaves the link's queue for the periodic drain), so their routing loops
no longer skip events of another link, which a `debug_assert_eq!` on the
link id now records as impossible. Before, both drained the whole queue
and published the other links' events unrouted, so a failure queued for a
sibling link of the same connector, such as the `InboundFailed` a dropped
Resource state produces, never reached its receipt once a Resource packet
landed on the first link, and that receipt parked `Receiving`; the
sibling's events now wait for the retry worker's routed drain. The
periodic, stop and other whole-queue drains are unchanged.
`cancel_incoming_response` (`resource/manager_cleanup.rs`) writes its
receiver arm in the `.then().flatten()` shape of its assembly arm, with
no change in behaviour; `handle_check_links`
(`jobs_parts/link_maintenance.rs`) loses a stale `#[allow(dead_code)]`;
`cancel_inbound_responses` (`links_parts/transport_sections/request.rs`)
collects the owed segments with `filter_map` instead of a `filter` whose
predicate also pushed events. New manager unit tests
(`resource/tests_timeouts_lifecycle.rs`) pin the per-link drain against
two links' interleaved events and the cancel arm that finds a receiver
and an assembly together. The owed-cancel bullet in the `CHANGELOG.md`
now says only this link's queued events ride the packet's drain.

Fork commit `f1985d01`: new fixture
`two_transport_loopback_requests_resource_form_sizes_probe14.rs` (one
`include!` line in `transport/tests.rs`): a queued failure leaves on the
advertisement another receipt admits, and an owed cancel and a sibling
link's queued failure leave on one admitting advertisement, the second of
which failed at `3568f1ab`'s parent and passes after it.
`..._sizes_cancel.rs` adds `SIZES_OVER_D`, the over-limit `d` the
`..._probe8.rs`, `..._probe9.rs`, `..._probe10.rs` and `..._probe11.rs`
advertisements spelt out; those probes' tests with a listener tap close by
counting no further `ResourceReceiverCancel` (the last two such closes,
probe11's first test and probe10's second, land in `348cf74d`);
`..._probe13.rs`'s header says it biases the race rather than deciding it.
`two_transport_loopback_requests_receipt_stop.rs` pins the stop sweep
cancelling a `Receiving` receipt's inbound response once. The
`CHANGELOG.md` receipt and owed-cancel bullets name the new pins.

Fork commit `348cf74d`: `ResourceManager::drain_events_for_link`
(`resource/manager.rs`) becomes `pub(crate)`; every caller is in-crate
and the crate's public surface is as it was before `3568f1ab`. The stop
sweep (`transport/lifecycle.rs`) drains the manager queue first and
appends the cancel events it raises, the order of the retry worker and
`handle_check_links`. The comment above the owed-cancel drain in
`handle_link_resource_packet` (`transport/resource_wire.rs`) and the
owed-cancel bullet in the `CHANGELOG.md` say what the per-link drain
covers: events queued for the link before a Resource packet it admits
(or a proof it handles); a refused or unsolicited packet leaves the queue
for the periodic drain, as other links' events always do. Test changes:
`..._sizes_probe14.rs` gains a third test, a refused over-limit
advertisement on a link with a queued failure, where the receipt still
fails `Receive` on the periodic drain (it documents the contract and
passes with either drain shape, as its doc says), and its header and
second test's doc describe the two drains at work; `..._probe11.rs`'s
first test and `..._probe10.rs`'s second close by counting no further
`ResourceReceiverCancel` after the `/echo` round trip;
`..._sizes_cancel_probe.rs` sizes its over-limit envelope from
`SIZES_OVER_D`; `..._probe12.rs`'s `sizes_probe12_assert_still_inbound`
treats a lagged broadcast receiver as more events to read rather than
the end of them. The `CHANGELOG.md` receipt bullet is reflowed in two
places with no wording change.

Fork commit `a93e3127`: new fixture
`two_transport_loopback_requests_resource_form_sizes_probe15.rs` (one
`include!` line in `transport/tests.rs`): the responder's part requests
and proof for a resource-form request on one link leave a sibling link's
queued failure for the periodic drain, which fails that receipt `Receive`
while the request is proved and answered; it fails with the proof-side
drain of `3568f1ab` reverted alone, which no earlier fixture told apart.
The `CHANGELOG.md` owed-cancel bullet names it.

Fork commit `c7d5f533`: the third `..._sizes_probe14.rs` test's doc says
in plain words that a drain on the refused packet would pass the same
assertions sooner.

Fork commit `d9585ef0`: new fixture
`two_transport_loopback_requests_resource_form_sizes_probe16.rs` (one
`include!` line in `transport/tests.rs`): a receipt `Receiving` on a
sibling link of the same connector has its inbound Resource dropped, which
queues a `link_closed` failure for that link, while a receipt on the
pair's link is still `Receiving` when `Transport::stop` runs; the sweep
fails both receipts `LinkClosed` (the queued failure is published, not
routed) and on `resource_events()` the queued failure precedes the
`cancelled` failure the sweep raises, each reported once, with no cancel
for the receiver already gone. It passes at `348cf74d` and fails with the
sweep order before it, which no earlier fixture told apart. The
`CHANGELOG.md` receipt bullet says the sweep publishes the manager queue
first, in the order of the periodic drains and routed to no receipt, and
names the fixture.

Fork commit `6499621f`: F2, link proof event and receipt link id. Upstream
`Link::handle_proof_packet`
(`src/destination/link_parts/link_sections/handle_proof_packet.rs`)
validates a per-packet proof on an active link, resolves the pending
Channel frame it may answer and returns `LinkHandleResult::None` without
posting anything; the fork posts `LinkEvent::PacketProven { packet_hash }`
there when the proved hash answered no pending Channel frame (the gate
`875edf92` replaces that test with the sent-hash record). Of a link's Data
packets the peer proves the plain `PacketContext::None` ones
(`RNS/Link.py:950-962`, the crate's gate in `new.rs`) and Channel frames
(`:1147-1151`); a request, a response or a Resource packet is never proved
and so never produces the event. `LinkEvent` (`linkhandleresult.rs`) becomes
`#[non_exhaustive]`, a breaking change that makes the next release
`0.15.0`; the exhaustive name map in `Link::post_event`
(`next_watchdog_deadline.rs`) gains the `packet_proven` arm and the one in
`crates/apps/rns-tools/src/bin/independent_interop_node/events.rs` gains it
and, being in another crate, a wildcard arm. `DeliveryReceipt`
(`src/transport/mod.rs`) gains a private `link_id: Option<AddressHash>`
with a `link_id()` getter and a crate-private `for_proof(hash, &packet)`
constructor that records `packet.destination` when the proof is addressed
to a Link; `transport/wire.rs::handle_proof` and
`Transport::handle_inbound_for_test` (`transport/core.rs`) build the
receipt through it, `DeliveryReceipt::new` keeps `link_id` as `None`. New
fixture `two_transport_loopback_link_packet_proven.rs` (one `include!`
line in `transport/tests.rs`): a plain Data packet sent on the pair's link
yields exactly one `PacketProven` with its hash on the sender's link stream
and one `DeliveryReceipt` naming the link at the receipt handler, a packet
request and a request answered by a Resource on the same link yield none,
the responder's in-link posts the event for a plain Data packet it sends,
and `DeliveryReceipt::new` has no link id. The crate `CHANGELOG.md` adds
the event and the getter, notes the breaking change, and the crate
`README.md` gains a `Delivery proofs on a link` section.

Fork commit `d4127c6d`: the four `RequestReceipt` status orders pinned
exactly. New `destination/request/receipt_order_tests.rs` (declared in
`destination/request/mod.rs`; `receipt_tests.rs` lends it `hash`, `id` and
`response_segment` as `pub(super)`) drives `PendingRequest` and reads the
status watch after every transition; `response_record_tests.rs` pins that
`note_request_resource_outcome` never marks a packet-form entry
`Delivered`; new fixture
`two_transport_loopback_requests_receipt_orders.rs` (one `include!` line
in `transport/tests.rs`) pins the same four orders end to end with a
responder that waits before answering, so no change is coalesced on the
watch, while `..._resource_form_sizes.rs` keeps its subsequence tests. The
`CHANGELOG.md` receipt bullet names the three files.

Fork commit `04de5d71`: the packet-proven fixtures. In
`two_transport_loopback_link_packet_proven.rs` the `/big` handler answers
with an `over_mdu_body`, so the response the test calls a Resource travels
as one (a 64 KiB body fits a single packet at the default loopback MTU);
its header names the sent-hash record of `875edf92`. New fixture
`two_transport_loopback_link_packet_proven_probe.rs` (one `include!` line
in `transport/tests.rs`): three plain Data packets in a burst earn exactly
their three `PacketProven` hashes and three receipts naming the link; a
plain Resource and a resource-form request on the link earn none on either
side; a single-destination proof reaches the `ReceiptHandler` with
`link_id()` of `None`. New `tests/link_event_packet_proven_api.rs`: the
downstream match shape with the wildcard arm, under
`#[deny(unreachable_patterns)]` so the arm pins `#[non_exhaustive]`, and
the public `DeliveryReceipt::new` naming no link.
`two_transport_loopback_requests_receipt_orders.rs` reuses `HANDLER_DELAY`
from `two_transport_loopback_requests_receipt.rs` in place of a twin
constant and names both subsequence siblings; `response_segment` in
`destination/request/receipt_tests.rs` says its `q` is fixed to `id(1)`.

Fork commit `875edf92`: `PacketProven` only for a plain Data packet this
side sent. `Link` (`linkhandleresult.rs`) gains `unproven_data_hashes`, a
`VecDeque<Hash>` capped at `UNPROVEN_DATA_HASHES_CAP = 1024`
(`link_parts/module_prelude.rs`), initialised in both constructors
(`new.rs`, `new_from_request.rs`); `TransportHandler::note_link_packet_sent`
(`transport/handler.rs`) records every `PacketType::Data` packet with
`PacketContext::None` through the new `Link::note_data_sent`
(`handle_proof_packet.rs`), and `handle_proof_packet` posts the event only
when `take_unproven_data_hash` consumes the proved hash, independently of
the pending Channel frame it may resolve, as Python's proof removes the
one `SENT` receipt it validates (`RNS/Transport.py:2748-2761`); the FIFO
cap stands in for Python's receipt timeout (`RNS/Packet.py:428`). New
`link_sections/data_proof_tests.rs` (one `include!` line in `link.rs`):
proven once and not again on a replayed proof, nothing for an unrecorded
hash, nothing for a Channel frame whose state still goes `Delivered`, the
oldest record dropped past the cap. The variant rustdoc, the crate
`CHANGELOG.md` bullets and the crate `README.md` section describe the
contract, the Python peer's `PROVE_NONE` default (`RNS/Link.py:961-968`,
`RNS/Destination.py:161`), the `ReceiptHandler` twin and the `PartialEq`
change; the `CHANGELOG.md` names the four pinning files.

Fork commit `8c262c9f`: new fixture
`two_transport_loopback_link_packet_proven_probe2.rs` (one `include!` line
in `transport/tests.rs`): the sent-hash record driven through the
transport send path, a burst of `UNPROVEN_DATA_HASHES_CAP` plus three plain
Data packets while the peer is stalled earning exactly the cap in
`PacketProven` events, for the newest hashes, while every proof still
reaches the `ReceiptHandler` naming the link, and a Channel frame sent
through `Transport::channel` proved, marked `Delivered` and reaching the
`ReceiptHandler` without a `PacketProven`. The `PacketProven` variant and
`unproven_data_hashes` rustdoc, the `DeliveryReceipt::link_id` rustdoc, the
crate `CHANGELOG.md` bullet and the crate `README.md` section between them
(no one site carries all three) name the 1024 bound as Python's
`Transport.MAX_RECEIPTS` applied per link (`RNS/Transport.py:159`,
`:745-748`), cite the record side (`:1359-1377`) with this side's narrowing
to plain Data, and say the `DeliveryReceipt` is handed out for every
validly signed link proof, not filtered by the record; the `CHANGELOG.md`
names the fixture.

Fork commit `631203b5`: new fixture
`two_transport_loopback_link_packet_proven_probe3.rs` (one `include!` line
in `transport/tests.rs`): the peer's proof of a plain Data packet captured
off the connector's interface tap and fed back in; byte-identical, it is
refused by the ingress packet filter and adds neither a `DeliveryReceipt`
nor a `PacketProven`; delivered past the filter into inbound processing, it
is validated again and handed out as a second `DeliveryReceipt` naming the
link, the consumed record posts no event, and a fresh plain Data packet
afterwards earns exactly one. The crate `CHANGELOG.md` bullet and the crate
`README.md` section qualify the replayed-proof clause to a replay the
ingress filter lets through (one the bounded duplicate cache has
forgotten), split Python's recorded receipts into request and response
packets, which expire unproved, and Channel frames, which the peer proves,
and the `README.md` names the Channel frame's proof in place of "those";
the `PacketProven` rustdoc (`linkhandleresult.rs`) states the 1024 bound as
a number, `UNPROVEN_DATA_HASHES_CAP` being `pub(crate)`; the record-site
comment in `transport/handler.rs` cites `RNS/Transport.py:1359-1377`
beside `:2748-2761`. In `two_transport_loopback_link_packet_proven_probe.rs`
the Resource test asserts an empty receipt list (its `all()` over an empty
`Vec` held vacuously) and its docstring says why none arrives;
`..._probe2.rs` notes the link event channel capacity equals the cap;
`response_record_tests.rs` says it hosts the receipt-entry rows its sibling
has no room for; the rns-tools interop node `events.rs` logs an unnamed
link event at debug before reporting it as `unknown`. The `CHANGELOG.md`
names the fixture.

Fork commit `50793515`: F3, outbound outcomes with link fast fail and
accept hook. The ResourceManager hands out a verdict for an outbound Resource.
New `src/resource/outcome.rs` (an `include!` line in `resource.rs`)
carries `RejectReason` (`PeerCancelled`; `Oversize { limit }` and
`Unknown` reserved, since a refusal arrives as a `RESOURCE_RCL` carrying
only the resource hash, `RNS/Resource.py:155-165`), `OutboundFailure`
(`NotSent`, `RetriesExhausted`, `Segmenting`, `LinkClosed`),
`OutboundResult` (`Complete`, `Failed`, `Rejected`, `Cancelled`), all three
`#[non_exhaustive]`, the oneshot-backed `OutboundOutcome` future, which
resolves `Failed(LinkClosed)` when the manager is dropped before a verdict,
`ResourceManager::track_prepared_with_outcome`, keyed by the original hash
so a split resource resolves once, and the crate-private
`conclude_outbound`, the one exit that queues the terminal `ResourceEvent`
and resolves the outcome registered for it.
Upstream sites: `resource.rs` makes `ResourceEventKind::OutboundRejected` a
struct variant carrying `reason: RejectReason`; `resource/manager.rs` adds
the `outbound_outcomes` table and routes the local cancel and the receiver
cancel through `conclude_outbound`, `manager_start.rs` initialises the
table, `manager_cleanup.rs` concludes `Failed(LinkClosed)` once per
original hash when a link's state is removed, `manager_proof.rs` concludes
`Complete` on the final proof and `Failed(Segmenting)` when a later segment
cannot be built, `manager_segments.rs` concludes `Failed(NotSent)` when
`confirm_outbound_dispatch` learns the advertisement was not sent, and
`manager_polling.rs` maps the new `OutboundResourcePoll::RetriesExhausted`,
which `resource/sender.rs` returns in place of `Failed` on an exhausted
retry budget, to `Failed(RetriesExhausted)` and the failed state to
`Failed(Segmenting)`. Every `OutboundRejected` match in the workspace takes
the `{ .. }` or `{ reason }` form: the `rncp` client and server, the
independent interop node's `events.rs`, reticulumd's
`bridge_remote_control_link.rs`, inbound worker `module_core.rs` and the
`python_channel_events.rs` test support, lxmf-runtime's `link_delivery.rs`
and `tests.rs`, the link's `pending_requests.rs` with its two test files,
and the resource and transport test fixtures. New `resource/tests_outcome.rs`
(an `include!` line in `resource/tests.rs`) pins the not-sent, link-closed,
receiver-cancel, local-cancel, manager-dropped and retries-exhausted
verdicts. The release notes for this commit are in fork commit `9eaafcc1`,
below.

Fork commit `4a4a91ae`: `Transport::{send_resource_with_outcome,
send_request_resource_with_outcome, send_response_resource_with_outcome}`,
the existing `send_*` entry points returning `(Hash, OutboundOutcome)`.
Upstream sites: `transport_sections/send_outcome.rs` (new, an `include!`
line in `links_parts/transport.rs`) carries the three public methods and
their rustdoc: the verdict mapping, a request Resource refused under
`max_request_size` (`RNS/Link.py:1036-1042`) and a response Resource
refused under `max_response_size` (`RNS/Link.py:1044-1051`) both reading
`Rejected(PeerCancelled)`, a response advertisement nobody expects dropped
without an answer (`RNS/Link.py:1046-1047`) ending in
`Failed(RetriesExhausted)`, and dropping the outcome cancelling nothing.
`transport_sections/reset_out_link.rs` moves the bodies of
`send_resource_observed_with_compression` and
`send_request_resource_with_max_response_size` into the crate-private
`send_resource_outcome_inner` and `send_request_resource_outcome_inner`,
which call `track_prepared_with_outcome` under the handler lock and return
the outcome beside the hash; the public forms map it away.
`transport/request_dispatch.rs` has the free `send_response_resource`
return the pair and `send_response_as_resource` discard the outcome. New
fixture `two_transport_loopback_resources_outcome.rs` (one `include!` line
in `transport/tests.rs`) pins a three-segment send resolving `Complete`
only once the final segment is proven, a torn-down link resolving
`Failed(LinkClosed)`, a receiver cap drawing `Rejected(PeerCancelled)`, and
`cancel_resource` resolving `Cancelled`. The release notes for this commit
are in fork commit `9eaafcc1`, below.

Fork commit `3a5cd726`: `Transport::link` returns
`Result<Arc<Mutex<Link>>, RnsError>` and fails fast with the new
`RnsError::NoPath` when the path table has no next hop for the destination
and the transport does not broadcast; the request used to be dropped as
`DroppedNoRoute` on the way out and the caller learned of it only when the
pending link outlived its establishment timeout, 6 s + `PATHFINDER_M` (128)
hops x 6 s = 774 s for a destination without a path entry (this commit's
message and notes said `DEFAULT_ESTABLISHMENT_TIMEOUT`, 12 s; fork commit
`873ef34f` corrects them). With `broadcast` on the request still goes out
on every outgoing interface, as `RNS/Transport.py:1443` floods a packet
without a path entry.
Upstream sites: `link()` moves out of `transport_sections/close_channel.rs`
into the new `transport_sections/link_open.rs` (an `include!` line in
`links_parts/transport.rs`), which reads the next hop, the hop count and
the `broadcast` flag under one handler guard and returns `Err(NoPath)`
before any link is built or inserted; `error.rs` adds the variant, citing
`RNS/Transport.py:1383` for the path-table route and `:1443` for the
flood, and makes `RnsError` `#[non_exhaustive]`. `delivery::send_via_link`
(`delivery.rs`) and lxmf-runtime's `activate_link`
(`lxmf-runtime/src/link_delivery.rs`) surface the failure at once instead
of waiting out their activation timeout; reticulumd's
`propagation_link_for_node` (`bridge_propagation.rs`) returns an
`io::Result` so the delivery task
(`resolve_or_create_propagation_link.rs`) can report it, and
`bridge_link_send.rs` records the failed outbound activity before
returning. The remaining call sites in reticulumd, rns-tools (`rncp`,
`rngit`, `rnsh`, the independent interop node) and the crate's own tests
and fixtures unwrap or map the `Result`. Tests: new fixture
`link_no_path_fast_fail.rs` (one `include!` line in `transport/tests.rs`)
pins the refusal without broadcast and the request still leaving with it;
`delivery.rs` gains
`send_via_link_without_path_fails_before_the_activation_wait` and
lxmf-runtime `tests.rs`
`link_delivery_without_path_fails_before_the_first_activation_wait`, each
bounding the failure under a second;
`two_transport_loopback_requests_errors.rs` seeds a path for its unserved
destination over the served one's interface, since its connector does not
broadcast. The crate `CHANGELOG.md` carries the `### Changed` bullet.

Fork commit `a0a88883`: `AnnounceEvent::is_path_response`. Upstream sites:
`AnnounceEvent` (`transport/mod.rs`) gains the public `bool` field, true
for an announce the destination sent in answer to a path request, which it
files under `PacketContext::PathResponse` (`RNS/Destination.py:308`), the
flag Python hands its announce handlers (`RNS/Transport.py:2524-2528`);
`transport/announce.rs` sets it from the packet context where the event is
published. New fixture `two_transport_loopback_announce_path_response.rs`
(one `include!` line in `transport/tests.rs`) pins a fresh announce
reading false and the announce drawn out by `request_path` for a
destination registered without announcing reading true. The crate
`CHANGELOG.md` carries the `### Added` bullet.

Fork commit `9eaafcc1`: `Transport::set_resource_accept_hook`, the
`ACCEPT_APP` callback of `RNS/Link.py:1066-1074`. Upstream sites: new
`transport/resource_accept_hook.rs` (declared in `transport/mod.rs`, which
re-exports the alias) carries `ResourceAcceptHook`, an
`Arc<dyn Fn(&ResourceAdvertisement, &LinkId, Option<&Identity>) -> bool +
Send + Sync>`, and `set_resource_accept_hook`, and takes over
`set_receipt_handler` and `emit_receipt_for_test` from `transport/core.rs`
unchanged; `TransportHandler` (`transport/mod.rs`) gains the
`resource_accept_hook` seat, initialised to `None` in `core.rs`.
`handle_link_resource_packet` in `transport/resource_wire.rs` consults the
hook, under the link guard, for an advertisement that is neither a request
nor a response and that the size caps did not already refuse, with the
link id and `Link::identified_peer_identity`; `false` logs at debug and
queues the same receiver cancel the caps send (`RNS/Resource.py:155-165`),
so the sender reads `Rejected(PeerCancelled)`. New fixture
`two_transport_loopback_resources_accept_hook.rs` (one `include!` line in
`transport/tests.rs`) pins a refusing hook drawing `Rejected(PeerCancelled)`
on the sender's outcome and event stream with the link it saw, nothing
assembled and the link surviving, a replacement hook returning `true`
admitting the next transfer, and a request advertisement completing under
`max_request_size` (`RNS/Link.py:1036-1042`) without the hook being
consulted. The crate `CHANGELOG.md` gains the `### Added` bullets for the
`send_*_with_outcome` family and the hook, and the `### Changed` bullet
for `OutboundRejected { reason }` as a breaking change whose out-of-tree
patterns need `{ .. }` or `{ reason }`.

Fork commit `98718b4d`: the crate `README.md` gains an `Outbound resource
outcomes` section, with a `send_resource_with_outcome` snippet whose match
keeps the wildcard arm the `#[non_exhaustive]` enums call for and the
verdict mapping as the rustdoc of `resource/outcome.rs` and
`transport_sections/send_outcome.rs` states it, and an `Opening links and
admitting resources` section on `Transport::link` failing fast with
`RnsError::NoPath` (`RNS/Transport.py:1439-1443`), `RnsError` being
`#[non_exhaustive]`, `AnnounceEvent::is_path_response` and the scope of
`Transport::set_resource_accept_hook` (`RNS/Link.py:1036-1064` against
`:1065-1074`), both between `Delivery proofs on a link` and `Versioning
and releases`. The `Request handlers` and `Sending requests` snippets,
which `tests/request_receipt_readme.rs` drives verbatim, are untouched.
Fork commit `97bd6a91` spells out the alias's `Send + Sync` bounds there.

Fork commit `85c20ef5`: `Transport::stop()` concludes every outbound
transfer and a panicking accept hook is isolated. Upstream sites:
`transport/lifecycle.rs`, whose stop sweep failed the pending requests and
cancelled the responses they were receiving but left every
`OutboundOutcome` pending, now also calls `remove_link_state` for every
link it could lock, after the receipt cancels so a cancelled response keeps
its reason, and publishes the events that queues, so every outbound
transfer on a stopped transport concludes `Failed(LinkClosed)`. The hook
call moves out of `transport/resource_wire.rs` into `consult_accept_hook`
in the new `transport/resource_wire_parts/accept_hook.rs` (an `include!`
line in `resource_wire.rs`, keeping that file under the size limit),
wrapped in `catch_unwind` as `channel.rs` wraps channel handlers: a panic
is logged at error and the advertisement dropped unanswered, as
`RNS/Link.py:1073-1074` logs the callback's exception and neither accepts
nor rejects; the `ResourceAcceptHook` rustdoc in
`transport/resource_accept_hook.rs` covers the panic path, once-per-segment
consultation and the `ACCEPT_ALL` / `ACCEPT_NONE` parity.
`resource/manager_segments.rs` has `confirm_outbound_dispatch(.., false)`
conclude under the sender's original hash, the key of the outcome table;
`resource/manager.rs` extends the test-only `has_no_outbound_state` to
empty segment chains and an empty outcome table. Tests: new fixture
`two_transport_loopback_resources_outcome_stop.rs` (one `include!` line in
`transport/tests.rs`) stalls the listener, sends a two-segment resource,
stops the connector and awaits `Failed(LinkClosed)`, which times out
without the lifecycle change;
`two_transport_loopback_resources_accept_hook.rs` gains
`accept_hook_panic_drops_the_advertisement_and_the_transport_survives`;
`resource/tests_outcome.rs` asserts `has_no_outbound_state` after each
verdict and pins `Failed(Segmenting)` at the variant level.
`#[deny(clippy::await_holding_invalid_type)]` lands on `Transport::link`
(`link_open.rs`), the `delivery.rs` async tests and every async fn in
`tests_outcome.rs`.

Fork commit `1bd387b4`: lxmf-runtime's `activate_link`
(`lxmf-runtime/src/link_delivery.rs`) propagated `RnsError::NoPath` with
`?` on every attempt, so a retry that found no path, after the pending
link's expiry removed it (`jobs_parts/link_maintenance.rs`), ended the
whole retry budget and discarded the activation error before it. The
first attempt still fails fast without a path; a later attempt that finds
none folds the previous activation error into `last_error`, spends the
attempt in `Transport::await_path` and continues, and other `RnsError`
variants keep failing fast. Test: lxmf-runtime `tests.rs`
`link_delivery_waits_for_a_path_that_vanished_between_attempts` seeds a
path by announce, expires it by hand once the first `LinkRequest` has
left and bounds its wait for the retry's path request; it fails on the
previous code with `unexpected message: link request not sent: no path to
destination`. The crate `CHANGELOG.md` extends the `Transport::link`
bullet.

Fork commit `5df575ec`: usage fixtures for the outcome and hook surface.
New `two_transport_loopback_resources_outcome_probe.rs` (one `include!`
line in `transport/tests.rs`, eight tests) pins a dropped outcome leaving
the transfer running to `OutboundComplete`, a response Resource resolving
`Complete` under the requester's `max_response_size`,
`Rejected(PeerCancelled)` over it and `Failed(RetriesExhausted)` when no
request solicited it, a stopped and dropped sender resolving
`Failed(LinkClosed)`, and the hook seeing `None` before the peer
identifies and `Some(identity)` after, consulted once for a split resource
it refuses (one receiver cancel, one `Rejected`) and never for a response
advertisement. New `tests/outbound_outcome_api.rs` (six tests) uses the
surface from another crate: the `#[non_exhaustive]` enums and `RnsError`
need a wildcard arm, the README snippet compiles as written, an unknown
link id is `InvalidArgument` on all three `_with_outcome` entry points,
the plain `send_*` signatures are unchanged, `link()` fails fast and
repeatably without a path and is pending with `broadcast` on, and the
`ResourceAcceptHook` alias installs. The crate `CHANGELOG.md` pin clauses
name both files, the stop fixture and the hook panic-isolation test, and
state `stop()`'s `Failed(LinkClosed)` verdict and the hook's
once-per-segment and panic behaviour.

Fork commit `873ef34f`: doc corrections in the `link()` rustdoc
(`transport_sections/link_open.rs`), the crate `README.md` and
`CHANGELOG.md`: the no-path wait was the path-sized establishment timeout
(6 s + 128 x 6 s = 774 s), not 12 s; `NoPath` sends neither a link request
nor a path request, discovery first being `request_path` plus
`await_path`; `Failed(NotSent)` never reaches a caller of
`send_*_with_outcome`, which return `Err(ConnectionError)` instead;
`is_path_response` repeats the peer-set packet context and is not matched
against a sent path request; an unknown link id is `Err(InvalidArgument)`;
and the `OutboundRejected { reason }` enumeration of adapted matches lists
every crate fork commit `50793515` touched.

Fork commit `211d84b8`: lxmf-runtime `src/tests.rs` had reached 541 lines,
over the 500-line budget `check-module-size.sh` applies to it. The two
link delivery tests move verbatim into the new `src/tests_link_delivery.rs`,
`include!`d at the end of `tests.rs`.

Fork commit `9a4e8735`: `ResourceEventKind` (`resource.rs`) and
`AnnounceEvent` (`transport/mod.rs`) are `#[non_exhaustive]`; the enum's
rustdoc states the attribute in place of the earlier catch-all advice, and
the `is_path_response` rustdoc adds that Python drops a path response
before the handler unless it sets `receive_path_responses`
(`RNS/Transport.py:2500-2504`) while every announce reaches
`Transport::recv_announces()` here, the flag being peer-asserted. The three
out-of-crate exhaustive matches on the enum gain a wildcard arm:
`reticulumd` `bridge_remote_control_link.rs` and
`inbound_worker_parts/module_core.rs` log the unknown kind at debug under
their `[daemon-control]` and `[daemon-rx]` prefixes, `rns-tools`
`independent_interop_node/events.rs` records `{"state": "unknown"}` as it
does for `LinkEvent`. `reticulumd` `bridge_remote_control_link.rs`
`open_refreshed_remote_link` returned a first `Transport::link`
`Err(NoPath)` straight away, past its own refresh branch; it now takes the
branch a timed-out activation takes (`request_path`, the one second grace,
`await_path`, one more `link`), through two helpers, `refresh_remote_path`
and `open_remote_link_once`, and the retry's error names the first error
either way; a second `NoPath` is final. The `ACCEPT_APP` cite is
`RNS/Link.py:1066-1074` in `resource_wire.rs`, `resource_accept_hook.rs`
and the two `tests_parts` fixtures that quoted it (`:1065` is the
`ACCEPT_NONE` line); the `Segmenting` rustdoc (`resource/outcome.rs`) says
the sender-failed path is open to single-segment sends; the
`link_no_path_fast_fail.rs` wall-clock bound is 500 ms (fork commit
`01e93fc3` below mirrors it in `tests/outbound_outcome_api.rs`, which had
kept 10 ms).

Fork commit `a4cf4f4b`: new fixture
`two_transport_loopback_resources_outcome_probe2.rs` (four tests, one
`include!` line in `transport/tests.rs`) covers a hook applying a size
policy read off the advertisement sorting two sends in flight on one link
into `Complete` and `Rejected(PeerCancelled)`, two split Resources in
flight each resolving `Complete` under their own hash, `stop()` concluding
every transfer on the links it locks, and a sender dropped without `stop()`
resolving `Failed(LinkClosed)`. `tests/outbound_outcome_api.rs` gains the
`ResourceEventKind` wildcard-arm pin and describes its send snippet as the
README's shape; `bridge_remote_control_link_tests.rs` pins
`open_refreshed_remote_link` without a path (one path request leaves the
interface, the error names the missing path and the refresh, the call
returns inside the timeout plus two seconds); `resource/tests_outcome.rs`
pins `Failed(Segmenting)` from a reader failing while the next segment is
built on the first segment's proof; lxmf-runtime `tests_link_delivery.rs`
also checks that nothing left the interface. Every async fn carries
`#[deny(clippy::await_holding_invalid_type)]`.

Fork commit `65fb8646`: `transport/lifecycle.rs` rustdoc on `stop()` and
`fail_pending_requests_on_stop` and the two `[tp]` warn lines say outbound
outcomes conclude on the links the sweep locks within the budget and
resolve on drop for a skipped link; the crate `CHANGELOG.md` and
`README.md` say the same, the `CHANGELOG.md` `Changed` section gains one
bullet per `#[non_exhaustive]` type, the `is_path_response` entries carry
the `receive_path_responses` parity sentence, the README accept-hook
section mirrors the panic path and once-per-segment consultation, the
outcome pin list names the new fixture, and both documents cite
`RNS/Link.py:1066-1074`.

Fork commit `01e93fc3`: new fixture
`two_transport_loopback_resources_outcome_probe3.rs` (three tests, one
`include!` line in `transport/tests.rs`) covers an accepting hook
consulted once per segment advertisement of a three-segment Resource,
seeing the sender's hash as `original_hash` on each; a panicking hook
that is never replaced resolving `Failed(RetriesExhausted)` on the sender
with nothing assembled, the retries never re-running the hook because the
sender re-sends the first advertisement packet and the receiver's
duplicate filter drops it before admission, where Python builds a fresh
packet per retry (`RNS/Resource.py:597-598`) and so re-runs its callback,
a divergence the fixture's doc comment records; and `stop()` skipping a
link whose lock it cannot take within the budget, that transfer's outcome
resolving `Failed(LinkClosed)` only when the transport is dropped. Every
async fn carries `#[deny(clippy::await_holding_invalid_type)]`.
`tests/outbound_outcome_api.rs` bounds its no-path fast-fail at 500 ms,
mirroring `link_no_path_fast_fail.rs`; `resource/tests_outcome.rs`'s
header says one test per failure path plus the `Complete`, `Rejected`
and `Cancelled` verdicts.

Fork commit `17348ca6`: the `AnnounceEvent::is_path_response` rustdoc
(`transport/mod.rs`), the crate `CHANGELOG.md` and `README.md` name the
accessor every announce reaches as `Transport::recv_announces()` (the
rustdoc as an intra-doc link); the name the three had used belonged to
nothing in the crate. The `CHANGELOG.md` `Transport::link` pin clause
adds `tests/outbound_outcome_api.rs`
(`link_without_a_path_fails_fast_and_repeatably_when_broadcast_is_off`)
and the reticulumd
`open_refreshed_remote_link_without_a_path_refreshes_once_and_fails`, and
states the 500 ms in-crate bound against `delivery.rs`'s 1 s; the outcome
bullet says `PeerCancelled` follows a receiver cancel of the advertisement
or a later segment, calls the dropped-sender pin the only loopback pin
through the drop path alone, and gains the `..._outcome_probe3.rs` pin
clause. `transport/lifecycle.rs`'s `stop()` rustdoc says the per-link
sweep runs provided the handler itself can be re-locked within the budget,
and the handler-locked warn line names the outbound outcome sweep beside
the receipt sweep.

Fork commit `60e74ffb`: Python-in-the-loop request and proof interop. The
test support gains the `request-forms` payload kind and the knobs the
request and proof fixtures need. `tests/support/python_channel_endpoint.py`
adds a module-level `request_forms_body(size)` (`bytes((i % 251) for i in
range(size))`), a `prove_all` flag on `ChannelEndpoint` (the `--prove-all`
argument) that sets the destination's proof strategy to
`RNS.Destination.PROVE_ALL`, and the `request-forms` kind on both sides.
The server registers it on `/test/request` under `ALLOW_ALL`; its handler
writes one stdout JSON line (`request_received`, the `request_id` as hex,
`path`, `response_kind`, `response_size`, `response_sha256`) and returns
`None` for `{"response": "none"}`, `request_forms_body(n)` for
`{"response_size": n}` and `reply:<data>` otherwise. The client takes
`--request-size` (a run of `x` of that length in place of the message
data) and `--identify` (identify on the link, print `identified`, then
hold the request until a plain Data packet `go` arrives), prints
`request_sent` with the `request_id`, and exits 0 after every request
outcome with one terminal line: `response_size` plus `response_sha256`,
`request_failed`, or `request_no_response`; the last exists because a
packet-form request the peer never answers never fires `failed_callback`:
`RNS/Link.py:1398` needs `DELIVERED`, which only the Resource form sets
(`:1372`).
`tests/support/python_channel_process.rs` adds `extra_args` to
`spawn_python_endpoint` and `PythonChannelClientConfig`, and the
`spawn_endpoint_with_args` and `spawn_channel_client_with_args` helpers,
the latter taking the client timeout as well; every existing caller passes
an empty slice.

Fork commit `6e583b50`: new fixture
`python_channel_interop_parts/python_to_rust_request_handler.rs` (five
ignored tests, one `include!` line in `python_channel_interop.rs`) has the
Python RNS 1.5.2 client call `Link.request` against a handler registered
through `SingleInputDestination::register_request_handler` on a Rust
`Transport` built with broadcast off. Under `RequestAllow::All`:
`python_to_rust_request_handler_allow_all_answers_a_packet_request`
(`hello-rust` answered `reply:hello-rust`),
`..._answers_a_resource_request_with_a_resource_response` (a 4096-byte
request answered by a 4102-byte `reply:`) and
`..._answers_a_packet_request_with_a_resource_response` (an 8192-byte
`RequestResponse::Bytes` body); the names of the last two claimed the
Resource form, but both payloads sit below the 16 KiB link MDU the TCP
link to Python negotiates, so each ran packet for packet, and `3f615a3b`
removed them. Under `RequestAllow::List`:
`..._allow_list_answers_the_identified_peer` builds the list from the
identity `LinkEvent::PeerIdentified` delivers and releases the client with
a plain `go` Data packet;
`..._allow_list_denies_an_unlisted_peer_silently` lists a different
identity, asserts the handler never ran and reads `request_no_response`
from the client. Every answered test compares `InboundRequest::request_id()`
as hex with the Python `RequestReceipt.request_id`, the truncated hash of
the request packet for the packet form (`RNS/Link.py:1332`) and of the
packed request for the Resource form (`:504`), and checks the response
size and SHA-256 the client reports. Every async fn but the lock-taking
test fns carries `#[deny(clippy::await_holding_invalid_type)]`.

Fork commit `ea6edfaa`: new fixture
`python_channel_interop_parts/rust_to_python_request_receipt.rs` (six
ignored tests, one `include!` line in `python_channel_interop.rs`) drives
`Transport::request` from a Rust `Transport` built with broadcast off,
connected by `TcpClient` to a Python `request-forms` server whose stdout is
drained line by line after the ready line so RNS logging cannot stall it.
`rust_to_python_request_receipt_packet_roundtrip_matches_request_id`
(`hello-python` answered `reply:hello-python`, 18 bytes),
`..._resource_request_gets_a_resource_response` (a 4096-byte request
answered by a 4102-byte `reply:`) and
`..._packet_request_gets_a_resource_response` (an 8192-byte binary body,
its SHA-256 compared with the server's report) conclude `Ready`, the last
two in packet form despite their names, their payloads sitting below the
16 KiB link MDU, which is why `3f615a3b` removed them;
`..._none_response_fails_with_timeout` has the Python handler return
`None`, so RNS sends nothing (`RNS/Link.py:844`), and the receipt, under a
3 s `RequestOptions::with_timeout`, concludes `Err(RequestError::Timeout)`
with status `Failed`. Each compares `RequestReceipt::request_id()` as hex
with the `request_id` the Python handler saw. Two proof tests send a plain
Data packet through `send_link_packet_on_bound_iface` to a `link-data`
server:
`rust_to_python_plain_data_packet_is_proven_by_a_prove_all_destination`
(started with `--prove-all`) waits for `LinkEvent::PacketProven` carrying
the packet's hash on the link, and
`..._is_not_proven_by_a_default_destination` reads link events until 3 s
pass without one and asserts none was such a proof, the stock `PROVE_NONE`
destination proving nothing (`RNS/Destination.py:161`,
`RNS/Link.py:961-968`). Every async fn but the lock-taking test fns
carries `#[deny(clippy::await_holding_invalid_type)]`.

Fork commit `9223ef43`: `tests/support/python_channel_events.rs`'s
`wait_for_outbound_resource_rejected` returns the `RejectReason` of the
`OutboundRejected { reason }` event it waits for, and
`python_channel_interop_parts/resource_faults.rs`'s
`rust_sender_maps_pinned_python_receiver_cancel_to_rejection` asserts it is
`RejectReason::PeerCancelled`, so a Python receiver cancel is pinned to the
typed reason rather than to the variant alone.

Fork commit `6e9eedf2`: the `request-forms` endpoint logic moves out of
`tests/support/python_channel_endpoint.py` (1201 to 1096 lines) into the
new sibling module `tests/support/python_channel_request_forms.py` (143
lines): `request_forms_body`, an `emit` helper that writes each JSON
report in one stdout write so RNS log lines from the transport thread
cannot interleave with it, `serve_request` and `run_client`; the
endpoint's `_on_request` and `ChannelClient.run` each dispatch to the
module in one line. The client gains `--request-timeout` (a float,
threaded to `Link.request(timeout=...)`), re-anchors its request wait
deadline right before `Link.request` so link setup and the `--identify`
go-wait no longer eat into the window a silently dropped request gets
(the go-wait keeps the overall deadline), and reports the wire form it
used, `"request_form": "resource"` or `"packet"` from
`receipt.resource is not None` (`RNS/Link.py:1327-1337`), on the
`request_sent` line and on every terminal line.

Fork commit `3f615a3b`: the request wire-form matrix. The TCP link to
Python negotiates a 16384-byte link MTU:
`RNS/Interfaces/TCPInterface.py:76-78` sets `BITRATE_GUESS` to 10 Mbps
with `AUTOCONFIGURE_MTU = True`, `RNS/Interfaces/Interface.py:257` turns
that into an HW_MTU of 16384, the Rust side signals `min(interface MTU,
262144)` and adopts the confirmed value, and the resulting
`Link::link_mdu()` measures 16303. RNS picks the wire form by
`len(packed) <= self.mdu` (`RNS/Link.py:493` for a request,
`:849` for a response), so the 4096-, 4102- and 8192-byte payloads of the
four tests `6e583b50` and `ea6edfaa` named for the Resource form travelled
as single packets. This commit deletes those four
(`python_to_rust_request_handler_answers_a_resource_request_with_a_resource_response`
and `..._answers_a_packet_request_with_a_resource_response` with their
`request_handler_bytes_body` and `request_handler_bytes` helpers from
`python_to_rust_request_handler.rs`, 323 to 283 lines;
`rust_to_python_request_receipt_resource_request_gets_a_resource_response`
and `..._packet_request_gets_a_resource_response` from
`rust_to_python_request_receipt.rs`, 319 to 254 lines) and adds the
fixture `python_channel_interop_parts/python_request_forms_matrix.rs`
(485 lines, nine ignored tests, one `include!` line in
`python_channel_interop.rs`), which reuses the two sibling fixtures with
a `REQUEST_FORMS_MATRIX_LARGE` payload of 48 KiB. Every Resource-form
test pins the form through `Transport::resource_events()`, as `Complete {
is_request }`, `Complete { is_response }` or `OutboundComplete`, and the
first Rust-to-Python one also asserts the payload exceeds the measured
`link_mdu()`. Python to Rust:
`python_to_rust_request_handler_resource_request_over_mdu_gets_a_resource_response`,
`..._resource_request_over_mdu_gets_a_packet_response`,
`..._packet_request_gets_a_resource_response_over_mdu`,
`..._none_response_leaves_the_packet_request_unanswered` (the Rust
handler returns `None`, the Python receipt stays `SENT`, the Rust twin of
`RNS/Link.py:844`) and
`..._allow_list_denied_resource_request_fails_on_python` (Rust accepts
the request Resource on size alone as Python does at
`RNS/Link.py:1036-1040`, the handler never runs, Python's Resource-form
receipt reaches `DELIVERED` (`:1372`) and `failed_callback` fires
(`:1398`), so the client reports `request_failed` with status 0). Rust
to Python:
`rust_to_python_request_receipt_resource_request_over_mdu_gets_a_resource_response`,
`..._resource_request_over_mdu_gets_a_packet_response`,
`..._packet_request_gets_a_resource_response_over_mdu` and
`..._two_concurrent_requests_resolve_separately` (two packet-form
requests in flight on one link, each receipt resolving to its own reply
and each Python report naming the matching Rust id). The surviving tests
gain pins: `request_handler_assert_response` takes the expected form and
checks `request_form` on the `request_sent` and terminal lines; the
silent-denial test asserts status 1 (`SENT`, `RNS/Link.py:1319`) and
`request_form == "packet"` under a 12 s setup budget;
`RequestHandlerRecord` records the identity the handler saw
(`InboundRequest::remote_identity`), `Some(peer)` in the allow-list test
and `None` under `RequestAllow::All`. Every async fn but the lock-taking
test fns carries `#[deny(clippy::await_holding_invalid_type)]`.

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
