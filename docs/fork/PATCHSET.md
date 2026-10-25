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
Commits that only update this ledger are omitted.
`tools/scripts/patchset-ledger-check.sh` checks that the current section's
table equals the files changed since the previous release tag (files the
range deletes are told in the patch narrative, not tabled) and that every
commit in that range outside this ledger and the crate `CHANGELOG.md` is
cited in the section. The crate README
(`crates/libs/rns-transport/README.md`) carries the versioning policy, the
rebase recipe and a prose summary of the current patch list; this file is
the per-release ledger of the same patches, with commit hashes and file
sites, so a rebase can re-apply them one commit at a time. For fork-only
parity rows this file plus the crate README `## Known divergences from
Python RNS` are the parity ledger; `docs/status/*` matrices are not updated
per fork task.

## 0.16.0 (upstream 0.14.0)

Every file changed since the `0.15.1` release and the patch each change
belongs to. Patch ids: H1 is test hygiene, H2 is release tooling, H3 is
ledger conventions, H4 is guard discipline, H5 is shared-instance
hardening, H6 is carried hygiene. The `test hygiene` patch is
fork commit `08362717`; the
`release tooling` patch is fork commits `7e63de1a`, `7fbc97cb`, `fbd4da9d`,
`f206f31c`, `8e3e587f`, `0e034698`, `3008c9fb`, `72a3a4ab`, `b48e5d81`,
`9b7bf2e6`, `ab8673b8`, `824b1388`, `bf35ab79`, `d6c59496`, `1405ea9e`,
`4b11d3c0`, `1ee6045e`, `2f41f29e`, `b91113d6` and `cbab03dc`; the
`ledger conventions` patch is fork commit `c45a4e5c`; the
`guard discipline` patch is fork commits `f7be0f41`, `ae4f16d8`, `5f6c230f`,
`e916ce9e`, `39d107b9`, `882baabc`, `e9c2ea93`, `7908fcff`, `044b64f4`,
`fd3b249e`, `d4e1634d`, `71c25285`, `1b7ce1cb`, `6c0ea58e`, `66a703e9`,
`719bff37`, `0d42d2e4`, `e29addcb`, `0c4457e2`, `3e77860a`, `c80d6996`,
`120956aa`, `3b2352c0`, `c6d28577`, `05989491`, `5366399d`, `b879fc9b`,
`d36f611c`, `31299980`, `1b02e70a`, `1393cecd`, `4576a35e`, `57cd07fa`,
`f0e02c44`, `10888bf2`, `b785306b`, `f2bdb2ac`, `23f196ea`, `a594ebb7`,
`64f4fa37`, `b7ddb0ee`, `45d4a0dd`, `2997ad2a`, `9e1145fc`, `2293a968`,
`d9b5ad57`, `47350e23`, `9136c972` and `02316362`; the
`shared-instance hardening` patch is fork commits `829c8e6d`, `d629ac08`,
`cb818b0a`, `a55eaf6f`, `e4046601`, `0c553858`, `f5f8b639`, `6ecb0340`,
`c78bdc7d`, `ca66c50d`, `00bd6579`, `24cf2089`, `31ce6211`, `615d9e45`,
`16850244`, `f8b74067`, `4d6d7280`, `320720af`, `ba84a543`, `597fb2b8`,
`46f5cb21`, `59acbc9f`, `e19fc0ba`, `c0348634`, `0c49f2d9`, `a31cb1a0`,
`91ba55f9`, `b4e0cf47`, `6d6f7aa8`, `f55e9ab2`, `03639529` and
`ea67d176`; the `carried hygiene` patch is fork commits `985bd5bd`,
`81c9ed73`, `faacb7fd`, `9877bb6e`, `5631defa`, `1f321d0d`, `8f6b847c`,
`218d784a`, `dbe495ca`, `4783f1ef`, `db4c5a5e`, `44962a96`, `ce8ed659`,
`9865dbdd`, `e5ba2807`, `1b04a361`, `f890dbf9`, `fd6322b5`, `05975d7d`,
`5cca182d`, `42f988ce`, `f2ea78f9`, `6c5ca548`, `012f7337`, `63bbbc87`,
`2c89e7ba`, `f6c0b59c`, `842feae6`, `c296bb84`, `ceccaeb1`, `22961b3b`,
`71c9e7c6`, `eb45dc62`, `8b3e02eb`, `e1804e1c`, `07c1fe37`, `cbe20719`,
`90ec6e86`, `a2333881`, `18452f13`, `4072932d`, `50db3da3`, `a284f96a`,
`c926a2e6`, `93d28bc6`, `95467e04` and `a30aa093`; the
`Python-in-the-loop identify allow-list interop` patch is the
carried-hygiene cell fork commits `9865dbdd` and `eb45dc62` add, named so
the release's Python-in-the-loop proof rows are attributed to it; the
`release 0.16.0` patch carries no other id and is fork commits `c281063e`,
`a6ad4884`, `1a39183c`, `e14ead8d`, `44c19d48`, `5b9f69a8`, `704cd444`,
`4ebfcc62`, `4e40639e` and `129c96ed`.

| File | Patch |
| --- | --- |
| `.github/workflows/ci-full.yml` | release tooling, carried hygiene |
| `.github/workflows/ci.yml` | release tooling, carried hygiene |
| `AGENTS.md` | ledger conventions |
| `Cargo.lock` | release 0.16.0 |
| `Cargo.toml` | guard discipline, release 0.16.0 |
| `crates/apps/reticulumd/Cargo.toml` | guard discipline, release 0.16.0 |
| `crates/apps/reticulumd/examples/interfaces-reference.toml` | shared-instance hardening |
| `crates/apps/reticulumd/src/bin/reticulumd/announce_ingest.rs` | guard discipline |
| `crates/apps/reticulumd/src/bin/reticulumd/announce_worker.rs` | carried hygiene |
| `crates/apps/reticulumd/src/bin/reticulumd/bootstrap_interface_startup.rs` | shared-instance hardening |
| `crates/apps/reticulumd/src/bin/reticulumd/bootstrap_interface_startup_parts/module_prelude.rs` | shared-instance hardening |
| `crates/apps/reticulumd/src/bin/reticulumd/bootstrap_interface_startup_parts/shared_instance_attach.rs` | shared-instance hardening |
| `crates/apps/reticulumd/src/bin/reticulumd/bootstrap_interface_startup_parts/shared_instance_attach_options_tests.rs` | shared-instance hardening |
| `crates/apps/reticulumd/src/bin/reticulumd/bootstrap_interface_startup_parts/shared_instance_attach_probe2_tests.rs` | shared-instance hardening |
| `crates/apps/reticulumd/src/bin/reticulumd/bootstrap_interface_startup_parts/shared_instance_attach_probe_tests.rs` | shared-instance hardening |
| `crates/apps/reticulumd/src/bin/reticulumd/bootstrap_interface_startup_parts/shared_instance_attach_tests.rs` | shared-instance hardening, carried hygiene |
| `crates/apps/reticulumd/src/bin/reticulumd/bootstrap_parts/configure_startup_rpc_token_auth.rs` | carried hygiene |
| `crates/apps/reticulumd/src/bin/reticulumd/bootstrap_parts/module_core.rs` | carried hygiene |
| `crates/apps/reticulumd/src/bin/reticulumd/bootstrap_transport.rs` | shared-instance hardening |
| `crates/apps/reticulumd/src/bin/reticulumd/bridge_announce.rs` | carried hygiene |
| `crates/apps/reticulumd/src/bin/reticulumd/bridge_propagation.rs` | guard discipline |
| `crates/apps/reticulumd/src/bin/reticulumd/bridge_service_identity.rs` | carried hygiene |
| `crates/apps/reticulumd/src/bin/reticulumd/inbound_control_parts/module_core.rs` | carried hygiene |
| `crates/apps/reticulumd/src/bin/reticulumd/inbound_control_parts/test_validated_peer_links_sections/core_tests.rs` | carried hygiene |
| `crates/apps/reticulumd/src/bin/reticulumd/tests/interface_hot_apply.rs` | guard discipline |
| `crates/apps/reticulumd/src/config_parts/interfaceconfig_sections/enabled.rs` | shared-instance hardening |
| `crates/apps/reticulumd/tests/config.rs` | shared-instance hardening |
| `crates/apps/reticulumd/tests/config_parts/shared_instance_client_knobs.rs` | shared-instance hardening |
| `crates/apps/reticulumd/tests/fork_publish_contract.rs` | release tooling, guard discipline, shared-instance hardening, carried hygiene, release 0.16.0 |
| `crates/apps/reticulumd/tests/fork_publish_contract_parts/ledger_contract.rs` | release tooling, release 0.16.0 |
| `crates/apps/reticulumd/tests/fork_publish_contract_parts/ledger_contract_fixtures.rs` | release tooling |
| `crates/apps/reticulumd/tests/fork_publish_contract_parts/lint_discipline_probe.rs` | guard discipline |
| `crates/apps/reticulumd/tests/fork_publish_contract_parts/lint_discipline_probe10.rs` | guard discipline, shared-instance hardening, carried hygiene, release 0.16.0 |
| `crates/apps/reticulumd/tests/fork_publish_contract_parts/lint_discipline_probe11.rs` | carried hygiene |
| `crates/apps/reticulumd/tests/fork_publish_contract_parts/lint_discipline_probe12.rs` | carried hygiene |
| `crates/apps/reticulumd/tests/fork_publish_contract_parts/lint_discipline_probe2.rs` | guard discipline |
| `crates/apps/reticulumd/tests/fork_publish_contract_parts/lint_discipline_probe3.rs` | guard discipline |
| `crates/apps/reticulumd/tests/fork_publish_contract_parts/lint_discipline_probe4.rs` | guard discipline |
| `crates/apps/reticulumd/tests/fork_publish_contract_parts/lint_discipline_probe5.rs` | guard discipline |
| `crates/apps/reticulumd/tests/fork_publish_contract_parts/lint_discipline_probe6.rs` | guard discipline |
| `crates/apps/reticulumd/tests/fork_publish_contract_parts/lint_discipline_probe7.rs` | guard discipline |
| `crates/apps/reticulumd/tests/fork_publish_contract_parts/lint_discipline_probe8.rs` | guard discipline |
| `crates/apps/reticulumd/tests/fork_publish_contract_parts/lint_discipline_probe9.rs` | guard discipline, release 0.16.0 |
| `crates/apps/reticulumd/tests/fork_publish_contract_parts/release_frozen_sections_probe.rs` | release 0.16.0 |
| `crates/apps/reticulumd/tests/fork_publish_contract_parts/release_tooling_probe.rs` | release tooling |
| `crates/apps/reticulumd/tests/fork_publish_contract_parts/release_tooling_probe2.rs` | release tooling |
| `crates/apps/reticulumd/tests/fork_publish_contract_parts/release_tooling_probe3.rs` | release tooling |
| `crates/apps/reticulumd/tests/fork_publish_contract_parts/release_tooling_probe4.rs` | release tooling |
| `crates/apps/reticulumd/tests/fork_publish_contract_parts/release_tooling_probe5.rs` | release tooling |
| `crates/apps/reticulumd/tests/fork_publish_contract_parts/release_tooling_probe6.rs` | shared-instance hardening |
| `crates/apps/reticulumd/tests/fork_release_docs_contract.rs` | release tooling, release 0.16.0 |
| `crates/apps/reticulumd/tests/python_channel_interop.rs` | guard discipline, carried hygiene, Python-in-the-loop identify allow-list interop |
| `crates/apps/reticulumd/tests/python_channel_interop_parts/multi_segment_python_interop.rs` | test hygiene |
| `crates/apps/reticulumd/tests/python_channel_interop_parts/python_request_late_reply.rs` | carried hygiene |
| `crates/apps/reticulumd/tests/python_channel_interop_parts/rust_identify_python_allow_list.rs` | carried hygiene, Python-in-the-loop identify allow-list interop |
| `crates/apps/reticulumd/tests/python_channel_interop_parts/rust_to_python_raw_resource_roundtri.rs` | carried hygiene |
| `crates/apps/reticulumd/tests/python_channel_interop_parts/rust_to_python_request_receipt.rs` | carried hygiene |
| `crates/apps/reticulumd/tests/python_tcp_ifac_lifecycle.rs` | guard discipline |
| `crates/apps/reticulumd/tests/support/python_channel_endpoint.py` | carried hygiene, Python-in-the-loop identify allow-list interop |
| `crates/apps/rns-tools/Cargo.toml` | release 0.16.0 |
| `crates/apps/rns-tools/src/bin/independent_interop_node/events.rs` | carried hygiene |
| `crates/apps/rns-tools/src/bin/independent_interop_node/model.rs` | carried hygiene |
| `crates/apps/rns-tools/src/bin/independent_interop_node/node.rs` | carried hygiene |
| `crates/apps/rns-tools/src/bin/rncp_parts/network.rs` | carried hygiene |
| `crates/apps/rns-tools/src/bin/rngit_parts/network.rs` | carried hygiene |
| `crates/apps/rns-tools/src/bin/rnsh_parts/network.rs` | carried hygiene |
| `crates/libs/lxmf-sdk/src/backend/zmq_pipeline/transport.rs` | guard discipline |
| `crates/libs/lxmf-sdk/src/backend/zmq_pipeline/tests/mod.rs` | guard discipline |
| `crates/libs/lxmf-sdk/src/client/async_ops.rs` | guard discipline |
| `crates/libs/meshchat-api/src/chat.rs` | guard discipline |
| `crates/libs/meshchat-api/src/control.rs` | guard discipline |
| `crates/libs/meshchat-api/src/lib.rs` | guard discipline |
| `crates/libs/rns-transport/CHANGELOG.md` | test hygiene, release tooling, ledger conventions, guard discipline, shared-instance hardening, carried hygiene, release 0.16.0 |
| `crates/libs/rns-transport/Cargo.toml` | release 0.16.0 |
| `crates/libs/rns-transport/README.md` | release tooling, guard discipline, shared-instance hardening, carried hygiene, release 0.16.0 |
| `crates/libs/rns-transport/examples/tcp_connection_memory.rs` | guard discipline |
| `crates/libs/rns-transport/fuzz/Cargo.lock` | release 0.16.0 |
| `crates/libs/rns-transport/src/destination/link_parts/link.rs` | carried hygiene |
| `crates/libs/rns-transport/src/destination/link_parts/link_sections/data_proof_tests.rs` | carried hygiene |
| `crates/libs/rns-transport/src/destination/link_parts/link_sections/new.rs` | carried hygiene |
| `crates/libs/rns-transport/src/destination/link_parts/link_sections/next_watchdog_deadline.rs` | carried hygiene |
| `crates/libs/rns-transport/src/destination/link_parts/link_sections/pending_requests.rs` | carried hygiene |
| `crates/libs/rns-transport/src/destination/link_parts/link_sections/pending_requests_resolve_tests.rs` | carried hygiene |
| `crates/libs/rns-transport/src/destination/link_parts/link_sections/remove_channel_handler.rs` | shared-instance hardening |
| `crates/libs/rns-transport/src/destination/link_parts/link_sections/response_packet.rs` | carried hygiene |
| `crates/libs/rns-transport/src/destination/link_parts/link_sections/response_record_tests.rs` | carried hygiene |
| `crates/libs/rns-transport/src/destination/link_parts/link_status_predicates_make_lifecycl.rs` | shared-instance hardening |
| `crates/libs/rns-transport/src/destination/link_parts/link_status_predicates_make_lifecycl_sections/core_tests.rs` | shared-instance hardening |
| `crates/libs/rns-transport/src/destination/link_parts/link_status_predicates_make_lifecycl_sections/teardown_sends_link_close_for_handshake_link_tests.rs` | shared-instance hardening |
| `crates/libs/rns-transport/src/destination/link_parts/module_prelude.rs` | shared-instance hardening, carried hygiene |
| `crates/libs/rns-transport/src/destination/request/receipt.rs` | carried hygiene |
| `crates/libs/rns-transport/src/error.rs` | shared-instance hardening |
| `crates/libs/rns-transport/src/hash.rs` | carried hygiene |
| `crates/libs/rns-transport/src/identity/tests.rs` | carried hygiene |
| `crates/libs/rns-transport/src/iface/auto_parts/default_multicast_discovery_address_sections/core_tests.rs` | guard discipline |
| `crates/libs/rns-transport/src/iface/auto_parts/module_prelude.rs` | guard discipline |
| `crates/libs/rns-transport/src/iface/auto_runtime_parts/autoruntimeplan_sections/runtime_json.rs` | guard discipline |
| `crates/libs/rns-transport/src/iface/auto_runtime_parts/autoruntimeplan_sections/send_due_multicast_peer_announces_wi.rs` | guard discipline, carried hygiene |
| `crates/libs/rns-transport/src/iface/auto_runtime_parts/autoruntimeplan_sections/spawn_peer_data_receive_loop.rs` | guard discipline |
| `crates/libs/rns-transport/src/iface/auto_runtime_parts/module_prelude.rs` | guard discipline |
| `crates/libs/rns-transport/src/iface/auto_runtime_parts/module_prelude_sections/transport_bridge.rs` | guard discipline |
| `crates/libs/rns-transport/src/iface/auto_runtime_parts/tests.rs` | guard discipline |
| `crates/libs/rns-transport/src/iface/auto_runtime_parts/tests_sections/auto_peer_data_transport_bridge_regi.rs` | guard discipline |
| `crates/libs/rns-transport/src/iface/auto_runtime_parts/tests_sections/auto_peer_route_origination.rs` | carried hygiene |
| `crates/libs/rns-transport/src/iface/auto_runtime_parts/tests_sections/core_tests.rs` | guard discipline |
| `crates/libs/rns-transport/src/iface/auto_runtime_parts/tests_sections/peer_data_receive_loop_progress.rs` | guard discipline |
| `crates/libs/rns-transport/src/iface/auto_runtime_parts/tests_sections/peer_data_receive_loop_progress_probe.rs` | guard discipline |
| `crates/libs/rns-transport/src/iface/auto_runtime_parts/tests_sections/peer_route_table_expiry.rs` | guard discipline |
| `crates/libs/rns-transport/src/iface/auto_runtime_parts/tests_sections/peer_route_table_probe.rs` | guard discipline, carried hygiene |
| `crates/libs/rns-transport/src/iface/i2p.rs` | guard discipline |
| `crates/libs/rns-transport/src/iface/kiss.rs` | guard discipline, carried hygiene |
| `crates/libs/rns-transport/src/iface/kiss_parts/ifac_runtime_tests.rs` | guard discipline |
| `crates/libs/rns-transport/src/iface/kiss_parts/kisstcpclientinterface.rs` | guard discipline |
| `crates/libs/rns-transport/src/iface/kiss_parts/module_prelude.rs` | guard discipline |
| `crates/libs/rns-transport/src/iface/kiss_parts/shared_receiver_lease_probe_tests.rs` | guard discipline, carried hygiene |
| `crates/libs/rns-transport/src/iface/kiss_parts/tx_queue_survives_reconnect_tests.rs` | guard discipline |
| `crates/libs/rns-transport/src/iface/local.rs` | guard discipline, shared-instance hardening |
| `crates/libs/rns-transport/src/iface/local/tests/tx_queue_survives_reconnect.rs` | guard discipline |
| `crates/libs/rns-transport/src/iface/lora_parts/ifac_runtime_tests.rs` | guard discipline |
| `crates/libs/rns-transport/src/iface/lora_parts/lorainterface.rs` | guard discipline |
| `crates/libs/rns-transport/src/iface/lora_parts/module_extra.rs` | guard discipline |
| `crates/libs/rns-transport/src/iface/lora_parts/module_prelude.rs` | guard discipline |
| `crates/libs/rns-transport/src/iface/meshtastic_parts/ifac_runtime_tests.rs` | guard discipline |
| `crates/libs/rns-transport/src/iface/meshtastic_parts/interface.rs` | guard discipline |
| `crates/libs/rns-transport/src/iface/pipe.rs` | guard discipline |
| `crates/libs/rns-transport/src/iface/pipe_parts/status.rs` | guard discipline |
| `crates/libs/rns-transport/src/iface/pipe_parts/stream.rs` | guard discipline |
| `crates/libs/rns-transport/src/iface/pipe_tests.rs` | guard discipline |
| `crates/libs/rns-transport/src/iface/pipe_tests_parts/transmit_join_probe.rs` | guard discipline |
| `crates/libs/rns-transport/src/iface/pipe_tests_parts/transmit_panic.rs` | guard discipline |
| `crates/libs/rns-transport/src/iface/rnode_bearer_interface_tests.rs` | guard discipline |
| `crates/libs/rns-transport/src/iface/rnode_ble.rs` | guard discipline |
| `crates/libs/rns-transport/src/iface/rnode_multi.rs` | guard discipline |
| `crates/libs/rns-transport/src/iface/rnode_multi_ifac_tests.rs` | guard discipline |
| `crates/libs/rns-transport/src/iface/serial.rs` | guard discipline |
| `crates/libs/rns-transport/src/iface/serial_parts/ifac_runtime_tests.rs` | guard discipline |
| `crates/libs/rns-transport/src/iface/serial_parts/module_prelude.rs` | guard discipline |
| `crates/libs/rns-transport/src/iface/serial_parts/run_serial_stream.rs` | guard discipline |
| `crates/libs/rns-transport/src/iface/serial_parts/transmit_join_probe_tests.rs` | guard discipline |
| `crates/libs/rns-transport/src/iface/tcp_client.rs` | guard discipline, carried hygiene |
| `crates/libs/rns-transport/src/iface/tcp_client/tests/connect_cancel.rs` | carried hygiene |
| `crates/libs/rns-transport/src/iface/tcp_client/tests/tx_queue_survives_reconnect.rs` | guard discipline |
| `crates/libs/rns-transport/src/iface/tcp_client_memory_tests.rs` | guard discipline |
| `crates/libs/rns-transport/src/iface/tcp_server.rs` | guard discipline |
| `crates/libs/rns-transport/src/iface/udp.rs` | guard discipline |
| `crates/libs/rns-transport/src/iface/weave.rs` | guard discipline |
| `crates/libs/rns-transport/src/iface/weave_ifac_runtime_tests.rs` | guard discipline |
| `crates/libs/rns-transport/src/iface/weave_management_handle_tests.rs` | guard discipline, carried hygiene |
| `crates/libs/rns-transport/src/iface_parts/interfacemanager_management.rs` | shared-instance hardening |
| `crates/libs/rns-transport/src/iface_types.rs` | guard discipline |
| `crates/libs/rns-transport/src/resource.rs` | carried hygiene |
| `crates/libs/rns-transport/src/resource/advertisement_limits.rs` | carried hygiene |
| `crates/libs/rns-transport/src/resource/manager.rs` | carried hygiene |
| `crates/libs/rns-transport/src/resource/manager_cleanup.rs` | carried hygiene |
| `crates/libs/rns-transport/src/resource/manager_polling.rs` | carried hygiene |
| `crates/libs/rns-transport/src/resource/manager_proof.rs` | carried hygiene |
| `crates/libs/rns-transport/src/resource/manager_segments.rs` | carried hygiene |
| `crates/libs/rns-transport/src/resource/outcome.rs` | shared-instance hardening, carried hygiene |
| `crates/libs/rns-transport/src/resource/sender.rs` | carried hygiene |
| `crates/libs/rns-transport/src/resource/sender_parts/collision_free_hash.rs` | carried hygiene |
| `crates/libs/rns-transport/src/resource/tests.rs` | carried hygiene |
| `crates/libs/rns-transport/src/resource/tests_advert_limits.rs` | carried hygiene |
| `crates/libs/rns-transport/src/resource/tests_confirm_advertised.rs` | carried hygiene |
| `crates/libs/rns-transport/src/resource/tests_link_scope.rs` | carried hygiene |
| `crates/libs/rns-transport/src/resource/tests_link_scope_outbound.rs` | carried hygiene |
| `crates/libs/rns-transport/src/resource/tests_link_scope_probe.rs` | carried hygiene |
| `crates/libs/rns-transport/src/transport/announce.rs` | guard discipline |
| `crates/libs/rns-transport/src/transport/core.rs` | guard discipline, carried hygiene |
| `crates/libs/rns-transport/src/transport/core_parts/stop_interface.rs` | carried hygiene |
| `crates/libs/rns-transport/src/transport/handler.rs` | guard discipline, shared-instance hardening, carried hygiene |
| `crates/libs/rns-transport/src/transport/handler_parts/path_request.rs` | guard discipline |
| `crates/libs/rns-transport/src/transport/handler_parts/unicast_gc.rs` | carried hygiene |
| `crates/libs/rns-transport/src/transport/inbound_processing.rs` | guard discipline, shared-instance hardening |
| `crates/libs/rns-transport/src/transport/inbound_processing_tests.rs` | guard discipline, shared-instance hardening |
| `crates/libs/rns-transport/src/transport/jobs.rs` | guard discipline, carried hygiene |
| `crates/libs/rns-transport/src/transport/jobs_parts/link_maintenance.rs` | guard discipline |
| `crates/libs/rns-transport/src/transport/jobs_parts/link_table_cleanup.rs` | guard discipline |
| `crates/libs/rns-transport/src/transport/link_table.rs` | shared-instance hardening |
| `crates/libs/rns-transport/src/transport/links_parts/transport_sections/close_channel.rs` | guard discipline |
| `crates/libs/rns-transport/src/transport/links_parts/transport_sections/path_table.rs` | guard discipline |
| `crates/libs/rns-transport/src/transport/links_parts/transport_sections/reset_out_link.rs` | guard discipline |
| `crates/libs/rns-transport/src/transport/links_parts/transport_sections/resource_reader.rs` | guard discipline |
| `crates/libs/rns-transport/src/transport/links_parts/transport_sections/shared_instance.rs` | shared-instance hardening |
| `crates/libs/rns-transport/src/transport/links_parts/transport_sections/shared_instance_announce.rs` | guard discipline |
| `crates/libs/rns-transport/src/transport/links_parts/transport_sections/shared_instance_reconnect.rs` | shared-instance hardening |
| `crates/libs/rns-transport/src/transport/links_parts/transportchannel.rs` | guard discipline |
| `crates/libs/rns-transport/src/transport/mod.rs` | guard discipline, shared-instance hardening, carried hygiene |
| `crates/libs/rns-transport/src/transport/mod_parts/transport.rs` | carried hygiene |
| `crates/libs/rns-transport/src/transport/next_hop_metrics.rs` | guard discipline |
| `crates/libs/rns-transport/src/transport/packet_disk_cache.rs` | carried hygiene |
| `crates/libs/rns-transport/src/transport/path.rs` | guard discipline |
| `crates/libs/rns-transport/src/transport/request_dispatch.rs` | guard discipline, carried hygiene |
| `crates/libs/rns-transport/src/transport/resource_wire.rs` | guard discipline, carried hygiene |
| `crates/libs/rns-transport/src/transport/resource_wire_parts/proof.rs` | guard discipline |
| `crates/libs/rns-transport/src/transport/reticulum_path_store.rs` | guard discipline |
| `crates/libs/rns-transport/src/transport/runtime_management.rs` | carried hygiene |
| `crates/libs/rns-transport/src/transport/shared_instance.rs` | shared-instance hardening |
| `crates/libs/rns-transport/src/transport/tests.rs` | guard discipline, shared-instance hardening, carried hygiene |
| `crates/libs/rns-transport/src/transport/tests_parts/announce_broadcast_policy.rs` | carried hygiene |
| `crates/libs/rns-transport/src/transport/tests_parts/announce_identity_drift.rs` | carried hygiene |
| `crates/libs/rns-transport/src/transport/tests_parts/announce_table_retransmission_gate.rs` | carried hygiene |
| `crates/libs/rns-transport/src/transport/tests_parts/blackhole_path_eviction.rs` | carried hygiene |
| `crates/libs/rns-transport/src/transport/tests_parts/blackholed_path_restore.rs` | carried hygiene |
| `crates/libs/rns-transport/src/transport/tests_parts/disabled_shared_child_local_announce.rs` | carried hygiene |
| `crates/libs/rns-transport/src/transport/tests_parts/disabled_transport_shared_child_local_destination.rs` | carried hygiene |
| `crates/libs/rns-transport/src/transport/tests_parts/enabled_shared_child_local_announce.rs` | carried hygiene |
| `crates/libs/rns-transport/src/transport/tests_parts/enabled_transport_shared_child_local_destination.rs` | carried hygiene |
| `crates/libs/rns-transport/src/transport/tests_parts/encrypted_resource_control_packet.rs` | carried hygiene |
| `crates/libs/rns-transport/src/transport/tests_parts/expired_route_restart_recovery.rs` | carried hygiene |
| `crates/libs/rns-transport/src/transport/tests_parts/held_udp_announce_preserves_peer_sou.rs` | carried hygiene |
| `crates/libs/rns-transport/src/transport/tests_parts/inbound_link_request_registers_unicast.rs` | carried hygiene |
| `crates/libs/rns-transport/src/transport/tests_parts/inbound_proof_link_selection.rs` | carried hygiene |
| `crates/libs/rns-transport/src/transport/tests_parts/link_broadcast_helpers_route_via_bound_iface.rs` | carried hygiene |
| `crates/libs/rns-transport/src/transport/tests_parts/module_prelude.rs` | carried hygiene |
| `crates/libs/rns-transport/src/transport/tests_parts/ordinary_proof_duplicate.rs` | carried hygiene |
| `crates/libs/rns-transport/src/transport/tests_parts/packet_hashlist_flush.rs` | carried hygiene |
| `crates/libs/rns-transport/src/transport/tests_parts/packet_hashlist_flush_probe.rs` | carried hygiene |
| `crates/libs/rns-transport/src/transport/tests_parts/packet_hashlist_flush_probe2.rs` | carried hygiene |
| `crates/libs/rns-transport/src/transport/tests_parts/packet_replay_restart.rs` | carried hygiene |
| `crates/libs/rns-transport/src/transport/tests_parts/pending_out_link_establishment_timeout.rs` | carried hygiene |
| `crates/libs/rns-transport/src/transport/tests_parts/pending_out_link_rediscovery.rs` | carried hygiene |
| `crates/libs/rns-transport/src/transport/tests_parts/pinned_python_local_announce_schedule.rs` | carried hygiene |
| `crates/libs/rns-transport/src/transport/tests_parts/prepared_broadcast_link_contract.rs` | carried hygiene |
| `crates/libs/rns-transport/src/transport/tests_parts/request_dispatch_closed_link.rs` | carried hygiene |
| `crates/libs/rns-transport/src/transport/tests_parts/request_dispatch_closed_link_probe.rs` | carried hygiene |
| `crates/libs/rns-transport/src/transport/tests_parts/resource_advert_reject_warn_cadence.rs` | carried hygiene |
| `crates/libs/rns-transport/src/transport/tests_parts/resource_retry_dispatch.rs` | carried hygiene |
| `crates/libs/rns-transport/src/transport/tests_parts/reticulum_path_restore_bad_cache.rs` | carried hygiene |
| `crates/libs/rns-transport/src/transport/tests_parts/reticulum_runtime_management.rs` | carried hygiene |
| `crates/libs/rns-transport/src/transport/tests_parts/rns_1_5_ingress_admission.rs` | carried hygiene |
| `crates/libs/rns-transport/src/transport/tests_parts/send_packet_outcome_and_resource_request_responses.rs` | carried hygiene |
| `crates/libs/rns-transport/src/transport/tests_parts/shared_child_remote_link_request.rs` | carried hygiene |
| `crates/libs/rns-transport/src/transport/tests_parts/shared_owner_unknown_link_request.rs` | carried hygiene |
| `crates/libs/rns-transport/src/transport/tests_parts/single_destination_delivery_proof.rs` | carried hygiene |
| `crates/libs/rns-transport/src/transport/tests_parts/tcp_connect_cancel.rs` | carried hygiene |
| `crates/libs/rns-transport/src/transport/tests_parts/transport_register_channel_handler_e.rs` | carried hygiene |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback.rs` | carried hygiene |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_lifecycle.rs` | guard discipline, carried hygiene |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_lifecycle_probe.rs` | carried hygiene |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_isolation.rs` | shared-instance hardening, carried hygiene |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_probe.rs` | carried hygiene |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_receipt.rs` | carried hygiene |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_receipt_orders.rs` | carried hygiene |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_resource_form.rs` | carried hygiene |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_resource_form_probe.rs` | carried hygiene |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_resource_form_sizes_cancel.rs` | carried hygiene |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_resource_form_sizes_cancel_probe.rs` | carried hygiene |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_resource_form_sizes_probe12.rs` | carried hygiene |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_routing.rs` | carried hygiene |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_resources.rs` | carried hygiene |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_resources_directions.rs` | carried hygiene |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_resources_gone_link.rs` | carried hygiene |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_resources_outcome.rs` | carried hygiene |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_resources_outcome_probe.rs` | shared-instance hardening |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_resources_reject.rs` | carried hygiene |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_resources_retries.rs` | carried hygiene |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_shared_instance.rs` | carried hygiene |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_shared_instance_endpoint_warn.rs` | shared-instance hardening |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_shared_instance_options.rs` | shared-instance hardening |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_shared_instance_probe10.rs` | shared-instance hardening |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_shared_instance_probe11.rs` | shared-instance hardening |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_shared_instance_probe12.rs` | shared-instance hardening |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_shared_instance_probe13.rs` | shared-instance hardening |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_shared_instance_probe14.rs` | shared-instance hardening |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_shared_instance_probe15.rs` | shared-instance hardening |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_shared_instance_probe16.rs` | shared-instance hardening, carried hygiene, release 0.16.0 |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_shared_instance_probe17.rs` | shared-instance hardening |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_shared_instance_probe17_unix.rs` | shared-instance hardening |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_shared_instance_probe2.rs` | carried hygiene |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_shared_instance_probe3.rs` | carried hygiene |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_shared_instance_probe4.rs` | carried hygiene |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_shared_instance_probe5.rs` | shared-instance hardening |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_shared_instance_probe6.rs` | shared-instance hardening |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_shared_instance_reconnect.rs` | carried hygiene |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_shared_instance_storm.rs` | shared-instance hardening |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_shared_instance_tasks.rs` | shared-instance hardening |
| `crates/libs/rns-transport/src/transport/tunnels.rs` | shared-instance hardening |
| `crates/libs/rns-transport/src/transport/wire.rs` | guard discipline |
| `crates/libs/rns-transport/src/transport/wire_receipt.rs` | carried hygiene |
| `crates/libs/rns-transport/tests/breaking_changes_compile_fail.rs` | shared-instance hardening, release 0.16.0 |
| `crates/libs/rns-transport/tests/multicast_tx_guard.rs` | guard discipline |
| `crates/libs/rns-transport/tests/outbound_outcome_api.rs` | test hygiene, shared-instance hardening, carried hygiene |
| `crates/libs/rns-transport/tests/tcp_client_reconnect.rs` | guard discipline |
| `crates/libs/rns-transport/tests/tcp_connection_memory.rs` | guard discipline |
| `docs/fork/PATCHSET.md` | test hygiene, release tooling, ledger conventions, guard discipline, shared-instance hardening, carried hygiene, release 0.16.0 |
| `docs/getting-started.md` | release 0.16.0 |
| `docs/runbooks/release-runbook.md` | release tooling |
| `docs/status/reticulum-parity-matrix.md` | ledger conventions |
| `tools/scripts/cross-target-clippy-classify.sh` | guard discipline |
| `tools/scripts/old-name-tracked-scan.sh` | release tooling |
| `tools/scripts/patchset-ledger-check.sh` | release tooling, shared-instance hardening |
| `tools/scripts/python-interop-classify.sh` | guard discipline |
| `tools/scripts/semver-major-findings-check.sh` | release tooling, shared-instance hardening |
| `tools/scripts/workspace-clippy-classify.sh` | release tooling, guard discipline |
| `xtask/src/main.rs` | release tooling |
| `xtask/src/main_parts/capture_platform_descriptor.rs` | release tooling |
| `xtask/src/main_parts/release_version_tests.rs` | release tooling |
| `xtask/src/main_parts/run_unused_deps.rs` | release tooling |

Fork commit `08362717`: H1, test hygiene. Rewords the
`tests/outbound_outcome_api.rs` module doc so the plain senders' argument
lists are described as the ones from before this release rather than by a
label from the fork's own planning, and drops the
`multi_segment_python_interop.rs` copy of the interop harness's
`digest_hex` in favour of the shared helper at its three call sites. No
library behaviour changes.

Fork commits `7e63de1a`, `7fbc97cb`, `fbd4da9d`, `f206f31c`, `8e3e587f`,
`0e034698`, `3008c9fb`, `72a3a4ab`, `b48e5d81`, `9b7bf2e6`, `ab8673b8`,
`824b1388`, `bf35ab79`, `d6c59496`, `1405ea9e`, `4b11d3c0`, `1ee6045e`,
`2f41f29e`, `b91113d6` and `cbab03dc`: H2, release tooling. No library behaviour
changes. `7e63de1a` commits the release
checks that lived in the release operator's home directory under
`tools/scripts/`: `patchset-ledger-check.sh` (this ledger's file table
against `git diff --name-only --diff-filter=d <base>..<head>` and a citation
of every non-ledger commit in the range, with a zero-argument form that
takes the latest `coyote-rns-transport-v*` tag as the base),
`workspace-clippy-classify.sh` (the CI clippy invocation with every
diagnostic classified), `old-name-tracked-scan.sh` (the `git ls-files`
twin of the old-package-name contract walk) and
`semver-major-findings-check.sh` (the `cargo-semver-checks` verdict for a
release); the crate README rebase recipe, this preamble and
`docs/runbooks/release-runbook.md` name them. `7fbc97cb` makes the
`contracts` job of `.github/workflows/ci.yml` check out the full history,
run `cargo test -p coyote-rns-transport --tests` (every test target of this
crate, every `crates/libs/rns-transport/tests/*.rs` target and the lib unit
tests, on the pull-request path) and
run the ledger check, mirrors the steps into `.github/workflows/ci-full.yml`,
and deletes `.github/workflows/codex-review.yml` with its prompt
`.github/codex/prompts/review.md`; the contract pin keeps only its `ci.yml`
clauses, the twin pin mirrors every `run:` line of the job, and a new pin
requires the external-test and ledger steps and the full-history checkout.
`fbd4da9d` closes the ledger contract gaps in
`crates/apps/reticulumd/tests/fork_publish_contract.rs` and its new
include!'d part `fork_publish_contract_parts/ledger_contract.rs`: every
patch-header hash must be named by its section's legend, the all-legend-hash
and header-to-legend checks run over `Unreleased` and every released
section from `0.15.1` up, the per-bullet CHANGELOG cite check runs over
every released section from `0.15.1` up (earlier sections keep their
published text), patch ids take one or two digits, the Python-in-the-loop
trap names its rule when it fires, the old-package-name scan reads
`git ls-files --cached --others --exclude-standard`, and a pin requires the
four scripts to exist as bash under `set -euo pipefail` and to be named by
the runbooks. `f206f31c` makes `resolve_release_version` in
`xtask/src/main_parts/capture_platform_descriptor.rs` accept a
`coyote-rns-transport-v*` tag when its version equals the crate manifest
version (returned as written, the fork-tag form
`tools/scripts/check-publish-tag-version.sh` also guards), keeps `v*` and
bare tags on the root `VERSION` comparison, and moves the release-version
unit tests out of `xtask/src/main_parts/run_unused_deps.rs` to
`xtask/src/main_parts/release_version_tests.rs`, included from
`xtask/src/main.rs`. `8e3e587f` makes the zero-argument
`patchset-ledger-check.sh` take the newest `coyote-rns-transport-v*` tag
that is an ancestor of HEAD and grep each commit hash inside the labelled
section, rewrites `old-name-tracked-scan.sh` as one `grep -F -f` pass over
the filtered file list, disables colour in
`semver-major-findings-check.sh` (`CARGO_TERM_COLOR=never NO_COLOR=1`)
instead of stripping it, notes the python3 dependency of
`workspace-clippy-classify.sh`, and names the CI step for what it runs
(`Crate test targets`). `0e034698` adopts the usage pins in
`fork_publish_contract_parts/release_tooling_probe.rs` (script usage
errors and the zero-argument ledger run, the scan script's needles and
filters against the contract constants, the manifest shape
`cargo test --tests` relies on, the `AGENTS.md` carve-out, the
parity-matrix pointer and the CHANGELOG preamble rule), requires the four
scripts to be tracked, compares the shebang line without its newline, pins
the CHANGELOG bullet-cite gate's labels and makes the ci-full twin pin
require the full-history checkout. `3008c9fb` returns the fork tag as
written in `resolve_release_version`, corrects its rustdoc about the tag
guard and adds unit tests for `crate_manifest_version`. `72a3a4ab` orders
the README rebase recipe's ledger check after the bump commit, mirrors the
script's usage shape and covers patch and minor bumps for the semver
script. `b48e5d81` adds
`fork_publish_contract_parts/release_tooling_probe2.rs`, which clones the
repository into a temporary directory and proves the zero-argument ledger
check bases on the newest fork tag reachable from HEAD, cites commits per
section and that the old-name scan reports tracked hits only; aligns the
usage probe's skip guard with the script's `--merged HEAD` predicate and
pins its no-tag failure; scopes the parity-matrix pin to the `0.14.1`
legend; and states the `VERSION` token rule and the dual-tag tiebreak in
the xtask rustdoc with two more `crate_manifest_version` cases. `9b7bf2e6`
moves the README recipe's ledger check into the commit step with the
script's own usage shape, names the python3 dependency of the clippy
classifier, admits a `Removed` bullet in the CHANGELOG rule and points the
parity-matrix row at the `0.14.1` section, and words the ledger script's
header for the table it checks. `ab8673b8` pins in
`xtask/src/main_parts/release_version_tests.rs` the `VERSION` token rule
under a fork tag, the `--version` escape hatch and the flag's spelling, makes
the unparsable-manifest case check the path the xtask context adds, and
gives the clone probe the script's own reachable-tag skip predicate.
`824b1388` adds `fork_publish_contract_parts/release_tooling_probe3.rs`,
which moves every fork tag of a clone onto a commit HEAD does not contain
and proves the zero-argument ledger check fails naming the unmerged-branch
case while the probes' skip predicate reports no reachable tag. `bf35ab79`
adds `fork_publish_contract_parts/ledger_contract_fixtures.rs`, which
exercises the ledger-contract helpers on synthetic sections and changelogs
so a helper that reports nothing could not pass, anchors `ledger_section` on
the heading's trailing space so `0.15.1` cannot match a `0.15.10` heading,
and marks the usage probe's script helpers `#[cfg(unix)]` like their
callers. `d6c59496` appends to the crate README's `0.15.0` register bullet
that the Codex workflow was removed after `0.15.1`, pinned in
`crates/apps/reticulumd/tests/fork_release_docs_contract.rs`. `1405ea9e`
makes `patchset-ledger-check.sh` name a missing previous-release heading
before slicing the section and state that merge commits are never cited,
makes `old-name-tracked-scan.sh` pass without running grep when no tracked
file is scanned, and makes `semver-major-findings-check.sh` reject a
non-numeric expected count as a usage error. `4b11d3c0` gives the
throwaway clone of `fork_publish_contract_parts/release_tooling_probe2.rs`
a clone-local git identity so its `commit-tree` calls do not depend on the
host's, pins that the clone commits with the global and system config out
of reach and auto-detection disabled, and makes the reachable-higher-tag
case assert the missing `9.9.9` heading the ledger check now names.
`1ee6045e` anchors the summary match of `semver-major-findings-check.sh`
so an expected count of 1 cannot match 11, factors its usage line into a
function, names a non-integer count before the usage line, makes
`old-name-tracked-scan.sh` report a tree with nothing to scan as
`old-name scan skipped:` so `old-name scan ok:` stays a pass over scanned
files, and pins the non-numeric count in
`fork_publish_contract_parts/release_tooling_probe.rs`. `2f41f29e` adds
`fork_publish_contract_parts/release_tooling_probe4.rs`, which pins the
scripts' refusals of a previous version with no ledger heading, a scan over
an empty index and a non-numeric semver count. `b91113d6` makes the README
Codex pin in `crates/apps/reticulumd/tests/fork_release_docs_contract.rs`
require every `CODEX_REVIEW_ENABLED` mention to sit in a bullet that says
the workflow was removed, and documents the bare-heading case of
`ledger_section` in `crates/apps/reticulumd/tests/fork_publish_contract.rs`.
`cbab03dc` adds `fork_publish_contract_parts/release_tooling_probe5.rs`,
which answers `cargo semver-checks` from a stub placed first on `PATH` so
the semver script's summary match is pinned as a whole-number count (an
expected 1 is not satisfied by 11), the zero-expected form is pinned to
follow the cargo exit status, and a source walk requires every `commit-tree`
the probes issue to go through the identity-bearing clone; makes the
per-section cite probe of `release_tooling_probe2.rs` read the newest
ledger label instead of the `Unreleased` literal so it survives a release
promotion; removes `GIT_AUTHOR_*`, `GIT_COMMITTER_*` and `EMAIL` from the
clone's git environment so an identity exported by the host cannot satisfy
the identity pin in the clone's place; normalises the semver script's
expected count to decimal after the integer guard so `01` matches `1`; and
drops the non-numeric count case from `release_tooling_probe.rs`, which
`release_tooling_probe4.rs` covers.

Fork commit `c45a4e5c`: H3, ledger conventions. No library behaviour
changes. `AGENTS.md` records that fork-only parity rows live in this file
and the crate README `## Known divergences from Python RNS` while the
`docs/status/*` matrices track upstream rows only; the `#610` compression
row of `docs/status/reticulum-parity-matrix.md` points at the B1 patch
instead of carrying fork hashes; the crate `CHANGELOG.md` preamble widens
its test-only exception from a fixture hardening to any test-only change
that alters no library behaviour (later widened to release tooling and CI).

Fork commits `f7be0f41`, `ae4f16d8`, `5f6c230f`, `e916ce9e`, `39d107b9`,
`882baabc`, `e9c2ea93`, `7908fcff`, `044b64f4`, `fd3b249e`, `d4e1634d`,
`71c25285`, `1b7ce1cb`, `6c0ea58e`, `66a703e9`, `719bff37`, `0d42d2e4`,
`e29addcb`, `0c4457e2`, `3e77860a`, `c80d6996`, `120956aa`, `3b2352c0`,
`c6d28577`, `05989491`, `5366399d`, `b879fc9b`, `d36f611c`, `31299980`,
`1b02e70a`, `1393cecd`, `4576a35e`, `57cd07fa`, `f0e02c44`, `10888bf2`,
`b785306b`, `f2bdb2ac`, `23f196ea`, `a594ebb7`, `64f4fa37`, `b7ddb0ee`,
`45d4a0dd`, `2997ad2a`, `9e1145fc`, `2293a968`, `d9b5ad57`, `47350e23`,
`9136c972` and `02316362`:
H4, guard discipline. The library behaviour changes are: the two
auto-interface peer-table fixes of the eighth and thirteenth commits; the
pipe and serial pump fixes of the eighteenth (a panicking half no longer
leaves the other waiting); `WeaveManagementHandle::try_set_remote_display`
reporting the closed queue at once after the driver has stopped where it
used to buffer up to the channel capacity (fifth); a pipe or serial
transmit-task panic ending the interface instead of respawning it without
a transmit side (fifth, settled by the thirty-first, which replaced the
fifteenth's poll-level panic adapter); and the auto interface's unicast
discovery port stepping down to 65534 when the discovery port is 65535
where it used to overflow (thirty-second). Everything else keeps its
behaviour (the thirty-ninth's one-await lock form, which let two peer-data
receive loops wait on each other, lived only until the forty-third and
never in a release); `clippy::await_holding_invalid_type` (the tokio
`MutexGuard`, `RwLockReadGuard` and `RwLockWriteGuard` types named in
`clippy.toml`) now runs at its default level under the workspace clippy
invocation, and the production code of this crate holds no tokio guard
across an await except where a fn-level allowance names the reason. The
first commit moves
`NextHopMetrics` and the medium path timeout out of `transport/mod.rs`
into `transport/next_hop_metrics.rs` (re-exported at the same path) so the
lock-discipline rustdoc fits under the module cap. The second shortens the
guards that could be shortened without changing the order of effects:
`send_announce` builds its packet under a scoped destination guard and
hands it to `send_packet`, the shape `try_send_announce` already had, so
the destination guard no longer spans the handler lock; `outbound` reads
the broadcast flag under a scoped guard and sends through
`send_broadcast`; the path-table exporters snapshot each interface's mode,
hash and display name under the `iface_manager` guard before taking the
handler guard; `send_resource_from_reader_observed` scopes its first
handler guard in a block. The third documents the lock discipline once, on
`TransportHandler` (the guard spans one job; lock order handler, then
`packet_cache`; handler, then `iface_manager`, then the interface tx
channel; handler, then a link; handler, then a destination), and marks
each of the 36 transport fns whose handler guard crosses an await, the
`Transport::send_*` wrappers and the job fns alike, with the fn-level
allowance; `request_dispatch::send_on_iface` and
`shared_instance_announce::send_packet` lose their special status in
rustdoc. The fourth splits the auto-interface listener supervisors into a
sync take phase under the guard and an async finish phase after it (stop
and join the retired handles, prune the retired socket's outbound routes,
join the pending stops, in that order), hoists the `peer_ifaces` lookup in
`ensure_peer_iface` out of the `if let` scrutinee, and marks the peer-data
receive loop whose one sync body needs both the state and the dedupe
guard. The fifth lets every interface driver own the `InterfaceTxReceiver`
it gets from `InterfaceChannel::split` and lend it into the per-connection
code (`&mut` into the stream fn for tcp_client, local, weave, rnode_multi,
kiss and i2p; the tx loop runs inline on the driver's task for pipe and
serial, so a panic in it now unwinds the driver instead of being recorded
as the `transmit_task_failed` status; moved into the drain task and taken
back through the `JoinHandle` for tcp_server and the local server, whose
tasks were joined, never aborted), hoists udp's nested `PeerRouting`
lookup into a scoped binding, routes management frames through
`recv_optional_management_frame(Option<&mut Receiver<T>>)` for weave and
rnode_multi (kiss reads its own through the private `recv_optional_leased`
since the thirty-third), and keeps two
pub items with the allowance: `run_kiss_stream_with_ifac` (shared-receiver
signature, kept as a wrapper; since the thirty-third the allowance sits on
the private `ReceiverLease::recv` behind it rather than on the wrapper) and
`MeshtasticInterfaceHandle::recv_transmit`. The sixth adds one
module-level allowance per test root (test code holds guards across awaits
on purpose). The seventh removes the `[workspace.lints.clippy]` allow from
`Cargo.toml`, gives `meshchat-api` and `lxmf-sdk` a crate-root allowance
naming the sites a follow-up restructures, rewords the `reticulumd`
package allowance the same way, and rewords
`tools/scripts/workspace-clippy-classify.sh` so its zero-diagnostic
verdict describes the lint as live. The eighth puts the auto-interface
bridge's `peer_ifaces` and `outbound_routes` maps under one
`tokio::sync::Mutex<PeerRouteTable>`, so `ensure_peer_iface` looks up
and inserts under the same guard and expiry clears both halves at once;
the split locks had let a route be re-inserted for a virtual iface that
`remove_expired_peer_routes` had just stopped. The ninth lets the LoRa
driver own its receivers and call `run_kiss_stream_owned` (its management
channel moves to the take-once shape weave and rnode_multi use), makes a
second take of a weave, rnode_multi or LoRa management receiver log a
warning naming the interface, has the serial stream return the caller's
receiver, and adopts the local unix client reconnect fixture. The tenth
makes each allowance comment name the guard that actually crosses the
await (`iface_manager` in `TransportHandler::send`,
`send_recursive_path_request_with_modes`, `announce_retransmit_tick` and
`request_path`; the shared `InterfaceRxReceiver` in `manage_transport`;
the `Link` in `handle_link_resource_packet`), narrows the rustdoc to fns
that own a handler guard, pins `outbound` on the pathless broadcast-on
and broadcast-off cases, and drops the production-code clause from the
test-root comments. The eleventh replaces the `reticulumd` package
allowance and the `meshchat-api` and `lxmf-sdk` crate-root allowances
with a same-line fn-level allowance on each production fn the live lint
reports (thirteen fns; the ninth `meshchat-api` site is a scoped read
instead), and makes the classify script refuse any tracked `Cargo.toml`
that mentions the lint. The twelfth adopts the lint discipline probe into
the publish contract; the fourteenth names the probe's marked-crate table
entry type with an alias so the workspace clippy invocation accepts it.
The thirteenth makes the peer table the authority
on `ensure_peer_iface`'s miss path: when the discovery and data tasks
both miss the first lookup for one peer, the first registration to reach
the table wins, the loser's route folds onto it and the losing virtual
iface is stopped after the guard drops, where before the second insert
overwrote the first and the peer received every broadcast twice. The
fifteenth wrapped the inline pipe and serial transmit loops in a
poll-level panic adapter, `CatchUnwind` (removed again by the
thirty-first), so a panic was logged,
recorded (the pipe `transmit_task_failed` status, the serial
`transmit task failed` last error) and the driver returned with its
receiver, as the spawned task of old did through its `JoinError`; it also
gives `pipe_tests.rs`, whose Linux-only test holds a guard across an
await, the test-root allowance (cross-target clippy on
`x86_64-unknown-linux-gnu` then reports zero diagnostics of the lint for
the workspace minus the two crates that pull `libdbus`). The sixteenth
names the `iface_manager` guard on `send_prepared_packet_broadcast_with_trace`,
credits the `entry()` fold in the transport bridge comment and adopts the
peer-table probe fixture. The seventeenth hardens the lint discipline
probe (allowances counted by owner tag with slack, the open
`allow(clippy::await_holding_invalid_type` form, no production allowance
outside the four pinned crates, a floor on the fn-level deny carriers,
the CHANGELOG section read label-aware, the `reticulumd` manifest checked
for neither a lint table nor workspace inheritance, and the classify
manifest gate exercised against a throwaway clone with a staged allowing
manifest) and makes the classify script's manifest listing fatal on
failure. The eighteenth makes the pipe and serial pumps finish on
whichever half ends first: both select over the caught writer and the
reader `JoinHandle`, cancel `stop` as soon as either ends and then collect
the other, so a reader panic, which never reaches its own `stop.cancel()`,
is recorded promptly instead of leaving the writer waiting on an idle
receiver (the base pipe pump awaited the reader first, so there a writer
panic left the reader waiting on the child's stdout; the base serial pump
awaited the writer first, so there a reader panic left the writer idle;
both are fixes); `CatchUnwind` drops its inner future on a caught panic and refuses a
second poll, and the serial panic arm records `tx_errors` and `link_state`
like its write-error arm. The transmit-loop panic probes move to
`pipe_tests_parts/transmit_panic.rs`, included from `pipe_tests.rs` under
its root attribute. The nineteenth re-attributes the double-broadcast fix
in the transport bridge comment to the bridge's own fan-out, which the
`entry()` fold prevents (the manager already skips `VirtualUnicast`),
widens the `manage_transport` allowance to the
three worker guard families it covers and notes in the `TransportHandler`
rustdoc that `handle_check_links` nests a link guard, then
`iface_manager`. The twentieth records two checks that had only been run
by hand as scripts: `tools/scripts/cross-target-clippy-classify.sh` (the
workspace minus the two crates that pull `libdbus`, linted for
`x86_64-unknown-linux-gnu` with `zig cc` as the C cross-compiler; `SKIP:`
when zig or the target is absent) and `tools/scripts/python-interop-classify.sh`
(the Python-in-the-loop batch, passing only when every failure is one of
the named macOS environment reds or the reference-pin test while the
reference checkout is off its pin). The twenty-first adopts the classify
manifest-gate probes (`lint_discipline_probe2.rs`: an untracked manifest
is ignored and clippy is reached through a stub `cargo`, a `warn`-level
mention is refused, a dangling `gitdir:` makes the listing fatal) and pins
the two scripts (`lint_discipline_probe3.rs`). The twenty-second pins the
stopped weave handle (`weave_management_handle_tests.rs`: before spawn the
`WeaveInterface` value holds the receiver and a remote display command
queues; after `stop_interface` the same call fails at once with
`WouldBlock` naming the closed queue) and drops the serial transmit-panic
arm's `link_state` write, which the unconditional `closed` write a few
lines later always overwrote. The twenty-third makes the interop script
classify every nextest status token (`SIGABRT`, `SIGSEGV`, `TIMEOUT` and
`TRY n FAIL` count as failures; `LEAK`, `SLOW`, `FLAKY` and `TRY n PASS` as passes)
and cross-check its counts against the nextest `Summary` line, captures
the manifest grep's exit code in the classify script and reads the
installed targets through a here-string in the cross-target script. The
twenty-fourth adopts the verdict probes for both scripts
(`lint_discipline_probe4.rs`, a stub toolchain that never runs cargo or
nextest). The twenty-fifth rewords the weave handle fixture so it says the
`WeaveInterface` value, not a driver, holds the receiver before spawn. The
twenty-sixth scopes the classify script's `dead_code` tolerance to macOS
(`uname -s` reports `Darwin`) so a Linux run fails on real dead code, tails
cargo's stderr there too, and makes the interop script keep each outcome's
status token: a listed name is tolerated only on `FAIL` or `TIMEOUT`, the
verdict lists print `name (TOKEN)`, and a `RETICULUM_PY_REPO` that is not
a git checkout fails with a `FAIL:` line. The twenty-seventh tightens the
verdict probes (the cross-build failure arm asserts the stderr tail, the
signal-token arm asserts the exact failure line and counts, the synthetic
`Summary` lines take nextest's shape) and adopts `lint_discipline_probe5.rs`
(`TIMEOUT` and `TRY n FAIL` tokens, a crash in a tolerated name, a cancelled
run, a missing tracked manifest making the listing exit 2, and a Linux
`uname` refusing the macOS tolerance). The twenty-eighth makes the
classify script count codeless error-level diagnostics (a parse, macro or
link error carries no lint code) as failures, so a non-zero cargo exit
passes only when every error-level record is a tolerated `dead_code` one,
and adds the `set -u` guard its cross-target twin had. The twenty-ninth
pins the kiss wrapper's lock span: while `run_kiss_stream_with_ifac` runs
on a duplex stream the shared receiver cannot be locked, and it can once
the stream has been cancelled and joined (the idle stream's pending
receive holds the guard; the thirty-third renames the pin to say so). The
thirtieth adopts
`lint_discipline_probe6.rs` (the stderr tail of the classify script, a
listed macOS name tolerated on `TIMEOUT`, the codeless-error arms) and
teaches the discipline probe that `_tests_parts/` directories are test
code. The thirty-first replaces the fifteenth's
adapter: the pipe and serial transmit loops are spawned tasks again that
carry the receiver in and hand it back through their `JoinHandle`, the
pump still finishes on whichever half ends first, and a transmit-task
panic, which takes the receiver with it, logs and records the failure as
before and then ends the interface (the respawn and reconnect loops
stop); `run_pipe_stream` moves to `pipe_parts/stream.rs` so
`pipe.rs` stays under the module cap, and the adapter module is deleted.
The thirty-second fixes `AutoInterfaceConfig::unicast_discovery_port`
(`auto_parts/module_prelude.rs`): it computed `discovery_port + 1`, which
overflowed in debug builds when a fixture drew the ephemeral port 65535
(Python has no answer either; its bind of `discovery_port + 1` raises,
`RNS/Interfaces/AutoInterface.py:173`), and now uses
`checked_add(1).unwrap_or(u16::MAX - 1)` so the pair stays adjacent and
distinct at the top of the range; pinned by
`unicast_discovery_port_is_the_next_port_and_steps_down_at_the_top_of_the_range`
in `default_multicast_discovery_address_sections/core_tests.rs` (48555 to
48556, 0 to 1, 65535 to 65534), and the three loopback fixtures in
`auto_runtime_parts/tests_sections/core_tests.rs` re-draw a 65535
discovery port through `reserve_loopback_udp_port_below_max`. The
thirty-third returns the pub `run_kiss_stream_with_ifac` to locking each
shared receiver per receive: a private `ReceiverLease` enum (an owned
driver receiver, or a shared mutex receiver re-locked per receive) feeds
the loop body, now `run_kiss_stream_leased`, and `run_kiss_stream_owned`
and the pub wrapper keep their signatures and wrap it, so a co-consumer
queued on the mutex is served between passes (tokio's `Mutex` is fair, so
the guard passes to a queued waiter at the pass boundary); before the
drivers took owned receivers the wrapper locked the tx receiver per pass
and the management receiver per receive, and both are now leased per
receive; the allowance moves
from the wrapper onto `ReceiverLease::recv`, the one span left. Pinned by
`kiss_stream_with_ifac_holds_the_tx_receiver_lock_while_idle_and_frees_it_on_stop`
(`kiss_parts/ifac_runtime_tests.rs`, renamed; both shapes satisfy it) and
`kiss_stream_with_ifac_serves_a_queued_locker_between_transmit_passes` in
`kiss_parts/shared_receiver_lock_span_probe_tests.rs`, which queues a
locker, drives one transmit pass and asserts the waiter is served while
the stream runs (a life-of-stream hold fails it). The thirty-fourth adopts
the transmit-join probes, `pipe_tests_parts/transmit_join_probe.rs`
(included from `pipe_tests.rs`) and
`serial_parts/transmit_join_probe_tests.rs` (a `mod` in `serial.rs`): the
transmit loop is a spawned task that is joined, never aborted, so a write
in flight when the reader ends first still reaches the child or port and
the receiver comes back, and a writer that ends first on a write error
cancels the stop token so a parked reader is collected (pinned on the
pipe side; the serial twin pins only the EOF-first join). The thirty-fifth
makes each of the thirteen app-crate allowance markers (`reticulumd`
`announce_ingest.rs` and `bridge_propagation.rs`, `meshchat-api` `chat.rs`,
`control.rs` and `lib.rs`, `lxmf-sdk` `zmq_pipeline/transport.rs` and
`client/async_ops.rs`) name its guard, the await it spans and whether the
site is to be scoped away or kept by design (the `lxmf-sdk` slot-ownership
and per-key idempotency holds), has the two remaining category sites in
`rns-transport` (`spawn_peer_data_receive_loop.rs`, the documented lock-order
nest; `meshtastic_parts/interface.rs`, the public-API-pinned shared
receiver) name their category (the former went in the thirty-ninth and
came back, with the order documented, in the forty-third), and makes
`lint_discipline_probe.rs` ask
every marked crate for the owner tag plus a non-empty reason instead of
one fixed sentence (the per-file site counts are unchanged). The
thirty-sixth has `tools/scripts/workspace-clippy-classify.sh` describe the
Darwin tolerance precisely (the interop test target builds on macOS; only
its linux-only cases are cfg'd out there, which strands their helpers as
`dead_code`) and has `tools/scripts/python-interop-classify.sh` follow its
`OK:` verdict with `tolerated but did not run: <names|none>` so a stale
tolerated name is visible; pinned in `lint_discipline_probe5.rs` and
`lint_discipline_probe6.rs`
(`interop_script_reports_tolerated_names_that_did_not_run_after_the_ok_line`).
The thirty-seventh drops the dead `process_state = "closed"` write in
`pipe.rs`'s lost-receiver arm (the post-loop block overwrote it with
`stopped`; the log line and `pipe_is_open=false` stay), names the receiver
the unix server in `local.rs` drops after its listener exits, gives the
`TransportHandler` rustdoc in `transport/mod.rs` a `# Lock discipline`
heading that lists two more nests (`iface_manager` before link in `send`'s
announce arm through `note_link_packet_sent`, inert because an announce
never addresses a link; link before destination in the resource request
size check in `resource_wire.rs`), documents on `ExportedIface` in
`transport/reticulum_path_store.rs` the snapshot window between the
`iface_manager` guard and the handler guard (Python reads
`Transport.interfaces` unlocked, `RNS/Transport.py:2983-2984` and
`:3820-3822`) and its unreachable `?` arms, and makes the negative pin in
`tests_parts/two_transport_loopback_lifecycle.rs` use `try_recv`.
The thirty-eighth adopts the shared-receiver lease probes,
`kiss_parts/shared_receiver_lease_probe_tests.rs` (a `mod` in `kiss.rs`):
a co-consumer holding the tx lock parks neither inbound decoding nor
cancellation (a per-pass lock parks the whole loop on the mutex), a
co-consumer that takes the lock after a pass consumes the next queued
packet itself and the stream resumes transmitting once it releases, and
the shared management receiver is leased per receive too (a frame fed
through it reaches the wire byte for byte and a locker queued on its
mutex is served); and the allowance-reason probe,
`fork_publish_contract_parts/lint_discipline_probe7.rs` (included from
`fork_publish_contract.rs`): every production allowance reason names a
guard noun, an await verb and a code identifier or one of the two
sanctioned category phrases, and under `kiss_parts/` the pub wrapper
carries no allowance while the one allowance sits on the lease `recv`
whose shared arm is `.lock().await.recv().await`. The thirty-ninth has
`spawn_peer_data_receive_loop.rs` take its `state` and `dedupe` guards
through one `join!` over the two lock futures and drops the fn's
`await_holding_invalid_type` allowance on the premise that `dedupe.lock()`
has a single production site so no order could invert; the premise
counted sites, not tasks (one receive loop is spawned per adopted NIC and
all of them share both mutexes), and the joined form parks the first-won
guard while the other lock pends, so two loops could wait on each other
on a multi-thread runtime; the forty-third undoes it. The thirty-ninth
also drops two stale `#[allow(dead_code)]`, on that fn (live from
`send_due_multicast_peer_announces_wi.rs`) and on
`transport/jobs_parts/link_table_cleanup.rs` (live from `jobs.rs`); the
production census went from 52 to 51 markers there and back to 52 in the
forty-third (39 in `rns-transport`, 13 in the app crates). The fortieth
drops the lost-receiver arm's
own `pipe_is_open = false` write in `pipe.rs` (the post-loop block writes
it; the log line and the `break` stay), has
`serial_parts/transmit_join_probe_tests.rs` assert `last_error == None`
after the in-flight write is joined and drop its inert root allowance
(the file holds no guard across an await), and has the
`pipe_tests_parts/transmit_join_probe.rs` header state what the
write-error test pins: the status record and the returned receiver, the
parked reader being released by the writer's own `tx_stop.cancel()`
rather than by pump ordering. The forty-first has the `# Lock discipline`
heading in `transport/mod.rs` name the two extra nests by fn (`send`'s
announce arm; the `max_request_size` check in
`handle_link_resource_packet`) instead of by line range and say both run
under the handler guard so they cannot interleave, has the
`transport/resource_wire.rs` marker name the destination
(`max_request_size`) and `iface_manager` (MTU) lock awaits its link
guard spans, and corrects the `transport/reticulum_path_store.rs` cite
to `RNS/Transport.py:3819-3821` (the throttle `if` plus the refresh). The
forty-second adds the `unicast_discovery_port` step-down to the crate
README's divergence register (Python computes `discovery_port + 1`,
`RNS/Interfaces/AutoInterface.py:173`, and its unicast bind fails there)
and has step 7 of the release recipe say that
`workspace-clippy-classify.sh` refuses any tracked manifest that mentions
`await_holding_invalid_type` and tolerates, on macOS only, the
`dead_code` of the interop test target whose linux-only cases are cfg'd
out there; the README cite is walked by `fork_release_docs_contract.rs`.
The forty-third has `spawn_peer_data_receive_loop.rs` take `state` and
then `dedupe` in sequence again, under a fn-level
`await_holding_invalid_type` allowance whose reason names the documented
lock-order nest (state, then dedupe; dedupe has no other locker), and
writes that order on the `dedupe` field of `AutoPeerDataReceiveLoopRuntime`
in `auto_runtime_parts/module_prelude.rs` so the category is now true;
the pin is `tests_sections/peer_data_receive_loop_progress.rs` (included
from `auto_runtime_parts/tests.rs`), a four-worker multi-thread test that
blasts four loopback data sockets under one `spawn_peer_data_receive_loops`
with four hundred datagrams at once and expects every `Processed` event
within two seconds (the joined form can fail this pin; the repeated-burst
twin of the forty-sixth is the one it fails every time, and the sequential
form passes both). The forty-fourth adopts
`fork_publish_contract_parts/lint_discipline_probe8.rs` (included from
`fork_publish_contract.rs`): exactly one production allowance claims the
lock-order-nest category and it sits on `spawn_peer_data_receive_loop`,
whose body takes `state` then `dedupe` sequentially while no `join!` or
`try_join!` over lock futures exists under `auto_runtime_parts` and
`dedupe.lock()` has one production site; `link_table_cleanup.rs` opens
with its marker and carries no `dead_code` allow; the `# Lock discipline`
rustdoc names fns that exist and cites no line ranges; the README register
names the unicast-port step-down with its `AutoInterface.py:173` cite; and
the serial transmit-join probe root binds no tokio guard and carries no
allowance. It also tightens `lint_discipline_probe7.rs` (`held` dropped
from the await verbs, the kiss window bounded by the file length, `e.g.`
and `i.e.` excluded from the dotted-identifier rule, the `sites >= 50`
floor annotated as tracking the census) and has the
`tests_parts/two_transport_loopback_lifecycle.rs` header name its three
announce and outbound pins. The forty-fifth rewords the `dedupe` field
rustdoc to name `spawn_peer_data_receive_loop` as the only locker and
has the probe8 README pin's doc comment describe the cite it checks.
The forty-sixth adopts
`tests_sections/peer_data_receive_loop_progress_probe.rs` (included from
`auto_runtime_parts/tests.rs`), a four-worker multi-thread test that runs
thirty rounds, each binding eight fresh loopback data sockets with a
backlog queued before the loop starts and live sends with no yield, under
one busy thread per core; each round must drain within two seconds and
its loops must shut down (the joined form stalls, the sequential form
passes), and adopts `fork_publish_contract_parts/lint_discipline_probe9.rs`
(included from `fork_publish_contract.rs`): the progress pin keeps the
shape that can see a deadlock (several sockets, a multi-thread runtime, a
bounded wait); the `dedupe` field rustdoc names the only locker and that
name resolves to the fn under the nest marker; no production source in
any crate joins lock futures; the census sentence above equals the
measured production allowances and their split; the probe sources and
auto-runtime test sections are ASCII with headers that cite no task or
lane; and the lifecycle fixture header names each pin its file holds. The
forty-seventh adopts `fork_publish_contract_parts/lint_discipline_probe10.rs`
(included from `fork_publish_contract.rs`): the CHANGELOG Unreleased entry
describes the sequential order under its allowance and no longer vouches
for the joined form; the thirty-ninth narrative above says the premise
counted sites, not tasks, and that the forty-third undoes it; the header's
behaviour sentence confines the joined form to the thirty-ninth through
forty-third and says never in a release; the README row is attributed to
`release tooling, guard discipline` and every row's attribution is a
legend phrase; and the code-identifier rule treats `e.g.` and `i.e.` as
prose and keeps dotted paths. It also tightens `lint_discipline_probe8.rs`
(its rustdoc needle now checks the order itself, `state` before `dedupe`
after `Lock order:`, plus `only locker`; the dead `_probe` path skip is
dropped), has the `peer_data_receive_loop_progress.rs` expect message name
a loopback drop as the other cause and replaces its local plan fixture
with a new `core_tests.rs` `plan_with_data_listeners(Vec<..>)`, to which
the existing `plan_with_data_listener` now delegates, adds one rustdoc line on
`AutoPeerDataListenerSupervisor.dedupe` in `module_prelude.rs` pointing at
the runtime field for the lock order, and has the
`tests_parts/two_transport_loopback_lifecycle.rs` header also name
`try_send_announce` surfacing the build error as `Err`.
The forty-eighth (`9136c972`) has
`fork_publish_contract_parts/lint_discipline_probe9.rs` pin the
repeated-burst twin's shape (a multi-thread flavour, at least two workers,
at least ten rounds, at least two sockets, `CpuPressure::start()`, the
two-second bound and its include from `auto_runtime_parts/tests.rs`), pin
the one-line `AutoPeerDataListenerSupervisor.dedupe` rustdoc that points at
`AutoPeerDataReceiveLoopRuntime::dedupe` for the lock order, add the
`try_send_announce` clause to the lifecycle header pin, drop the dead
`_probe` path filters from both workspace-wide scans, skip intervening
attributes when locating the marked fn, and name the leading-number parse;
has `lint_discipline_probe10.rs` read only the slice between the
thirty-ninth and fortieth narratives for its thirty-ninth pins (so a later
narrative quoting the needles cannot vouch for them) and keep only the
README row equality (the root ledger contract owns the every-row legend
sweep); caps `tests_sections/peer_data_receive_loop_progress_probe.rs` at
a backlog of sixty-four and ninety-six live datagrams per socket (at most
160 datagrams, about 120 KiB by kernel accounting, below the 208 KiB Linux
default receive buffer), has a stalled round report whether
it drained, whether its loops shut down and whether either mutex is still
held, and names a loopback drop as the other cause in its assert; rewords
the `peer_data_receive_loop_progress.rs` sender-registration comment (both
locks are taken either way); and backticks the identifier in the
supervisor pointer rustdoc in `auto_runtime_parts/module_prelude.rs`.
Against the joined form the capped twin fails five of five runs with every
stalled round reporting both mutexes held; the sequential form passes five
of five in under a second.
The forty-ninth has `lint_discipline_probe9.rs` pin that cap (backlog plus
live datagrams per socket at most 160) on the twin's shape test and has the
twin's `drain_burst` rustdoc state the margin in the same terms.
Pinned by
`tcp_client/tests/tx_queue_survives_reconnect.rs`,
`kiss_parts/tx_queue_survives_reconnect_tests.rs`,
`local/tests/tx_queue_survives_reconnect.rs`,
`tests_sections/peer_route_table_expiry.rs`,
`tests_sections/peer_route_table_probe.rs`,
`tests_sections/peer_data_receive_loop_progress.rs` (four receive loops
on one `state`/`dedupe` pair keep making progress under concurrent load),
`tests_sections/peer_data_receive_loop_progress_probe.rs` (thirty bursts
on eight sockets under CPU pressure each drain within two seconds; a
stalled burst reports whether either mutex is still held),
`tests_parts/two_transport_loopback_lifecycle.rs`,
`pipe_tests_parts/transmit_panic.rs` and `serial_parts/run_serial_stream.rs`
(the transmit-loop and reader panic pins),
`pipe_tests_parts/transmit_join_probe.rs` and
`serial_parts/transmit_join_probe_tests.rs` (the transmit-join pins),
`weave_management_handle_tests.rs`,
`fork_publish_contract_parts/lint_discipline_probe.rs`,
`fork_publish_contract_parts/lint_discipline_probe2.rs`,
`fork_publish_contract_parts/lint_discipline_probe3.rs`,
`fork_publish_contract_parts/lint_discipline_probe4.rs`,
`fork_publish_contract_parts/lint_discipline_probe5.rs`,
`fork_publish_contract_parts/lint_discipline_probe6.rs`,
`fork_publish_contract_parts/lint_discipline_probe7.rs` (every
allowance reason names its guard and await; the kiss wrapper holds no
allowance),
`fork_publish_contract_parts/lint_discipline_probe8.rs` (one
lock-order-nest allowance, on the peer-data loop, with the order on the
`dedupe` field; no join over lock futures in the auto runtime),
`fork_publish_contract_parts/lint_discipline_probe9.rs` (the progress pin
and its repeated-burst twin keep their deadlock-visible shape and the
twin's depth cap; the
supervisor's `dedupe` field points at the lock-order doc; no production
source joins lock futures; every census sentence the ledger states adds
up, and the one an Unreleased section states equals the census the tree
carries),
`fork_publish_contract_parts/lint_discipline_probe10.rs` (the ledger and
changelog prose on the lock order, read from the thirty-ninth narrative
itself; the README row's attribution; the probe9 parentheticals of both
0.16.0 sections state the split census pin; both 0.16.0 sections cite the
frozen-sections probe),
`kiss_parts/ifac_runtime_tests.rs` and
`kiss_parts/shared_receiver_lease_probe_tests.rs` (the per-receive lease
of both shared receivers, including a queued locker served across a
transmit pass).

Fork commits `829c8e6d`, `d629ac08`, `cb818b0a`, `a55eaf6f`, `e4046601`,
`0c553858`, `f5f8b639`, `6ecb0340`, `c78bdc7d`, `ca66c50d`, `00bd6579`,
`24cf2089`, `31ce6211`, `615d9e45`, `16850244`, `f8b74067`, `4d6d7280`,
`320720af`, `ba84a543`, `597fb2b8`, `46f5cb21`, `59acbc9f`, `e19fc0ba`,
`c0348634`, `0c49f2d9`, `a31cb1a0`, `91ba55f9`, `b4e0cf47`, `6d6f7aa8`,
`f55e9ab2`, `03639529` and `ea67d176`:
H5, shared-instance hardening. The library behaviour changes are:
`Transport::attach_shared_instance` refuses a `SharedInstanceEndpoint::Tcp`
whose address is not loopback with the new
`RnsError::SharedInstanceEndpointNotLoopback`, before any lock is taken and
with one warning, where it used to attach it with a warning; a caller that
sets `SharedInstanceOptions::allow_remote_endpoint(true)` gets the old
attach with one warning per attach, now in key=value form (first,
breaking); `RejectReason::Oversize` and `RejectReason::Unknown` are gone
from `resource/outcome.rs`, and the storage-feature module
`transport::test_bridge` is gone with its declaration (second, both
breaking); while attached, the duplicate filter is skipped only for packets
arriving on the interface flagged as the shared instance, or on every
interface when none is flagged, where it was skipped on every interface
(third); `Link::teardown` sends the LINKCLOSE packet for a link in
`Handshake` as well as `Active` and `Stale`, and the drop flush would
inherit it, though no `in_links`/`out_links` entry is observable in
`Handshake`: the responder proves, which makes it `Active`, before the
insertion (`transport/path.rs:372` vs `:383`;
`destination/link_parts/link_sections/new.rs:137-138`)
(fourth), and building that LINKCLOSE no longer warns `can't create data
packet for closed link` for a `Handshake` or `Stale` link (thirteenth); the
reconnect consumer's rule that treated a reconnect event arriving without
its disconnect as a lost disconnect and flushed first (fifth) is superseded
by the thirteenth, which restores the plain biased loop and records the
proof that the case cannot occur; the drop flush sends no LINKCLOSE for a link that rode the
dropped interface itself, where the packet used to be queued on the
client's channel and reach the restarted instance as a close for a link it
never knew (sixth); and reticulumd's shared-instance client posture goes
through `Transport::attach_shared_instance`, so the daemon gains the drop
flush and the reconnect re-announce, its unix and TCP shared-instance
clients wait the library default 8 s after a drop instead of the clients'
own 5 s, one shared instance per process is the rule and a second
client-posture entry is a recorded startup failure naming the one already
attached (`iface=<hash>`), a configured non-loopback shared-instance host is refused by the
daemon's own config before any attach, a host name such as `localhost`
resolves to its IPv4 loopback address first (twelfth), the per-interface `fixed_mtu`,
`force_shared_instance_bitrate` and mode reach the client through the
options, `prefer_ipv6` drives the address pick (IPv6 loopback first, then
any IPv6, then the IPv4 order) while `connect_timeout` and
`max_reconnect_tries`, which the shared-instance client has no setting for,
are warned about at startup, left out of the reference TOML's `local` and
`local_client` blocks and no longer echoed in settings JSON for those kinds
(nineteenth), and the daemon's own
stream-reconnect tunnel synthesizer serves only its `tcp_client` and
`backbone_client` interfaces (ninth); under `--strict-interface-startup`
the preflight dials the picked `SocketAddr` rather than the host string, so
a shared instance on the other loopback family is a recorded startup
failure instead of a pass (twenty-fourth); and `prefer_ipv6` is advertised
in the reference TOML's `local` and `local_client` blocks and echoed in
settings JSON for those kinds (twenty-sixth). The fork-only `SharedInstanceOptions`
builders `with_mtu`, `with_forced_bitrate` and `with_interface_mode` reach
the spawned client and `with_reconnect_wait` clamps at origin (first), and
`LocalUnixClient` gains inspection getters (seventh) and
`SharedInstanceOptions` the public `mtu` and `forced_bitrate_bps` getters
(twenty-fifth); everything else keeps its behaviour. The first commit adds
`RnsError::SharedInstanceEndpointNotLoopback` to `error.rs`, has the attach
in `links_parts/transport_sections/shared_instance.rs` refuse a
non-loopback TCP endpoint and warn in key=value form (`endpoint=<addr>`)
for one the options admit, gives `SharedInstanceOptions`
(`transport/shared_instance.rs`) `allow_remote_endpoint`, `with_mtu`,
`with_forced_bitrate` and `with_interface_mode` and has
`with_reconnect_wait` clamp its wait to at least 1 ms at origin as the
clients already did; rewrites
`tests_parts/two_transport_shared_instance_endpoint_warn.rs` around the
refusal and the admitted warn, has `two_transport_shared_instance_probe12.rs`,
`two_transport_shared_instance_probe13.rs` and
`two_transport_shared_instance_probe14.rs` opt in, pins the clamp in
`two_transport_shared_instance_probe10.rs`, adds
`two_transport_shared_instance_options.rs` (included from
`transport/tests.rs`), and gives the README its shared-instance paragraph
and the register row. The second removes `RejectReason::Oversize` and
`RejectReason::Unknown` from `resource/outcome.rs` (both reserved with no
producer; `PeerCancelled` is the one variant and the enum stays
`non_exhaustive`), deletes the storage-feature `transport/test_bridge.rs`
(`InboundTestBridge`, `register`, `reset`, `deliver_outbound`) and its
declaration in `transport/mod.rs`, drops the two arms from
`tests/outbound_outcome_api.rs` and rewords the README outcome sentence;
the deleted `transport/test_bridge.rs` is no longer in the tree and so has
no row in the table above. `cargo semver-checks` against `v0.15.1` reports
four major findings and no minor: `enum_variant_missing` for the two
variants, `function_missing` for `reset`, `register` and
`deliver_outbound`, `module_missing` for `test_bridge` and `trait_missing`
for `InboundTestBridge`. The third gates the duplicate filter in
`transport/inbound_processing.rs` per arriving interface
(`skip_duplicate_filter`), adds `InterfaceManager::has_shared_instance_interface`
to `iface_parts/interfacemanager_management.rs`, updates the test caller in
`transport/handler.rs`, adds a two-interface fixture to
`transport/inbound_processing_tests.rs` and the sentence to the README. The
fourth has `can_send_teardown` in `destination/link_parts/module_prelude.rs`
admit `Handshake`, splits the pin in
`link_status_predicates_make_lifecycl_sections/core_tests.rs`, adds
`link_status_predicates_make_lifecycl_sections/teardown_sends_link_close_for_handshake_link_tests.rs`
(included from `link_parts/link_status_predicates_make_lifecycl.rs`),
rewords the drop-flush rustdoc in
`links_parts/transport_sections/shared_instance_reconnect.rs` and the
README sentence to say every link past `Pending`, and removes the README
register row that had recorded the gap. The fifth tracks the pending
disconnect in `shared_instance_reconnect.rs` (`disconnect_pending`) so a
reconnect that arrives without it flushes first and records the lost event
at debug, adds `tests_parts/two_transport_shared_instance_storm.rs`
(included from `transport/tests.rs`) with the lost-disconnect fixture and a
storm of six restarts that loses no event, and adds the README clause; the
thirteenth supersedes its consumer rule and its fixture. The
sixth has the drop flush in `shared_instance_reconnect.rs` skip the close
packet of a link whose interface is the dropped one
(`close_packets_skipped`) and has the drop line read `N close packet(s)
sent, M skipped on the dropped interface`, re-pins
`two_transport_shared_instance_probe11.rs` on the new line, adds the
skipped-close fixture to the storm file and the README clause. The seventh
gives `LocalUnixClient` (`iface/local.rs`) the inspection getters `addr`,
`mtu_value`, `forced_bitrate_bps`, `reconnect_wait`,
`reconnect_events_enabled` and `disconnect_events_enabled` with a pin in
its test module, makes the re-announce select in
`shared_instance_reconnect.rs` `biased` towards cancel, rewords the
`reverse_table` wording in `transport/link_table.rs`,
`shared_instance_reconnect.rs` and the README (the received-packet entries,
not the receipt guard), extends the `TunnelTable::clear` rustdoc in
`transport/tunnels.rs` with the interface-to-tunnel index and the
`log_shared_instance_task_join_failed` rustdoc in
`links_parts/transport_sections/shared_instance.rs` with the aborted-handle
case. The eighth, test-only, has the lib test logger in
`tests_parts/two_transport_loopback_requests_isolation.rs` admit the
resource target and keep its dropped-outcome-receiver lines, pins in
`two_transport_loopback_resources_outcome_probe.rs` that a verdict whose
receiver was dropped is recorded once at debug, and pins in
`two_transport_shared_instance_tasks.rs` that a task aborted through its own
handle is reported once at warn by `stop`. The ninth adds
`bootstrap_interface_startup_parts/shared_instance_attach.rs` and
`shared_instance_attach_tests.rs` to reticulumd, routes the five
client-posture arms of `bootstrap_interface_startup_parts/module_prelude.rs`
through it, includes both from `bootstrap_interface_startup.rs`, drops the
connected-flag plumbing from `bootstrap_transport.rs` and adds the README
sentence. The tenth widens the README row pin in
`fork_publish_contract_parts/lint_discipline_probe10.rs` to the three
patches that touched the README. The eleventh has
`tools/scripts/patchset-ledger-check.sh` test each commit cite with a bash
substring match instead of a `printf | grep -q` pipe: with this section
past the pipe buffer, `grep -q` exited on an early match while `printf` was
still writing, and under `pipefail` the SIGPIPE read as an uncited commit,
a different one on every loaded run. The twelfth has reticulumd's
`bootstrap_interface_startup_parts/shared_instance_attach.rs` resolve a
shared-instance host name to every address and pick the IPv4 loopback
first, then any IPv4 address, then the first resolved
(`pick_shared_instance_tcp_addr`), where it took the resolver's first
address and so could dial `[::1]` for `localhost` while the shared instance
listened on `127.0.0.1`; it adds the picker's unit test, a `localhost`
attach, the config-refuses-a-remote-host test and the `::1` attach to
`shared_instance_attach_tests.rs`. The thirteenth restores the plain biased
loop in `shared_instance_reconnect.rs` and replaces the fifth's rule with
the proof in its rustdoc (two `mpsc::channel(8)` queues, the disconnect
emitted before the reconnect each cycle, every queued disconnect served
before any reconnect, so a disconnect is never dropped while its reconnect
lands and the realistic storm loss is a reconnect, healed by the next one
that lands; a per-cycle sequence number would change the
`Sender<AddressHash>` payload and is deferred), removes the hand-built
consumer fixture from `tests_parts/two_transport_shared_instance_storm.rs`
and keeps the six-restart storm; adds `LinkStatus::can_build_packet` to
`destination/link_parts/module_prelude.rs` and makes it the guard of
`packet_with_context` in `link_sections/remove_channel_handler.rs`, so a
LINKCLOSE built for a `Handshake` or `Stale` link no longer warns `can't
create data packet for closed link`, pinned in
`link_status_predicates_make_lifecycl_sections/core_tests.rs`, from which
the fourteenth moves that pin to
`teardown_sends_link_close_for_handshake_link_tests.rs` as its own test
since the file sat at the module-size budget; adopts
`tests_parts/two_transport_shared_instance_probe15.rs` (included from
`transport/tests.rs`), the no-consult case in
`transport/inbound_processing_tests.rs` and the Unreleased migration pins
in `tests/breaking_changes_compile_fail.rs`; and corrects the CHANGELOG
and README prose (the proof replaces the lost-disconnect sentence, the
daemon's own loopback refusal is named ahead of the library's, the
`rns-rpc` bridge is no longer called the same shape, the register gains the
dedup-gate row and the attach section names the remaining builders and
getters). The fifteenth, test-only, adopts
`tests_parts/two_transport_shared_instance_probe16.rs` (included from
`transport/tests.rs`): a `Handshake` teardown builds its LINKCLOSE with no
`can't create data packet for closed link` warning, with a data packet
built from the same `Handshake` link as the positive control that still
warns; and a reconnect served without its disconnect, the shape the fifth's
rule read as a lost disconnect, re-synthesizes the tunnel, runs no flush,
infers no lost event and keeps the learned path. That is the one pin of the
thirteenth's reverted rule: both storm tests stay green under the fifth's
consumer, so this fixture is what discriminates. The lib test logger in
`tests_parts/two_transport_loopback_requests_isolation.rs` gains the
closed-link packet warning family the count reads. The sixteenth has the
attach debug line in `links_parts/transport_sections/shared_instance.rs`
name the endpoint kind as `kind=`
(`iface=<hash> kind=tcp reconnect_wait=<wait>`) where it used `endpoint=`,
the key the warn lines use for the address, and re-pins the line's shape in
`tests_parts/two_transport_shared_instance_probe5.rs` and
`tests_parts/two_transport_shared_instance_probe6.rs`. The seventeenth,
doc-only, corrects three rustdocs: the `TunnelTable::clear` doc in
`transport/tunnels.rs` names the interface-to-tunnel index's Python twin,
`interface.tunnel_id` (`RNS/Transport.py:2827`, `:2834`, read at `:2467`),
which `shared_connection_disappeared` (`:3690-3702`) leaves set so Python's
`note_path` on a vanished tunnel logs the `:2469` warning where the fork's
does nothing; the no-flag arm comment in `transport/inbound_processing.rs`
names both routes to it, a posture set by hand and `stop_interface` of the
attached interface; and the `SharedInstanceOptions` doc in
`transport/shared_instance.rs` says loopback is recognised in any notation
after `to_canonical()`. The eighteenth, test-only, anchors the storm's
cycle counter in `tests_parts/two_transport_shared_instance_storm.rs` on
the drop summary's `reverse route(s) cleared` tail, since the per-link
close-packet line shares the `disconnected,` prefix and must not count as a
cycle, rewords the file header to say the storm pins the ordering and the
loss-free service at restart rate, and adds the `#[cfg(unix)]` twin of the
mode-and-MTU options pin to
`tests_parts/two_transport_shared_instance_options.rs`. The nineteenth has
reticulumd's `bootstrap_interface_startup_parts/shared_instance_attach.rs`
honour `prefer_ipv6` in `pick_shared_instance_tcp_addr` (IPv6 loopback
first, then any IPv6, then the IPv4 order) and warn at startup for
`connect_timeout` and `max_reconnect_tries` (`ignored_shared_instance_keys`;
`[daemon] <posture> name=<label> ignores <key>`), drops the two keys from
the `local` and `local_client` blocks of `examples/interfaces-reference.toml`
and from the settings JSON echo for those kinds in
`config_parts/interfaceconfig_sections/enabled.rs`, deletes the dead
`local | local_client` branch of `build_tcp_client_adapter` in
`bootstrap_interface_startup_parts/module_prelude.rs` and retargets its
bitrate test as `local_client_attach_registers_the_mode_and_mtu_from_the_entry`
in `shared_instance_attach_tests.rs`, which also gains the `prefer_ipv6`
pick, the ignored-key list and
`local_client_localhost_host_connects_to_the_ipv4_loopback_listener` (a
listener on `127.0.0.1` alone sees the accept). The twentieth has
`tools/scripts/semver-major-findings-check.sh` test each expected lint with
a bash substring match instead of `printf | grep -q`, the pipe shape the
eleventh dropped from the ledger script. The twenty-first reflows two
lines of `tests_parts/two_transport_shared_instance_probe6.rs` and
`tests_parts/two_transport_shared_instance_probe16.rs` that the shorter
`kind=` key and a long test name let `rustfmt` put on one line. The
twenty-second writes the README's drop-flush paragraph (the `Handshake`
LINKCLOSE would be inherited; `interface.tunnel_id` is the index's twin)
and the register's responder-`Active`-on-prove row. The twenty-third,
test-only, adds three files: reticulumd's
`bootstrap_interface_startup_parts/shared_instance_attach_probe_tests.rs`,
mounted from `shared_instance_attach_tests.rs` (`prefer_ipv6` dials the
`[::1]`-only listener for `localhost`; one startup warning per ignored key
under either client posture and none without the knobs;
`force_shared_instance_bitrate` paces the first frame on the new stream),
`crates/apps/reticulumd/tests/config_parts/shared_instance_client_knobs.rs`,
included by `tests/config.rs` (the two knobs still parse on the local
kinds, `connect_timeout = 0` is still refused, the echo drops them for
those kinds alone and the reference TOML's local blocks no longer advertise
them), and `tests_parts/two_transport_shared_instance_probe17.rs` (the
forced bitrate paces the first frame on a fresh stream), and points the
header of `tests_parts/two_transport_shared_instance_options.rs` at it.
The twenty-fourth has `startup_local_tcp_attach_endpoint` in
`bootstrap_interface_startup_parts/module_prelude.rs` resolve and pick the
address before the `--strict-interface-startup` preflight, which now dials
the picked `SocketAddr` rather than the host string, whose
`TcpStream::connect(&str)` tried every resolved family and so passed on a
loopback the attach never dialled; two pins in
`shared_instance_attach_probe_tests.rs` (a listener on the other loopback
family alone is a recorded startup failure naming the picked address; the
picked family listening passes and sees the accept). The twenty-fifth
widens `SharedInstanceOptions::mtu` and
`SharedInstanceOptions::forced_bitrate_bps` in `transport/shared_instance.rs`
to `pub`, factors `shared_instance_options_for` out of
`attach_shared_instance_client` in
`bootstrap_interface_startup_parts/shared_instance_attach.rs` and pins it
in the new `shared_instance_attach_options_tests.rs`, mounted next to the
probe child (the client's default MTU and no forced bitrate on a bare
entry; the entry's `mtu` and bitrate otherwise). The twenty-sixth
advertises `prefer_ipv6` in the `local` and `local_client` blocks of
`examples/interfaces-reference.toml`, echoes it for those kinds in
`config_parts/interfaceconfig_sections/enabled.rs`, documents the pick
order on `pick_shared_instance_tcp_addr` (loopback of the preferred family,
any address of it, the other family likewise, then the resolver's first
answer, reached by an IPv6-only answer under the default order) and pins
the single-family non-loopback arms in
`shared_instance_attach_options_tests.rs` and the advert and the echo in
`shared_instance_client_knobs.rs`. The twenty-seventh tightens the
endpoint-warn pins in
`tests_parts/two_transport_shared_instance_endpoint_warn.rs` (the refusal
names `allow_remote_endpoint`; the accepted-remote line names the
interface), has `data_packet_into` in
`link_sections/remove_channel_handler.rs` guard with
`can_build_packet(PacketContext::None)` as `packet_with_context` does,
documents `has_shared_instance_interface` in
`iface_parts/interfacemanager_management.rs` (the duplicate filter's
per-interface skip falls back to every interface while it is false) and
`remove_stale`'s pruning of the interface-to-tunnel index in
`transport/tunnels.rs`, says in the header of
`tests_parts/two_transport_shared_instance_storm.rs` that the fixture sits
below the drop regime by construction (each restart waits for the
re-accept, so the two `mpsc::channel(8)` queues never hold more than two
events), and has `lint_discipline_probe10.rs` say the README row carries
every patch attribution that touched it. The twenty-eighth has
`tools/scripts/semver-major-findings-check.sh` run `cargo semver-checks`
under its own `<CARGO_TARGET_DIR or target>/semver-findings-check` target
dir: both placeholder builds wrote their rustdoc JSON to the workspace
`target/doc` the rustdoc gate also uses, where a fresh `cargo doc` let one
side parse the other side's JSON and the verdict became a false "no semver
update required"; the new
`fork_publish_contract_parts/release_tooling_probe6.rs`, included from
`fork_publish_contract.rs`, pins the dir and the unchanged check-release
arguments with a recording `cargo` stub, with and without an operator-set
`CARGO_TARGET_DIR`. The twenty-ninth, test-only, adds reticulumd's
`bootstrap_interface_startup_parts/shared_instance_attach_probe2_tests.rs`,
mounted from `shared_instance_attach_tests.rs` (the default pick rejects an
IPv6-only listener for `localhost`; an IP-literal host passes strict
startup and attaches; the lenient path never preflights). The thirtieth
has the second-entry refusal in
`bootstrap_interface_startup_parts/shared_instance_attach.rs` end with
`iface=<hash>` of the entry holding the slot, read from
`Transport::shared_instance`, and has
`second_client_posture_entry_is_recorded_as_already_attached` in
`shared_instance_attach_tests.rs` assert it on the batch failure and the
runtime record. The thirty-first marks `SharedInstanceOptions::mtu` and
`SharedInstanceOptions::forced_bitrate_bps` in `transport/shared_instance.rs`
`#[must_use]` as the builders are, names the two getters in the crate
README, adds `tests_parts/two_transport_shared_instance_probe17_unix.rs`,
included from `transport/tests.rs` (the forced bitrate paces the first
frame on a fresh stream over a raw `UnixListener`, the unix arm of the
spawned client), and has the header of
`tests_parts/two_transport_shared_instance_options.rs` name both wire
pins. The thirty-second has every skip note in
`shared_instance_attach_tests.rs`, `shared_instance_attach_probe_tests.rs`
and `shared_instance_attach_probe2_tests.rs` print `skipped:` and adds to
`shared_instance_attach_probe2_tests.rs` a pin that feeds the `[::1]`
`pick_shared_instance_tcp_addr` returns under `prefer_ipv6` straight into
`attach_shared_instance_client` and sees the accept on the `[::1]`-only
listener, so it needs no resolver and cannot skip where `localhost` has no
AAAA record. Pinned by
`tests_parts/two_transport_shared_instance_endpoint_warn.rs`,
`tests_parts/two_transport_shared_instance_options.rs`,
`tests_parts/two_transport_shared_instance_probe10.rs`,
`tests_parts/two_transport_shared_instance_probe11.rs`,
`tests_parts/two_transport_shared_instance_probe12.rs`,
`tests_parts/two_transport_shared_instance_probe13.rs`,
`tests_parts/two_transport_shared_instance_probe14.rs`,
`tests_parts/two_transport_shared_instance_probe15.rs` (the refusal wins
over a held slot and over a stopped transport, and the error's `Display`
names the rule),
`tests_parts/two_transport_shared_instance_probe16.rs` (the quiet
`Handshake` LINKCLOSE with its data-packet positive control, and the lone
reconnect that keeps the tables),
`tests_parts/two_transport_shared_instance_probe17.rs` (the forced bitrate
paces the first frame on a fresh stream) with
`tests_parts/two_transport_shared_instance_probe17_unix.rs` (the same over
the unix arm),
`tests_parts/two_transport_shared_instance_probe5.rs` and
`tests_parts/two_transport_shared_instance_probe6.rs` (the `kind=` line),
`tests_parts/two_transport_shared_instance_storm.rs` (the skipped close
packet and the storm of six restarts),
`tests_parts/two_transport_shared_instance_tasks.rs`,
`tests_parts/two_transport_loopback_resources_outcome_probe.rs`,
`transport/inbound_processing_tests.rs` (the two-interface dedup gate and
the no-consult case), `tests/breaking_changes_compile_fail.rs` (the
Unreleased migrations),
`link_status_predicates_make_lifecycl_sections/teardown_sends_link_close_for_handshake_link_tests.rs`
and `link_status_predicates_make_lifecycl_sections/core_tests.rs`,
`tests/outbound_outcome_api.rs`, the `iface/local.rs` test module and
reticulumd's `bootstrap_interface_startup_parts/shared_instance_attach_tests.rs`
(the attached record's interface fills the transport's slot and sets the
connected posture for the TCP, `LocalClientInterface` TCP and unix arms;
two client entries leave one attached record and one failure; a remote
host is refused by the config before any attach; `::1` and `localhost`
attach, the latter on the IPv4 loopback address, whose listener sees the
accept; the pick with and without `prefer_ipv6`; the ignored-key list; the
mode and MTU on the attached interface),
`bootstrap_interface_startup_parts/shared_instance_attach_probe_tests.rs`
(`prefer_ipv6` reaches the dial, one warning per ignored key, the forced
bitrate paces the first frame, strict startup passes iff the picked family
listens),
`bootstrap_interface_startup_parts/shared_instance_attach_probe2_tests.rs`
(the default pick, the IP-literal host and the lenient path under the
strict preflight order; the `[::1]` pick fed straight into the attach),
`bootstrap_interface_startup_parts/shared_instance_attach_options_tests.rs`
(the options an entry asks for; the single-family pick arms) and
`crates/apps/reticulumd/tests/config_parts/shared_instance_client_knobs.rs`
(parse, echo, zero-timeout refusal, reference TOML text).

Fork commits `985bd5bd`, `81c9ed73`, `faacb7fd`, `9877bb6e`, `5631defa`,
`1f321d0d`, `8f6b847c`, `218d784a`, `dbe495ca`, `4783f1ef`, `db4c5a5e`,
`44962a96`, `ce8ed659`, `9865dbdd`, `e5ba2807`, `1b04a361`, `f890dbf9`,
`fd6322b5`, `05975d7d`, `5cca182d`, `42f988ce`, `f2ea78f9`, `6c5ca548`,
`012f7337`, `63bbbc87`, `2c89e7ba`, `f6c0b59c`, `842feae6`, `c296bb84`,
`ceccaeb1`, `22961b3b`, `71c9e7c6`, `eb45dc62`, `8b3e02eb`, `e1804e1c`,
`07c1fe37`, `cbe20719`, `90ec6e86`, `a2333881`, `18452f13`, `4072932d`,
`50db3da3`, `a284f96a`, `c926a2e6`, `93d28bc6`, `95467e04` and
`a30aa093`: H6, carried hygiene. The library behaviour
changes are: an inbound resource cancel, hash update, part or
split-segment advertisement reaches only a transfer on the link it
arrived on (first); a rejected resource advertisement is counted per link
and warned on the first and every 64th, where the only warn for it sat in
a branch that could not run (second); an in-limit response packet for a receipt
already `Receiving` cancels the response Resource still in flight instead
of leaving it to complete (third); a link restart or close forgets the
unproven data hashes (fourth); a response whose link is no longer `Active`
is dropped before a packet is built (fifth), the resource form under its
second link guard too (thirty-eighth); a resource retry is recorded
as not dispatched only when no interface sent or queued it (sixth); the
packet hashlist flush waits for the packet cache and replaces the file by
rename (seventh); the `TcpClient` connect await breaks out on cancel
(eighteenth); a split assembly is torn down, and an outgoing resource
served, only on the link that owns it, and an advertisement the receiver
constructor refuses joins the per-link count (twenty-eighth); a response
record no receipt owns cancels its overtaken Resource as a receipt's does
(twenty-ninth); each hashlist save writes its own temporary file
(thirtieth); and the load-time orphan sweep tolerates an orphan another
process removed first (fortieth) and warns about, instead of failing the
load on, one it cannot remove (forty-third). `RejectReason` gains `Display` (eleventh). In the apps,
reticulumd control requests decode through `RequestFrame::decode`
(tenth), the app announce sites report a failed announce build with the
role that asked for it (twelfth, thirty-sixth) and the rns-tools
independent interop node merges link snapshots instead of replacing them
(thirteenth) and reports `is_path_response` from the announce most
recently seen for the link's destination when the link is first recorded
(thirty-second).
Everything else keeps its behaviour. The first commit makes every lookup
in the packet path of `resource/manager.rs` require the entry's `link_id`
to equal the arriving link's: the split-segment expected-segment check in
admission, the hash update, the part loop and both `cancel_into` arms,
which now take the link id from the dispatcher; before, inbound receivers
and segment assemblies were keyed by hash alone, so a peer naming a
sibling link's resource hash could tear down or alias that link's
assembly, and a receiver cancel for an outgoing resource was open the
same way. Pinned by the two cross-link cancel cases in
`resource/tests_link_scope.rs` (written into
`resource/tests_timeouts_lifecycle.rs` and moved by the seventeenth). The
second drops from `resource/advertisement_limits.rs` the transfer-size
warn branch that the smaller reference cap ahead of it made unreachable,
turns the part-count line into a debug line, and has
`ResourceManager::record_rejected_advertisement` count each drop in
`LinkStats::rejected_advertisements` (`resource.rs`), warning on the first
and on every 64th. Pinned by
`resource_manager_counts_rejected_advertisements_per_link` in
`resource/tests_advert_limits.rs`. The third has `resolve_pending_request`
(`link_sections/pending_requests.rs`) hand back the receipt's inbound
response Resource on the packet arm as well as the oversize one, so the
accepted-packet arm of `link_sections/new.rs`, which already queues what
that fn returns as an owed cancel the transport sends on the same packet,
now owes it too; Python's receipt is
single-outcome, the packet sets READY (`RNS/Link.py:873`) and removes the
pending request (`:881-883`), the Resource's later conclusion finds
nothing (`:897-912`) and only a FAILED receipt's Resource is cancelled
(`:1417-1435`), so the crate README register row is rewritten to say the
Resource is cancelled here. Pinned by
`link_sections/pending_requests_resolve_tests.rs` and by
`tests_parts/two_transport_loopback_requests_resource_form_sizes_cancel_probe.rs`,
whose in-limit case now expects one receiver cancel where it expected
none. The fourth has `Link::restart` and `finalize_local_close`
(`link_sections/next_watchdog_deadline.rs`) clear `unproven_data_hashes`
beside the other per-session state, so a hash recorded for a finished
session cannot match a late proof. Pinned by
`link_sections/data_proof_tests.rs`. The fifth has `send_packet_response`
and `send_response_resource` (`transport/request_dispatch.rs`) read the
link status under the guard they already hold and drop the response with
a debug line when `LinkStatus::can_exchange_data` (widened to `pub(crate)`
in `link_parts/module_prelude.rs`) is false; the resource form returns
`ConnectionError`, as it does when the advertisement never leaves (the
thirty-eighth repeats the check under the resource form's second guard).
Pinned by `tests_parts/request_dispatch_closed_link.rs`, whose oracle the
thirty-ninth widens to the swept-link arm. The sixth has
`send_resource_retry` (`transport/jobs.rs`) log `retry_not_dispatched`
only when `sent_ifaces == 0 && queued_ifaces == 0`, the pairing the
delivery-proof dispatch already used, opens the fn to the transport tests
as `pub(super)`, and teaches the shared `TestLogger` in
`tests_parts/two_transport_loopback_requests_isolation.rs` the jobs
target. Pinned by `tests_parts/resource_retry_dispatch.rs` (an interface
that matches nothing records the miss; a link without a bound interface
still leaves through path routing). The seventh has
`Transport::save_packet_hashlist` (`transport/runtime_management.rs`)
clone the packet cache out of the handler guard and await its lock in a
statement of its own, where it used `try_lock` under the handler guard
and failed `packet cache is busy` whenever an inbound packet was in
flight, and has `ReticulumPacketDiskCache::save_packet_hashlist`
(`transport/packet_disk_cache.rs`) write `packet_hashlist.raw.tmp` and
rename it over the target. Pinned by
`tests_parts/packet_hashlist_flush.rs`. The eighth documents
`Transport::send_prepared_packet_broadcast_with_trace` (`transport/core.rs`)
as the send for already-prepared, non-link packets that records nothing
in the packet cache or the link tables; the body is unchanged. Pinned by
`tests_parts/prepared_broadcast_link_contract.rs`. The ninth has
`transport/wire_receipt.rs` resolve proof links through
`resource_wire::link_by_id` instead of its own copy of the lookup, and
drops from that helper (`transport/resource_wire.rs`) the
`out_links.get(link_id)` step, which never hit because `out_links` is
keyed by destination hash. The tenth has reticulumd's
`parse_control_request_payload`
(`src/bin/reticulumd/inbound_control_parts/module_core.rs`) decode through
`RequestFrame::decode`, which bounds depth and lengths before allocating
and refuses trailing bytes and reserved markers, where `rmp_serde` had
accepted trailing bytes, and refuses a `requested_at` that is not
`float64`, so the control-request fixture in
`inbound_control_parts/test_validated_peer_links_sections/core_tests.rs`
sends an f64 `requested_at` instead of Nil; it also pins the trailing-byte
and reserved-marker refusals. The eleventh implements `Display` for
`RejectReason` (`resource/outcome.rs`), points the `RequestError::Receive`
rustdoc (`destination/request/receipt.rs`) at the crate README register,
and adds that register row: a response Resource failing in transfer fails
the receipt here, where Python leaves the receipt in its last status
(`RNS/Link.py:1367`, `:1377-1381`, `:1435`). Pinned by the `to_string`
assertion in `tests/outbound_outcome_api.rs`. The twelfth moves the 19
app `send_announce` calls across nine files to `try_send_announce`, so a
failed announce build is reported with the role that asked for it:
reticulumd's `announce_worker.rs`, `bridge_announce.rs`,
`bridge_service_identity.rs`, `bootstrap_parts/module_core.rs` and
`bootstrap_parts/configure_startup_rpc_token_auth.rs` warn, rns-tools'
`rncp_parts/network.rs` and `rngit_parts/network.rs` print under the
silent flag, `rnsh_parts/network.rs` warns and
`independent_interop_node/node.rs` returns the error from its announce
RPC; `send_announce` itself is unchanged. The thirteenth has the
independent interop node's `links` snapshot
(`independent_interop_node/events.rs`) merge each event into the
first-seen entry: identity fields stay, `state` moves, a `states` history
keeps the last 16 event names, and an `is_path_response` field is emitted
as null because the transport's `PathEntry` does not record the announce
context that created the path. The fourteenth is a test-only
Python-in-the-loop cell: the Python endpoint
(`tests/support/python_channel_endpoint.py`) gains `--allow-identified`,
which registers the request handler with `ALLOW_LIST` over a list filled
from the remote-identified callback, and
`python_channel_interop_parts/rust_identify_python_allow_list.rs` sends a
request before identifying, sees Python stay silent, identifies and gets
the reply on the same link. The fifteenth hoists four blocks token for
token into new part files under the module cap: the `Transport` struct
into `transport/mod_parts/transport.rs`, `gc_unicast_ifaces` into
`transport/handler_parts/unicast_gc.rs`, `stop_interface` into
`transport/core_parts/stop_interface.rs` and the collision-free hash
selection into `resource/sender_parts/collision_free_hash.rs`. The
sixteenth removes six `#[allow(dead_code)]` that suppressed nothing
(`request_path` in `transport/handler.rs`; `remove_listener` and
`shutdown_all` on both listener supervisors and
`restart_link_local_listener` on the peer-data one in
`iface/auto_runtime_parts/autoruntimeplan_sections/send_due_multicast_peer_announces_wi.rs`,
all still public) and documents `handle_inbound_for_test`
(`transport/core.rs`) as the test seam it is. The seventeenth keeps the
touched modules under the size cap: the link-scope tests move into
`resource/tests_link_scope.rs` (included from `resource/tests.rs`) and
the two rns-tools announce sites of the twelfth fold into fewer lines,
rngit's page and git calls becoming one loop, so the twelfth's 19 call
expressions are 18 sites from here on.
The eighteenth puts the `TcpClient` connect (`iface/tcp_client.rs`) in
a biased `select!` over the two cancel tokens, cancel arms ahead of the
timed connect, so `Transport::stop` on a client whose connect is in
flight against a black-holed endpoint returns inside the stop budget
instead of after the connect timeout. Pinned by the nineteenth's `iface/tcp_client/tests/connect_cancel.rs`
(which from the thirty-ninth on asserts the runtime status reads
`connecting` before the cancel lands) and
`tests_parts/tcp_connect_cancel.rs`. The nineteenth is test-only:
besides the connect-cancel pins it adds `resource/tests_confirm_advertised.rs`
(`confirm_advertised` keeps the serve-time timestamps once a part was
served and clamps the awaiting-proof retry budget), the no-event check on
the rejecting side in `tests_parts/two_transport_loopback_resources_reject.rs`
(helper in `tests_parts/two_transport_loopback_resources.rs`), the 64 MiB
`data_size` refusal in `resource/tests_advert_limits.rs`, and the
plus-prefixed hex pair refusals in `hash.rs` and `identity/tests.rs`. The
twentieth is test-only: the publish contract gains
`fork_publish_contract_parts/lint_discipline_probe11.rs`, the
free-function `join`/`join3`/`join_all` twin of the macro lock sweep, and
the three auto-interface tests in
`iface/auto_runtime_parts/tests_sections/peer_route_table_probe.rs` and
`auto_peer_route_origination.rs` that left a peer data tx loop running
now shut it down and join it under a one second timeout. The
twenty-first is test-only and consolidates weaker twins: the
`remove_interface` and `stop_interface` slot tests share one body in
`tests_parts/two_transport_shared_instance_probe4.rs` selected by
`SharedInstanceRemoval` and called from
`two_transport_shared_instance_probe2.rs`; the single-receipt test of
`two_transport_loopback_requests_resource_form_sizes_cancel.rs` goes,
its two-receipt probe twin being a strict superset; the prove-all interop
test in `python_channel_interop_parts/rust_to_python_request_receipt.rs`
goes in favour of the late-reply twin in `python_request_late_reply.rs`,
whose proof wait moves into `await_python_proof_of`; probe3 and probe4
share `shared_instance_attach_inside_a_stop`; `stall_transport_for` in
`two_transport_loopback_resources_retries.rs` is re-expressed on
`stall_transport_until_released` with a timed release and `stall_handler`
goes; and `rust_to_python_raw_resource_roundtri.rs` folds its two inline
sha2 report checks into `assert_python_response_report`. The
twenty-second is test-only: the four exact status-order fixtures in
`tests_parts/two_transport_loopback_requests_receipt_orders.rs` gate the
responder on `observe_statuses_releasing_at` (release once the subscriber
has read `Delivered` for the resource form, `Sent` for the packet form)
instead of a 300 ms `HANDLER_DELAY`, which
`two_transport_loopback_requests_receipt.rs` drops. The twenty-third is
test-only and dedups fixtures: `await_outbound_terminal` moves from
`two_transport_loopback_requests_resource_form_probe.rs` into
`two_transport_loopback_resources.rs` and replaces the inline matcher at
five sender-terminal sites (one in that fixture, one in
`two_transport_loopback_resources_directions.rs`, two in
`two_transport_loopback_resources_reject.rs` and one in
`two_transport_loopback_resources_outcome.rs`);
`loopback_link_pair_with_request_limit` takes an `Option<usize>` in the
directions fixture with `loopback_link_pair` as its wrapper, deleting the
reject copy; and `shared_instance_client_with_options` in
`two_transport_shared_instance.rs` serves
`two_transport_shared_instance_probe2.rs` and
`two_transport_shared_instance_reconnect.rs`. The twenty-fourth is
test-only: the six tests unrelated to
`tests_parts/held_udp_announce_preserves_peer_sou.rs` move, bodies
verbatim and with the advertisement helper include they use, into
`tests_parts/send_packet_outcome_and_resource_request_responses.rs`. The
twenty-fifth is test-only: rustfmt over the 37 include!'d fixtures under
`transport/tests_parts/` that failed `rustfmt --check` (the 38th was split
first); the diff is line
joins and splits plus five single-expression closure bodies losing their
braces, with the token stream otherwise identical. The twenty-sixth is a
style-only rustfmt pass over the hoisted
`resource/sender_parts/collision_free_hash.rs` (one type alias re-joined):
the hoisted part files are rustfmt-canonical and token-identical to the
bodies they replace, which is the sense in which the hoists are byte for
byte; a newly created include!'d file may be rustfmt-canonical where its
parent was not.
The twenty-seventh is test-only: it adopts
`tests_parts/resource_advert_reject_warn_cadence.rs` (65 over-cap
advertisements on one loopback link warn exactly twice, at `rejected=1`
and `rejected=64`, a second link starts its own count and no rejection
creates receiver state), has the shared `TestLogger` in
`tests_parts/two_transport_loopback_requests_isolation.rs` keep the
rejected-advertisement warns filtered by link id, and adds two manager
pins to `resource/tests_link_scope.rs` (a part arriving on another link is
not fed to the receiver advertised on the first; a split segment arriving
on another link naming the first link's original hash is dropped
unanswered). The twenty-eighth has `fail_inbound_segments` and
`take_inbound_segments` (`resource/manager_segments.rs`) and
`take_cancelled_incoming` (`resource/manager_cleanup.rs`) take the
receiver's link id and remove a split assembly only when it belongs to
that link, the pre-filter in `cancel_incoming_response` folding into that
gate and the callers in `resource/manager.rs` and
`resource/manager_polling.rs` passing the id; before, a receiver admitted
on one link could name any original hash, and failing or cancelling it
removed whatever assembly sat under that hash, another link's included,
and reported the failure under that link. The same commit counts an
advertisement the receiver constructor refuses (the 64 MiB data cap, a
malformed segmentation header, the parts bound) through
`record_rejected_advertisement` like the manager's
own guards, with `data_size` beside `transfer_size` in the warn
(`resource/manager.rs`, `resource/advertisement_limits.rs`); serves a
`ResourceRequest` or `ResourceProof` only when the sender belongs to the
link it arrived on (`resource/manager.rs`, `resource/manager_proof.rs`),
so a sibling link can neither draw parts nor conclude a transfer it does
not own; and pairs the READY cite in the `resolve_pending_request`
rustdoc (`link_sections/pending_requests.rs`) with the
`response_received` transition (`RNS/Link.py:1437-1443`) and the
`handle_response` entry (`:857-866`). Pinned by the teardown cases in
`resource/tests_link_scope.rs`, the new
`resource/tests_link_scope_outbound.rs` (another link's request earns no
parts and its proof concludes nothing; the same packets on the sender's
own link are served), the refused-advertisement count in
`resource/tests_advert_limits.rs` and, from the fortieth, the aliasing
receiver cases in `resource/tests_link_scope_probe.rs`. The twenty-ninth has the packet arm of
`link_sections/new.rs` spend a response record through the new
`take_response_limit_for_packet` (`link_sections/response_packet.rs`),
which hands back the Resource the record was receiving whether it admits
or refuses the packet, so a response record no receipt owns is cancelled
by the packet that answers it as a receipt's is under the third; the
Resource-arrival gate keeps admitting without a cancel, since the arrival
it admits is that Resource itself, and the README register row says the
rule covers both record kinds. Pinned by
`link_sections/response_record_tests.rs` and the two pins in
`tests_parts/two_transport_loopback_requests_resource_form_sizes_probe12.rs`,
which flip from no cancel to one (the thirty-ninth has the real-resource
pin wait for the overtaken transfer's cancel event). The thirtieth has
`ReticulumPacketDiskCache::save_packet_hashlist`
(`transport/packet_disk_cache.rs`) write `packet_hashlist.raw.tmp-<pid>-<n>`
from a process-wide counter, where the debounced persistence worker and
the shutdown flush wrote one `packet_hashlist.raw.tmp` and whichever
renamed second found its source gone and failed with `NotFound`; a failed
save removes its own temporary, `load_packet_hashlist` sweeps the
`packet_hashlist.raw.tmp-*` orphans a crash left, and the Windows
pre-remove goes since rename replaces the target there too. Pinned by
`tests_parts/packet_hashlist_flush.rs` (two saves under `tokio::join!`, a
stranger's `packet_hashlist.raw.tmp` left with its bytes untouched, the
error-path removal and the load sweep) and, from the fortieth, the
first-boot sweep shapes in `tests_parts/packet_hashlist_flush_probe.rs`.
The thirty-first is rustdoc and a
test: `Transport::send_prepared_packet_broadcast_with_trace`
(`transport/core.rs`) now says nothing sent through it is recorded,
whatever the destination type, where the eighth singled out link packets,
and names its caller as the one matching proofs by the hash in the trace;
`tests_parts/prepared_broadcast_link_contract.rs` gains a
`DestinationType::Single` twin built the way the opportunistic sender
builds its packet. The thirty-second has the independent interop node's
`links` snapshot report `is_path_response` from the announce most
recently seen for the link's destination at the moment the link's first
event creates its entry: `SharedState` (`independent_interop_node/model.rs`)
gains `announce_path_response`, the announce collector in
`independent_interop_node/events.rs` writes each `AnnounceEvent`'s flag
under a short guard and the link collector reads it before taking the
links lock; the field is null only when no announce for the destination
had been seen, where the thirteenth emitted null for every link on the claim
that the transport did not expose the flag. Pinned from the fortieth by
the announce-flag test in `independent_interop_node/events.rs`. The thirty-third is test and
comment only: `python_channel_interop_parts/rust_identify_python_allow_list.rs`
cites `RNS/Link.py:820-822` and `:853-855` where it pointed one line
early at both; the header of `tests_parts/request_dispatch_closed_link.rs`
says the pre-gate response was encrypted under the all-zero session key
`finalize_local_close` had left and sent anyway, where it said the build
failed at encryption; and `send_resource_retry` (`transport/jobs.rs`)
carries one comment saying `queued_ifaces` counts paced announces and
stays zero for a Direct retry. The thirty-fourth is test-only:
`assert_transfer_completes` moves from
`tests_parts/two_transport_loopback_resources_directions.rs` into
`tests_parts/two_transport_loopback_resources.rs` and takes an
`Option<u32>` segment count (`None` for a single-segment transfer, which
emits no `SegmentComplete`), `assert_small_transfer_completes` goes, its
callers in `two_transport_loopback_resources_retries.rs` and
`two_transport_loopback_resources_gone_link.rs` pass `None` and call
`assert_no_further_complete` themselves as the directions tests already
did, and the two inline sender-terminal match blocks in those two fixtures become
`await_outbound_terminal` calls. The thirty-fifth is test-only: the
publish contract gains `fork_publish_contract_parts/lint_discipline_probe12.rs`
(included from `fork_publish_contract.rs`), which walks the tracked
sources under `crates/apps/` and fails on any production call of the bare
`send_announce`, the test helper `tests/support/python_channel_events.rs`
being the one sanctioned call. The thirty-sixth has reticulumd's
`spawn_destination_announce_scheduler`
(`bootstrap_parts/configure_startup_rpc_token_auth.rs`) take the role its
two callers in `bootstrap_parts/module_core.rs` pass, so an undelivered
scheduled announce warns as `propagation` or `control` like the sibling
warns of the twelfth. The thirty-seventh is a style-only rustfmt pass over
the new `resource/tests_link_scope_outbound.rs` fixture (one call
re-wrapped). The thirty-eighth has `send_response_resource`
(`transport/request_dispatch.rs`) re-check `can_exchange_data` at the top
of its second link guard, the one taken after the MTU lookup released the
first, and return `ConnectionError` with the fifth's debug line when a
close landed in between, where it encrypted the advertisement under the
key `finalize_local_close` had zeroed; the window sits between two lock
acquisitions in one async fn and has no deterministic pin, the fifth's
loopback cases covering the common path. The thirty-ninth is test and
doc only: both tests in `iface/tcp_client/tests/connect_cancel.rs`
deny `clippy::await_holding_invalid_type` and assert the runtime status
reads `connecting` before the cancel, so they prove an in-flight connect
was interrupted; the shared `TestLogger` in
`tests_parts/two_transport_loopback_requests_isolation.rs` records the
`is gone` arm beside `closed before transmit`, since a maintenance tick
can sweep the closed link before the handler of
`tests_parts/request_dispatch_closed_link.rs` answers; the header of
`tests_parts/two_transport_loopback_requests_resource_form_sizes_probe12.rs`
states the cancel contract of the twenty-ninth and its real-resource test
waits for the overtaken transfer's `InboundFailed` cancel event and then
drains `resource_events()` for a `Complete` under the id, where a 500 ms
negative wait had passed on the pre-fix code as well; and the
`RequestError::Receive` rustdoc (`destination/request/receipt.rs`) and the
README register row cite the response arm, `request_timed_out`
(`RNS/Link.py:914-918`) failing only a receipt still `DELIVERED` (`:1398`)
and leaving a `RECEIVING` one (`:1420`), where the eleventh cited the
request-Resource arm. The fortieth is test-only but for one arm: it adopts
`resource/tests_link_scope_probe.rs` (a receiver on one link aliasing
another link's split assembly fails through the retry sweep and through
the local response cancel while the assembly it named completes),
`tests_parts/packet_hashlist_flush_probe.rs` (an orphan beside no saved
list; a storage directory that does not exist yet) and the announce-flag
test in `independent_interop_node/events.rs`; the one arm is
`remove_orphaned_hashlist_temporaries` (`transport/packet_disk_cache.rs`)
treating a `NotFound` from `remove_file` as success, since another process
sharing the storage may have swept the orphan first, other errors
failing the load until the forty-third. The forty-first is docs and a test pin: the ledger rows
this patch touched carry its phrase, the publish contract's pin on the
README row (`fork_publish_contract_parts/lint_discipline_probe10.rs`)
follows, and the README's `RejectReason` sentence and the in-limit packet
register row (now citing `handle_response`, `RNS/Link.py:857-866`) are
reworded. The forty-second is test-only: the announce-flag test in
`independent_interop_node/events.rs` carries the fn-level guard deny its
siblings carry, and the probe12 cancel test matches the `cancelled` reason
exactly. The forty-third is the one behaviour change of the close-out:
`remove_orphaned_hashlist_temporaries` (`transport/packet_disk_cache.rs`)
warns, with the path, about an orphan `remove_file` cannot remove and
carries on, where it failed the whole load and hid the valid list beside
the orphan; the file name is `HASHLIST_FILE`, and
`Transport::restore_packet_hashlist` (`transport/runtime_management.rs`)
carries the fn-level `await_holding_invalid_type` deny its save twin has.
Pinned by `tests_parts/packet_hashlist_flush_probe2.rs` (a directory
carrying the temporary prefix beside a saved list: the load warns once and
returns the list; it loads without a further warning once the entry is
gone), through a packet-disk-cache warn store on the shared `TestLogger`
in `tests_parts/two_transport_loopback_requests_isolation.rs`. The
forty-fourth is test-only but for log lines: it adopts
`tests_parts/request_dispatch_closed_link_probe.rs` (a close landing
inside the MTU lookup of `send_response_resource`, forced by holding the
interface manager the lookup needs, is refused with `ConnectionError`,
recorded once and publishes no resource event; the same stall on a live
link sends); `send_response_resource` (`transport/request_dispatch.rs`)
logs the swept-link arm as `is gone` beside the two `closed before
transmit` gates, the second tagged `(after MTU lookup)`, so every drop
lands in the test store; and the three connect-cancel tests
(`iface/tcp_client/tests/connect_cancel.rs`,
`tests_parts/tcp_connect_cancel.rs`) assert the `connecting` precondition
with a message saying a fast failure against TEST-NET-1 would not exercise
the cancel arm. The forty-fifth is hygiene: the split-assembly failure
warn (`resource/manager_segments.rs`) names the link; the response-limit
locals in `link_sections/new.rs` and `link_sections/response_packet.rs`
are `receiving_record_hashes`; the four `#[allow(dead_code)]` on live
interval and keep-alive constants in `transport/mod.rs` go;
`handle_inbound_for_test` (`transport/core.rs`) is documented as public
API; probe11 says the futures combinators poll in declaration order and
classifies a definition by `fn ` ahead of the match; and probe12 drops the
sanctioned-path clause the test-path skip already covers. The forty-sixth
is the run-closing hygiene pass over the whole batch, test-only: it
removes `iface/kiss_parts/shared_receiver_lock_span_probe_tests.rs` (and
its mount in `iface/kiss.rs`), whose queued-locker scenario
`shared_receiver_lease_probe_tests.rs` reproduces (spec 1 the pass and
the handoff, spec 2 the queued wait); removes the
slot-only `host = "localhost"` attach test from
`shared_instance_attach_tests.rs`, whose accept-oracle sibling subsumes it;
drops the named compile-fail step from the `contracts` job of both
workflows, the crate test-targets step already running that harness
(`fork_publish_contract.rs` now asks for four steps); and rewords three
test-file comments (`two_transport_shared_instance_probe16.rs`,
`weave_management_handle_tests.rs`, `shared_receiver_lease_probe_tests.rs`)
to state the contract rather than the change history. The forty-seventh
answers the review of the forty-sixth, test-only: spec 1 of
`shared_receiver_lease_probe_tests.rs` says what it discriminates beside
the idle pin, the probe16 negative assert states its contract, the weave
handle header paraphrases the CHANGELOG without quote marks, and the
CHANGELOG files the deleted fixture and CI step under Removed.

Fork commits `9865dbdd` and `eb45dc62`: Python-in-the-loop identify
allow-list interop. The fourteenth and thirty-third carried-hygiene commits
above: a Rust client identifies against a Python request handler whose
`allowed_list` names it
(`crates/apps/reticulumd/tests/python_channel_interop_parts/rust_identify_python_allow_list.rs`
included from `crates/apps/reticulumd/tests/python_channel_interop.rs`,
the `--allow-identified` flag of
`crates/apps/reticulumd/tests/support/python_channel_endpoint.py`), the
release section's Python-in-the-loop proof rows.

Fork commits `c281063e`, `a6ad4884`, `1a39183c`, `e14ead8d`, `44c19d48`,
`5b9f69a8`, `704cd444`, `4ebfcc62`, `4e40639e` and `129c96ed`: release
0.16.0. The first retargets the promotion-sensitive pins to the literal
0.16.0 label, since a released section is frozen:
`crates/apps/reticulumd/tests/fork_publish_contract_parts/ledger_contract.rs`
expects the full-hash and bullet-cite contracts to cover `0.16.0` and
`0.15.1`,
`crates/apps/reticulumd/tests/fork_publish_contract_parts/lint_discipline_probe9.rs`
and
`crates/apps/reticulumd/tests/fork_publish_contract_parts/lint_discipline_probe10.rs`
read the `0.16.0` ledger and changelog sections (probe10 expects the
README row to carry `release 0.16.0`),
`crates/libs/rns-transport/tests/breaking_changes_compile_fail.rs` names
its migrated consumer and rejected shapes after 0.16.0, and the header of
`crates/libs/rns-transport/src/transport/tests_parts/two_transport_shared_instance_probe16.rs`
stops attributing the biased-loop wording to the README. The second bumps
the crate version to 0.16.0 at every manifest, consumer-snippet and
contract-test site (`Cargo.toml`, `crates/libs/rns-transport/Cargo.toml`,
`crates/apps/reticulumd/Cargo.toml`, `crates/apps/rns-tools/Cargo.toml`,
the snippet in `crates/libs/rns-transport/README.md` and
`docs/getting-started.md`, the two pins in
`crates/apps/reticulumd/tests/fork_publish_contract.rs`) and regenerates
`Cargo.lock` and `crates/libs/rns-transport/fuzz/Cargo.lock`. The third
promotes `## Unreleased` to `## 0.16.0 - 2026-10-25` in the crate
`CHANGELOG.md` with a version bullet at the head of `### Changed`, the
`### Breaking` subsection first, and the compile-fail cross-reference
renamed with its test. The fourth adds the 0.16.0 paragraph to the
`## Patch list` of `crates/libs/rns-transport/README.md`, folding it into
the intro sentence and leaving the 0.15.1 block as a standalone paragraph
with its bullets unchanged. The fifth tightens the register: the README
0.16.0 block drops the reconnect-consumer clause (that consumer is
unchanged since 0.15.1) and describes the host pick as loopback-first in
the family `prefer_ipv6` selects, the CHANGELOG version bullet attributes
the four semver findings to the two removals,
`crates/apps/reticulumd/tests/fork_release_docs_contract.rs` pins the
patch-list order, the 0.16.0 block's items and the surviving 0.15.1
sentences, and
`crates/apps/reticulumd/tests/fork_publish_contract_parts/lint_discipline_probe10.rs`
pins the forty-sixth narrative's spec split. The ledger promotion itself
names the Python-in-the-loop identify allow-list interop patch above. The
sixth splits the probe9 census pin in
`crates/apps/reticulumd/tests/fork_publish_contract_parts/lint_discipline_probe9.rs`:
every ledger census sentence must be arithmetically consistent, and the
live production count is compared only against an Unreleased section that
states one, so a later marker retirement never asks for an edit to a
released section; and the README register pin in
`crates/apps/reticulumd/tests/fork_release_docs_contract.rs` names the
reticulumd-migration bullet by its own words. The seventh adds
`crates/apps/reticulumd/tests/fork_publish_contract_parts/release_frozen_sections_probe.rs`,
included from `crates/apps/reticulumd/tests/fork_publish_contract.rs`:
every `## <version>` section of the crate CHANGELOG and of this ledger that
existed at the newest fork tag reachable from HEAD must still read byte for
byte as it did there, in order and contiguous, and the README patch-list
block of that release keeps its bullets; the README register pin in
`crates/apps/reticulumd/tests/fork_release_docs_contract.rs` also asks for
the `prefer_ipv6` host-pick wording and refuses the superseded
reconnect-consumer clause, and the header of
`crates/libs/rns-transport/tests/breaking_changes_compile_fail.rs` names
its per-release constant pairs. The eighth rewords the probe9
parentheticals of the 0.16.0 sections to the split census pin, names the
root test file in the Python-in-the-loop patch header, and has the 0.15.0
register of `crates/libs/rns-transport/README.md` name 0.16.0 as the
release that records the Codex workflow removal. The ninth pins the
eighth's wording:
`crates/apps/reticulumd/tests/fork_publish_contract_parts/lint_discipline_probe10.rs`
reads the probe9 parenthetical of both 0.16.0 sections and refuses the
superseded census phrases,
`crates/apps/reticulumd/tests/fork_release_docs_contract.rs` asks the Codex
bullet to name 0.16.0 and refuses the by-position wording, and the
lane-free-header scan in
`crates/apps/reticulumd/tests/fork_publish_contract_parts/lint_discipline_probe9.rs`
covers every probe file under `fork_publish_contract_parts/`. The tenth
pins the ledger citations of the frozen-sections probe:
`crates/apps/reticulumd/tests/fork_publish_contract_parts/lint_discipline_probe10.rs`
asks the CHANGELOG 0.16.0 section to name
`release_frozen_sections_probe.rs` and state its byte-for-byte and
README-bullets clauses after it, and asks this ledger's 0.16.0 section for
the probe's table row and the narrative sentence that names its include
site; alongside, the split-census rustdoc no longer claims a tree-wide
search for the superseded phrases finds only the ledgers (probe9's own
live-compare message matches too), the two no-op fallbacks after a split in
`lint_discipline_probe10.rs` and `release_frozen_sections_probe.rs` become
find-based slices, and the probe9 header says census sentence as both
ledgers do.

## 0.15.1 (upstream 0.14.0)

Every file changed since the `0.15.0` release and the patch each change
belongs to. Patch ids: F4 is shared-instance client packaging, F5 is
shared-instance client parity. The
`shared-instance client packaging` patch is fork commits `2a027a9d`,
`b306b2a1`, `17b41d6a`, `ff8a8e8a`, `ff67110d`, `e4494576`, `21e4dacc`,
`a6036013`, `8a1c2d16`, `9247525e`, `afa72a00`, `c05832b9`, `013d0dd2`,
`b7df209e`, `6baf2e34`, `2e62274d`, `eba27ade`, `a4fdb0bb`, `952d6ca9`,
`18e2ccb5`, `52be26ad`, `428a699a`, `4b67a5e9`, `5319bcde` and `114cd382`;
the `shared-instance client parity` patch is fork commits `5af0c425`,
`c8a296f3`, `60656cdb`, `54035599`, `9dfc446e`, `8e20e0d4`, `4f41b2f2`,
`c972552f`, `867264ef`, `cce0a328`, `0f581ef3`, `41064bcb`, `6089ea85`,
`a9ce646f`, `98f4073f`, `2b44522c`, `8a33660f`, `f3613fba`, `fbfa0589`,
`1bb9b2cc`, `47cf9313`, `05337466`, `3239bf09`, `ef3827a6`, `f38a8b81`,
`abf09730`, `7eeaafb0`, `e790cb55` and `6ca33e3f`; the
`proof cap stall on signal`
patch carries no other id and is fork commit `cccc40e4`; the
`Python-in-the-loop shared-instance interop` patch carries no other id and
is fork commit `eceeb9ad`; the `release 0.15.1` patch carries no other id
and is fork commits `e529730c`, `84d90f75`, `9b2e20a6`, `0df584b1`,
`b4a941d8`, `59b66d8e`, `257c180d`, `8559e395`, `6fed34de`, `dffe6f9b`,
`134140fb`, `df914502`, `692d9327`, `e5e3bc88`, `d632c5e9`, `4fd3a8dc`,
`4994a8f1`, `cfb0f62f`, `220a4794` and `fd10554b`.

| File | Patch |
| --- | --- |
| `.github/workflows/ci-full.yml` | release 0.15.1 |
| `Cargo.lock` | release 0.15.1 |
| `Cargo.toml` | release 0.15.1 |
| `crates/apps/reticulumd/Cargo.toml` | release 0.15.1 |
| `crates/apps/reticulumd/tests/fork_publish_contract.rs` | shared-instance client packaging, release 0.15.1 |
| `crates/apps/reticulumd/tests/fork_release_docs_contract.rs` | shared-instance client packaging, shared-instance client parity, release 0.15.1 |
| `crates/apps/reticulumd/tests/python_channel_interop.rs` | Python-in-the-loop shared-instance interop, release 0.15.1 |
| `crates/apps/reticulumd/tests/python_channel_interop_parts/python_shared_instance_client_emit.rs` | release 0.15.1 |
| `crates/apps/reticulumd/tests/python_channel_interop_parts/python_shared_instance_live.rs` | Python-in-the-loop shared-instance interop, release 0.15.1 |
| `crates/apps/reticulumd/tests/support/python_channel_process.rs` | Python-in-the-loop shared-instance interop, release 0.15.1 |
| `crates/apps/reticulumd/tests/support/python_shared_instance_client.py` | Python-in-the-loop shared-instance interop, release 0.15.1 |
| `crates/apps/rns-tools/Cargo.toml` | release 0.15.1 |
| `crates/libs/rns-transport/CHANGELOG.md` | shared-instance client packaging, shared-instance client parity, release 0.15.1 |
| `crates/libs/rns-transport/Cargo.toml` | release 0.15.1 |
| `crates/libs/rns-transport/README.md` | shared-instance client packaging, shared-instance client parity, release 0.15.1 |
| `crates/libs/rns-transport/fuzz/Cargo.lock` | release 0.15.1 |
| `crates/libs/rns-transport/src/destination_parts/destination.rs` | shared-instance client parity |
| `crates/libs/rns-transport/src/iface/local.rs` | shared-instance client parity |
| `crates/libs/rns-transport/src/iface/tcp_client.rs` | shared-instance client parity |
| `crates/libs/rns-transport/src/resource/outcome.rs` | release 0.15.1 |
| `crates/libs/rns-transport/src/transport/announce_table.rs` | shared-instance client parity |
| `crates/libs/rns-transport/src/transport/core.rs` | shared-instance client packaging, shared-instance client parity |
| `crates/libs/rns-transport/src/transport/handler.rs` | shared-instance client packaging, shared-instance client parity |
| `crates/libs/rns-transport/src/transport/handler_parts/path_request.rs` | shared-instance client parity |
| `crates/libs/rns-transport/src/transport/inbound_processing.rs` | shared-instance client parity |
| `crates/libs/rns-transport/src/transport/inbound_processing_tests.rs` | shared-instance client parity |
| `crates/libs/rns-transport/src/transport/lifecycle.rs` | shared-instance client packaging, shared-instance client parity |
| `crates/libs/rns-transport/src/transport/link_table.rs` | shared-instance client parity |
| `crates/libs/rns-transport/src/transport/links_parts/transport.rs` | shared-instance client packaging, shared-instance client parity |
| `crates/libs/rns-transport/src/transport/links_parts/transport_sections/close_channel.rs` | shared-instance client parity |
| `crates/libs/rns-transport/src/transport/links_parts/transport_sections/shared_instance.rs` | shared-instance client packaging, shared-instance client parity, release 0.15.1 |
| `crates/libs/rns-transport/src/transport/links_parts/transport_sections/shared_instance_announce.rs` | shared-instance client parity |
| `crates/libs/rns-transport/src/transport/links_parts/transport_sections/shared_instance_reconnect.rs` | shared-instance client parity |
| `crates/libs/rns-transport/src/transport/mod.rs` | shared-instance client packaging, shared-instance client parity |
| `crates/libs/rns-transport/src/transport/packet_cache.rs` | shared-instance client parity |
| `crates/libs/rns-transport/src/transport/path_table.rs` | shared-instance client parity |
| `crates/libs/rns-transport/src/transport/path_table_codec.rs` | shared-instance client parity |
| `crates/libs/rns-transport/src/transport/request_dispatch.rs` | shared-instance client parity |
| `crates/libs/rns-transport/src/transport/shared_instance.rs` | shared-instance client packaging, shared-instance client parity |
| `crates/libs/rns-transport/src/transport/tests.rs` | shared-instance client packaging, shared-instance client parity, release 0.15.1 |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_link_packet_proven_probe2.rs` | proof cap stall on signal, shared-instance client packaging |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_isolation.rs` | shared-instance client packaging, shared-instance client parity |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_shared_instance.rs` | shared-instance client packaging, shared-instance client parity |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_shared_instance_endpoint_warn.rs` | release 0.15.1 |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_shared_instance_flush.rs` | shared-instance client parity |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_shared_instance_parity.rs` | shared-instance client parity |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_shared_instance_probe.rs` | shared-instance client packaging |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_shared_instance_probe2.rs` | shared-instance client packaging, shared-instance client parity |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_shared_instance_probe3.rs` | shared-instance client packaging, shared-instance client parity |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_shared_instance_probe4.rs` | shared-instance client packaging, shared-instance client parity |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_shared_instance_probe5.rs` | shared-instance client packaging, shared-instance client parity |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_shared_instance_probe6.rs` | shared-instance client packaging, shared-instance client parity |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_shared_instance_probe7.rs` | shared-instance client parity |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_shared_instance_probe8.rs` | shared-instance client parity |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_shared_instance_probe9.rs` | shared-instance client parity |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_shared_instance_probe10.rs` | shared-instance client parity |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_shared_instance_probe11.rs` | shared-instance client parity |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_shared_instance_probe12.rs` | release 0.15.1 |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_shared_instance_probe13.rs` | release 0.15.1 |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_shared_instance_probe14.rs` | release 0.15.1 |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_shared_instance_reconnect.rs` | shared-instance client parity |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_shared_instance_stop.rs` | shared-instance client parity |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_shared_instance_tasks.rs` | shared-instance client parity |
| `crates/libs/rns-transport/src/transport/tunnels.rs` | shared-instance client parity |
| `crates/libs/rns-transport/tests/shared_instance_api.rs` | release 0.15.1 |
| `docs/fork/PATCHSET.md` | shared-instance client packaging, shared-instance client parity, release 0.15.1 |
| `docs/getting-started.md` | release 0.15.1 |
| `tools/scripts/check-publish-tag-version.sh` | release 0.15.1 |

Fork commits `2a027a9d`, `b306b2a1`, `17b41d6a`, `ff8a8e8a`, `ff67110d`,
`e4494576`, `21e4dacc`, `a6036013`, `8a1c2d16`, `9247525e`, `afa72a00`,
`c05832b9`, `013d0dd2`, `b7df209e`, `6baf2e34`, `2e62274d`, `eba27ade`,
`a4fdb0bb`, `952d6ca9`, `18e2ccb5`, `52be26ad`, `428a699a`, `4b67a5e9`,
`5319bcde` and `114cd382`: F4, shared-instance client packaging. The first
lands `SharedInstanceEndpoint`
and `SharedInstanceOptions`, the four `Transport` methods
`attach_shared_instance`, `detach_shared_instance`,
`shared_instance` and `connected_to_shared_instance`, the `Transport::stop`
teardown of an attached shared instance, and moves
`local_hops_delta_for_packet` from `transport/mod.rs` to
`transport/handler.rs` to stay under the module-size cap; the second lands
the two-transport fixture; the third lands the README section, the release
notes and the contract-test registrations. The fourth gates the
`SharedInstanceEndpoint::Unix` variant, its `LocalUnixEndpoint` import and
its attach arm behind `cfg(unix)`, clears the shared-instance posture in
`Transport::stop` under the existing 5 s budget instead of an unbounded
handler lock, makes detach take the slot and clear the connected flag in
one handler-guarded critical section, warns when the tunnel cannot be
synthesized at attach, and corrects the Python cites in the rustdoc. The
fifth adopts the probe fixture (option default and builder, detach on
a never-attached transport, concurrent attaches admit one, attach before
the server listens connects once it does, detach silences the relay and
re-attach restores one-hop paths) and pins the shared instance's tunnel
table and the unix client's connect within the configured reconnect wait.
The sixth qualifies the README and release notes for unix targets, the
fixed TCP retry, the posture cites, the bool detach and the forced-off
flags and tunnel synthesis Python does not share, and pins the README
section as fence-free prose. The seventh corrects the probe fixture's
header and its late-server detach message. The eighth makes
`attach_shared_instance` set the connected posture under the handler lock
only while the slot still holds the new interface, so a detach or stop
running between the slot write and the posture write cannot leave the
posture set with the slot empty, and moves the posture clearing in
`Transport::stop` after the management-task join so a handler the task
still holds no longer eats the stop budget ahead of it. The ninth adopts
the second probe fixture (unix flavour end to end, TCP client keeps its
fixed retry under a short reconnect wait, stop with a wedged handler still
forgets the slot within budget, `stop_interface` leaves the slot held
until detach) and adds a probe that joins an attach with a detach on a
held slot five times and asserts only that the slot and the posture agree
afterwards. The tenth extends the release-notes cite check to the
`## Unreleased` section through two shared helpers, names the
`Tcp(SocketAddr)` endpoint and the slot-release rule in the README, and
rewords the release notes for the reconnect wait, the new pins and the
posture ordering. The eleventh makes the stop-budget warning name the
interface it left behind and fire only when a slot was actually held,
carries the endpoint kind in the attach debug line so a TCP trace does not
read as if the reconnect wait were in force, and documents on
`attach_shared_instance` and `stop` that reticulumd also re-synthesizes
the tunnel on reconnect and that an interface spawned after stop's sweep
but before it takes the slot stays registered until a second stop. The
twelfth adopts the third probe fixture (a detach queued on the handler
before the attach's posture write wins the slot and the posture stays off,
a stop racing an attach keeps the slot and the posture consistent,
attach/detach cycles leave no interface behind), gates the two probe2
helpers only the unix test calls behind `cfg(unix)`, guards the plain
empty-detach await with the stop timeout, and rewords the first probe's
race test as a refused attach racing a detach, since the slot is held
when its join starts. The thirteenth rewords the detach sentence in the
release notes, pins the third fixture there and records the gated warning
and the stop-vs-attach window. The fourteenth adopts the fourth fixture
(`remove_interface` leaves the slot held until detach, a second stop
detaches an interface spawned inside the first stop's window), bounds
every joined attach, detach or stop await in the earlier fixtures with the
stop timeout, makes their stall releases consistent, notes on the stall
helper's rustdoc that the shared-instance fixtures hold the handler with
it, and reuses the `LocalUnixEndpoint` import in the second fixture. The
fifteenth rewords the `stop` rustdoc so the repeated-stop sentence names
its exception, quiets the missing-interface warning at attach to a debug
line naming the racing detach or stop, labels the reconnect wait as in
force only for unix endpoints and says so when a detach finds a stale slot.
The sixteenth keeps the TCP attach line reading the configured reconnect
wait, which the TCP client ignores, so the options stay used when the
unix arm is compiled out. The seventeenth holds the `## Unreleased` ledger
section to the release-section contracts (files exist, rows name a legend
patch, legend hashes head a patch), requires an undated heading and a pin
clause per bullet in the Unreleased release notes, pins the
request-layer walker floor at its measured count and renames the README
fence test. The eighteenth cites
the fourth fixture, the two contract tests and the proof-cap stall in the
release notes. The nineteenth makes the Unreleased ledger checks fail on
a `## Unreleased` heading that lacks its `(upstream <base>)` suffix
instead of skipping. The twentieth writes the TCP attach line as
space-separated key=value like the unix arm, marking the reconnect wait
as ignored by the tcp client, names `stop_interface` among the racers in
the tunnel-miss debug line, starts the ledger header-hash walk at the
table so a legend sentence cannot vouch for its own hashes, and says on
the third and fourth fixtures' stop guards why they outlast the stop
timeout. The twenty-first extends the binary's one logger in
`two_transport_loopback_requests_isolation.rs` with a shared-instance
record store keyed by transport name and adopts the fifth fixture that
reads it (the attach never warns, the tcp line marks the reconnect wait
as ignored while the unix line reports it, a stale-slot detach and an
attach whose interface vanished between its sections log at debug). The
twenty-second admits the test-only fixture-hardening exception to the
release-notes listing rule, attributes the release notes and the ledger
section to the publish contract, and pins the fifth fixture and the
record store on the `Transport::stop` bullet. The twenty-third renames
that logger, its static and its install function for the two record
families they now serve. The twenty-fourth adopts the sixth fixture (the
attach line is space-separated key=value tokens and only the tcp arm
appends the ignored-wait parenthetical), scopes the never-warned oracle
and test logger rustdocs to the records they read, sharpens the publish
contract's cite message and the ledger header walk's missing-table
panic. The twenty-fifth pins the sixth fixture on the `Transport::stop`
bullet in the release notes. The file sites below describe the combined
result.
Upstream sites:
`crates/libs/rns-transport/src/transport/mod.rs` registers the
`shared_instance` module, re-exports its two types and gains the
`shared_instance` slot on `Transport`; `transport/core.rs` initialises
that slot; `transport/handler.rs` receives the moved
`local_hops_delta_for_packet`; `transport/lifecycle.rs` clears the slot
and the connected flag in `stop` after the management task has joined and
warns, naming the interface, only when a slot was held past the budget;
`links_parts/transport.rs` includes the new method section;
`transport/tests.rs` includes the seven fixtures;
`transport/tests_parts/two_transport_loopback_requests_isolation.rs`
keeps the shared-instance client's log records next to the link's drop
records;
`crates/apps/reticulumd/tests/fork_publish_contract.rs` scans the
`## Unreleased` release notes for file cites and checks the
`## Unreleased` ledger section. New files:
`src/transport/shared_instance.rs`,
`src/transport/links_parts/transport_sections/shared_instance.rs`,
`src/transport/tests_parts/two_transport_shared_instance.rs`,
`src/transport/tests_parts/two_transport_shared_instance_probe.rs`,
`src/transport/tests_parts/two_transport_shared_instance_probe2.rs`,
`src/transport/tests_parts/two_transport_shared_instance_probe3.rs`,
`src/transport/tests_parts/two_transport_shared_instance_probe4.rs`,
`src/transport/tests_parts/two_transport_shared_instance_probe5.rs` and
`src/transport/tests_parts/two_transport_shared_instance_probe6.rs`. The
crate `CHANGELOG.md` carries the release notes.

Fork commits `5af0c425`, `c8a296f3`, `60656cdb`, `54035599`, `9dfc446e`,
`8e20e0d4`, `4f41b2f2`, `c972552f`, `867264ef`, `cce0a328`, `0f581ef3`,
`41064bcb`, `6089ea85`, `a9ce646f`, `98f4073f`, `2b44522c`, `8a33660f`,
`f3613fba`, `fbfa0589`, `1bb9b2cc`, `47cf9313`, `05337466`, `3239bf09`,
`ef3827a6`, `f38a8b81`, `abf09730`, `7eeaafb0`, `e790cb55` and
`6ca33e3f`: F5,
shared-instance client parity.
The first gives `TcpClient` a configurable
reconnect wait, `with_reconnect_wait` and `reconnect_wait` with the default
kept at 5 s, used at both backoff sleeps of its spawn loop, and pins it
with a fast-retry twin of the reconnect-storm test. The second adds the
disconnect-event seam to both stream clients, `with_disconnect_events` on
`TcpClient` and `LocalUnixClient`: a sender, sibling of
`with_reconnect_events`, that fires once per established stream that ends
while the client is neither cancelled nor stopped, before the reconnect
loop runs again; connect failures and detach or stop fire nothing. The
third makes the TCP attach arm pass the configured reconnect wait to the
client, so the attach debug line reports the wait as in force for both
endpoints and the options rustdoc no longer calls it unix-only, and
updates the three log and retry pins (the tcp attach line ends at the wait;
a TCP client attached with a 50 ms wait reaches a late-bound shared
instance within 2 s). The fourth announces a destination registered on an
attached client as a path response 0.25 s later
(`RNS/Transport.py:2911-2916`), adds
`SingleInputDestination::announce_with_context` for the path-response
context (`RNS/Destination.py:308-315`) and
`Transport::announce_destinations_to_shared_instance`, which re-announces
every registered destination the same way, and adopts the parity fixture.
The fifth wires both clients' reconnect events into a per-attach consumer
task that re-synthesizes the tunnel at once and, `reconnect_wait` plus two
seconds later, announces every registered single destination as a path
response while the transport is still attached
(`RNS/Interfaces/LocalInterface.py:178-181`, `RNS/Transport.py:3705-3709`);
`Transport::synthesize_tunnel_on_interface` now delegates to a free
function that takes the handler and the iface_manager guards one after
the other, and the reconnect fixture restarts the shared instance on the
same port. The sixth serves the disconnect events in the same consumer: a
dropped stream, while the transport is connected and the interface is
still flagged as the shared instance, closes every in and out link with
the reason of its own side, drops their resource state and empties the
path, announce, link and tunnel tables plus the held announces
(`RNS/Transport.py:3690-3702`, `RNS/Interfaces/LocalInterface.py:255-261`);
`PathTable`, `AnnounceTable`, `LinkTable` and `TunnelTable` gain `clear`,
the msgpack decoders duplicated in `tunnels.rs` move to the path table
codec, and the fixture adds the flushed path, the closed-link event, the
surviving attach and the three no-flush cases (detach, dropped posture,
unflagged interface). The seventh returns early from
`filter_duplicate_packet` while attached, so nothing is inserted into the
local packet cache and nothing rolled back (`RNS/Transport.py:1618-1627`).
The eighth biases the consumer's select so a queued disconnect is served
before the reconnect behind it, and drives the consumer directly to pin
the order. The ninth refuses `attach_shared_instance` once `stop` has
cancelled the transport, logs the slot `stop` clears at debug under the
links target, parks the stop of the fourth probe fixture on a held worker
so the attach lands inside the sweep-to-cancel window for certain, pins
the refused racing attach in the third, adds
`two_transport_shared_instance_stop.rs` for the refused attach and the
cleared-slot line, and makes the test logger keep every Warn-or-above
record on the links target so the never-warned oracle covers any warning
naming the transport. The tenth rewrites the README's
attach section for the behaviour above, trims the divergence register to
the two claims that still hold, and loops the reconnect-order test over
twelve rounds. The eleventh gives `Transport` a private `JoinSet` that the
registration announce, the per-attach reconnect consumer and the delayed
re-announce are spawned through, reaped of finished tasks before each
spawn, and makes `stop` take the set out of its std guard right after the
cancel and join it under the existing budget, aborting and warning only on
expiry, pinned by `two_transport_shared_instance_tasks.rs`. The twelfth
adds `PacketCache::clear_reverse_routes`, the twin of Python's
`reverse_table` flush that empties the proof-destination index and the
source notes while keeping the duplicate-filter hashes, calls it from the
drop handler in its own scoped lock after the handler block, takes the
links out of both maps under the first handler guard before closing them
so a link opened on the `Closed` event survives the clears, corrects the
two docstrings that placed `reverse_table` on the link-table entries, and
adds `two_transport_shared_instance_flush.rs`, which seeds every table and
asserts each empty on its own after the drop. The thirteenth pins the
widened logger with a needle-less warning, adds a release-docs contract
test that the README's shared-instance section cites the five Python
ranges and calls nothing pending, puts `await_holding_invalid_type` on
every async test the parity commits added, runs the reconnect-order test
over twenty rounds and adopts the seventh probe fixture (the registration
announce waits its quarter second, the re-announce waits `reconnect_wait`
plus two seconds, a posture dropped after the reconnect re-announces
nothing, every restart re-announces again, a pending link is torn down,
and the `TcpClient` wait defaults to five seconds). The fourteenth moves
the cites to `LocalInterface.py:160`, `:178-181` and
`Destination.py:308-315` as checked at ea98db4f, narrows the hashlist
claim in the README, the duplicate-filter comment and the release notes to
the duplicate filter alone, says the tunnel is synthesized at attach and
on every reconnect, names the transport in the re-announce warning,
clamps `LocalUnixClient::with_reconnect_wait` to 1 ms like the TCP knob,
and refreshes two fixture comments. The fifteenth narrows
`clear_reverse_routes` to the index entries of received packets, so the
entry of a packet this node sent, its receipt guard, survives the drop as
Python's `receipts` list does, and has the flush fixture send data from
the client before the drop and assert that entry kept. The sixteenth adds
the two new section files to the release-docs contract's fork-new list,
cites `Transport.py:3697-3702` on the three `clear` methods that lacked
one, has `request_dispatch::send_on_iface` name the announce broadcast as
the other sanctioned guard-holding transmit seam, and puts the
`await_holding_invalid_type` deny on the registration announce spawn. The
seventeenth moves the two task warnings of stop's join loop into helpers
on the links target and threads the transport's name into
`spawn_shared_instance_task`, so every shared-instance task warning reads
`tp(<name>)` on the links target (stop's own budget warning for a handler
that stays locked keeps its `[tp]` prefix on the lifecycle target) and the
never-warned oracle of the tasks fixture catches a join that regresses to
an abort. The eighteenth narrows the
duplicate-filter claim: the filter neither inserts nor consults while
attached, but the source notes `handle_data` leaves share the cache and
count as seen once detached, now a divergence entry against Python's
separate `packet_hashlist`; it also says a posture set directly gets the
registration announce but not the reconnect consumer's work, adopts the
eighth probe fixture (the receipt guard survives a drop as the consumer
sees it), moves the unix client's clamp sentence to Changed, says when
`with_disconnect_events` fires for a pre-supplied stream, and derives the
fixtures' timing bounds from the waits they are built on. The nineteenth
adopts the ninth probe fixture (a posture set directly announces on
registration, a stop that runs out of budget on a pending announce warns
as `tp(<name>)` on the links target, a posture switched off announces
nothing, and a directly set posture neither flushes nor re-announces on a
restart), pins in `inbound_processing_tests.rs` that a Data packet
received while attached counts as seen once detached, and pins in both
stream clients that a client built from a pre-supplied stream reports its
disconnect once and finishes without reconnecting. The twentieth biases
the registration announce select toward its cancel, counts the link table
entries the drop flush clears on the disconnected debug line, and says
the tunnel re-synthesis is not posture gated. The twenty-first bounds the
seventh probe's registration announce from above, derives its two floors
from the production delay and the fixture's reconnect wait, and adds the
unflagged-interface twin of its dropped-posture test. The twenty-second
tidies the rustdoc and comments: the join-failed warning sees only a
panic, `LinkTable::clear` cites `Transport.py:3700`, stop's join no
longer claims every task saw the cancel, `with_reconnect_wait` notes the
clamp and the TCP client's immediate first redial, the two unguarded
packet-cache updates name the receipt guard, and the README carries the
clamp and the two fork-only collections the flush empties. The
twenty-third has the drop handler tear each drained link down with
`Link::teardown` instead of `close`, as `shared_connection_disappeared`
does (`RNS/Transport.py:3692`, `:3695`, `RNS/Link.py:657-668`): the
LINKCLOSE packet of every active link is sent on the interface the link
rides through `request_dispatch::send_on_iface` once every guard is
released, a packet that does not go out is logged at debug, the
disconnected line counts the ones sent, and the flush fixture gains a
third transport over plain TCP whose link sees `Closed` with the
initiator-side reason when the shared instance drops. The twenty-fourth
makes `LocalUnixClient` sleep `reconnect_wait` before every redial after
an established stream drops, selected against cancel and stop, as
Python's reconnect loop sleeps before every attempt
(`RNS/Interfaces/LocalInterface.py:159-160`), while a client built from a
pre-supplied stream still finishes at once, and adopts the tenth probe
fixture (the TCP clamp reads 1 ms, the TCP client redials at once after
the first drop and waits from the second, the unix client's re-accept
lands no sooner than the wait, the drop line counts the link table
entries, a posture dropped before the restart still re-synthesizes the
tunnel). In this workspace the change reaches reticulumd's unix
`LocalClientInterface` bootstrap
(`bootstrap_interface_startup_parts::module_prelude`), which builds
`LocalUnixClient::new_connect` without `with_reconnect_wait` and so
redials 5 s (`DEFAULT_RECONNECT_WAIT`) after a shared-instance drop rather
than at once. The twenty-fifth pins that a shared-instance task that ended by
panic is warned about once as `tp(<name>)` while stop completes. The
twenty-sixth carries the three into the README and the release notes. The
twenty-seventh adds the eleventh probe fixture (the drop's LINKCLOSE
reaches the initiator of a link the client accepted, a `stop` inside the
unix redial wait completes at once, a from-stream unix client ends at once
when its stream drops). The twenty-eighth registers the Handshake teardown
without a LINKCLOSE and the TCP client's immediate redial as divergences,
names `MIN_STABLE_CONNECTION` in the redial wording of the README, the
release notes and the `with_reconnect_wait` rustdoc, and notes in the
`on_disconnected` rustdoc why a teardown without an interface is a no-op.
The twenty-ninth pins the teardown and redial cites in the README
contract. The file sites below describe the combined result.
Upstream sites: `src/iface/tcp_client.rs` and `src/iface/local.rs` carry
the reconnect wait, its 1 ms clamp and the disconnect seam;
`src/destination_parts/destination.rs` gains `announce_with_context`;
`transport/mod.rs` gains the shared-instance task set on `Transport` and
`transport/core.rs` initialises it and delegates its tunnel synthesis to
the free function; `transport/inbound_processing.rs` returns early while
attached and `transport/inbound_processing_tests.rs` pins it;
`transport/lifecycle.rs` logs the slot `stop` clears and joins the task
set; `transport/packet_cache.rs` gains `clear_reverse_routes`;
`transport/announce_table.rs`, `transport/link_table.rs`,
`transport/path_table.rs` and `transport/tunnels.rs` gain `clear` and
`transport/request_dispatch.rs` names the second transmit seam,
`transport/handler.rs` and `transport/handler_parts/path_request.rs` mark
their packet-cache updates as the receipt guard, with
`transport/path_table_codec.rs` exposing the shared decoders;
`links_parts/transport.rs` includes the two new sections;
`links_parts/transport_sections/close_channel.rs` spawns the registration
announce from `register_destination`;
`links_parts/transport_sections/shared_instance.rs` wires the seams and
refuses the post-stop attach; `transport/shared_instance.rs` drops the
unix-only wording and notes the clamp; `transport/tests.rs` includes the ten new fixtures;
`transport/tests_parts/two_transport_loopback_requests_isolation.rs`
keeps every links-target warning; the shared-instance fixture and probe
fixtures two to six follow the new attach line, stop window and oracle;
`crates/apps/reticulumd/tests/fork_release_docs_contract.rs` holds the
README's shared-instance section to its eight parity cites, walks the two
new section files for cites and holds the request-layer walker to its
measured floor.
New files:
`src/transport/links_parts/transport_sections/shared_instance_announce.rs`,
`src/transport/links_parts/transport_sections/shared_instance_reconnect.rs`,
`src/transport/tests_parts/two_transport_shared_instance_parity.rs`,
`src/transport/tests_parts/two_transport_shared_instance_reconnect.rs`,
`src/transport/tests_parts/two_transport_shared_instance_stop.rs`,
`src/transport/tests_parts/two_transport_shared_instance_tasks.rs`,
`src/transport/tests_parts/two_transport_shared_instance_flush.rs`,
`src/transport/tests_parts/two_transport_shared_instance_probe7.rs`,
`src/transport/tests_parts/two_transport_shared_instance_probe8.rs`,
`src/transport/tests_parts/two_transport_shared_instance_probe9.rs`,
`src/transport/tests_parts/two_transport_shared_instance_probe10.rs` and
`src/transport/tests_parts/two_transport_shared_instance_probe11.rs`.
The README and the crate `CHANGELOG.md` carry the prose.

Fork commit `cccc40e4`: proof cap stall on signal. The cap fixture of
`crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_link_packet_proven_probe2.rs`
stalled the listener's handler for a fixed 1500 ms while the connector
sent its 1027 plain Data packets; under full-suite load the burst outran
the stall, the listener proved early hashes before the cap could drop
them, and the proven set no longer matched the newest 1024. The stall is
now held on a blocking thread until the test releases it after the burst
(`stall_handler_until_released` holds the guard, spawned by
`stall_transport_until_released`, both defined in that file and reused by
the third, fourth and fifth shared-instance fixtures), so its length
no longer depends on runner speed. The pinned counts and hashes are
unchanged. Upstream site: that fixture file only.

Fork commit `eceeb9ad`: Python-in-the-loop shared-instance interop. A
Python `rnsd` (RNS 1.5.2) runs as the shared instance on free ports, the
library attaches through `Transport::attach_shared_instance`, and a second
Python local client of the same `rnsd` exchanges announces and
`Link.request` calls with the Rust transport in both directions. The new
fixture
`crates/apps/reticulumd/tests/python_channel_interop_parts/python_shared_instance_live.rs`,
included from `crates/apps/reticulumd/tests/python_channel_interop.rs`,
drains the `rnsd` log on background threads so it gates on the readiness
line and on the line that files the Rust announce under a
`LocalInterface`, and dumps both child logs on failure; the abstract
AF_UNIX variant is Linux only, since Python's `use_af_unix()` is false
elsewhere. The new
`crates/apps/reticulumd/tests/support/python_shared_instance_client.py`
registers an echoing request handler, reports the Rust announce, sends one
`Link.request` to the Rust destination and then keeps announcing its own
destination until stdin closes so the Rust side can run the reverse
direction, one JSON object per stdout line.
`crates/apps/reticulumd/tests/support/python_channel_process.rs` gains
`spawn_rnsd` and `spawn_shared_instance_client`. Upstream site: test
files only.

Fork commits `e529730c`, `84d90f75`, `9b2e20a6`, `0df584b1`, `b4a941d8`,
`59b66d8e`, `257c180d`, `8559e395`, `6fed34de`, `dffe6f9b`, `134140fb`,
`df914502`, `692d9327`, `e5e3bc88`, `d632c5e9`, `4fd3a8dc`, `4994a8f1`,
`cfb0f62f`, `220a4794` and `fd10554b`: release 0.15.1. The first pins a shared-instance
usage example in the crate README (`crates/libs/rns-transport/README.md`):
the `## Attaching to a shared instance` section gains one rust fence
showing attach, destination plus request handler, the on-demand announce
and detach, which the new `crates/libs/rns-transport/tests/shared_instance_api.rs`
runs verbatim against an in-process loopback `TcpServer` shared instance
and reads the announce back as a path response at one hop;
`crates/apps/reticulumd/tests/fork_release_docs_contract.rs` adds that
file to the pin list, raises the snippet floor to five and requires
exactly one rust fence in the section, pinned by that test. The second
makes `attach_shared_instance`
(`crates/libs/rns-transport/src/transport/links_parts/transport_sections/shared_instance.rs`)
warn once under the links target, naming the transport and the address,
when a `Tcp` endpoint's address is not loopback, before any lock is taken;
the attach proceeds unchanged, the rustdoc, the README section and the
divergence register say so, and the fixture
`crates/libs/rns-transport/src/transport/tests_parts/two_transport_shared_instance_endpoint_warn.rs`,
included from `crates/libs/rns-transport/src/transport/tests.rs`, pins
one warning for `10.255.255.1:1` and a quiet detach and stop. The third
makes `ResourceManager::conclude_outbound`
(`crates/libs/rns-transport/src/resource/outcome.rs`) log a debug line
naming the resource hash and the link when the `OutboundOutcome` receiver
was dropped before the verdict, in place of an ignored send. The fourth
adds the fork publish and release-docs contract tests and the
breaking-change compile-fail harness to the contracts job of
`.github/workflows/ci-full.yml`, as the pull-request lane already ran
them. The fifth makes `tools/scripts/check-publish-tag-version.sh` strip
a leading `refs/tags/` from `PUBLISH_REF` before the prefix test, so a
caller passing `github.ref` is checked rather than let through; the
mismatched-tag test in
`crates/apps/reticulumd/tests/fork_publish_contract.rs` covers the
full-ref accept and reject cases and the README release section names
the accepted form. The sixth adds the release notes for the endpoint
warning and the outcome fix to the crate `CHANGELOG.md`. The seventh
bumps the crate version to 0.15.1 at every manifest, consumer-snippet and
contract-test site, regenerates `Cargo.lock` and
`crates/libs/rns-transport/fuzz/Cargo.lock`, promotes `## Unreleased` to
`## 0.15.1 - 2026-10-21` in the crate `CHANGELOG.md` with a version bullet
and no `### Breaking` subsection, and adds the 0.15.1 paragraph to the
README patch list. The eighth adds
`crates/libs/rns-transport/src/transport/tests_parts/two_transport_shared_instance_probe12.rs`,
included from `crates/libs/rns-transport/src/transport/tests.rs`, pinning
the warning's boundary (`[::1]` never warns, `[::ffff:10.255.255.1]` warns
once, an attach after a detach warns once per attach), and a test in
`crates/apps/reticulumd/tests/fork_publish_contract.rs` that the
`contracts` job of `.github/workflows/ci-full.yml` carries every `cargo
test` step the one in `ci.yml` runs. The ninth moves the warning in
`crates/libs/rns-transport/src/transport/links_parts/transport_sections/shared_instance.rs`
after the first critical section, so an attach refused by the slot or the
cancel warns nothing, and reads the address through
`IpAddr::to_canonical`, so `[::ffff:127.0.0.1]` is quiet; the endpoint
warn fixture gains the refused-attach pin and the rustdoc says the warning
is per accepted attach. The tenth makes the live test's announce oracle a
genuine rebroadcast:
`crates/apps/reticulumd/tests/python_channel_interop_parts/python_shared_instance_live.rs`
announces again once the Python client's ready line is seen, after waiting
out the first announce's emitted second, and
`crates/apps/reticulumd/tests/support/python_shared_instance_client.py`
drops `receive_path_responses` from its handler so `announce_received`
fires only on rnsd's live re-send to its other local clients
(`RNS/Transport.py:2398-2430`); the client also takes `--app-name`,
`--aspect`, `--request-path` and `--request-data`, passed by
`spawn_shared_instance_client` in
`crates/apps/reticulumd/tests/support/python_channel_process.rs` from the
Rust constants, and refuses to build the link when the recalled identity
under that name does not hash to the argv hash; the same commit renames
the rnsd config string, names the `LOG_PATHING` floor on the loglevel
line and joins the log tail's drain threads in `Drop`. The eleventh makes
the full-ref accept case in
`crates/apps/reticulumd/tests/fork_publish_contract.rs` assert the skip
line is absent and adds the `refs/tags/` strip line to the token pin. The
twelfth rewords `crates/libs/rns-transport/tests/shared_instance_api.rs`
and the README section so the one-hop reading is attributed to the plain
`TcpServer`, with rnsd and reticulumd reading zero
(`RNS/Transport.py:1938`), leaving the pinned snippet lines untouched. The
thirteenth applies review nits: the CHANGELOG parenthetical that still
named the Unreleased checks, a README paragraph break before the endpoint
warning, the divergence register's sentence naming itself plus this file
as the parity ledger for fork-only rows, and the undelivered verdict on
the debug line in `crates/libs/rns-transport/src/resource/outcome.rs`.
The fourteenth adds the release notes for the warning placement, the
live-rebroadcast oracle and the ci-full twin pin to the crate
`CHANGELOG.md`. The fifteenth registers the live test's destination before
the attach, so the registration announce a destination added under the
posture gets 250 ms later cannot land in the second after the explicit
early announce and move rnsd's timebase past the second announce
(`RNS/Transport.py:2238`), and takes the guard instant after the early
announce returns so it bounds the emission time from above.
The sixteenth adds
`crates/libs/rns-transport/src/transport/tests_parts/two_transport_shared_instance_probe13.rs`,
included from `crates/libs/rns-transport/src/transport/tests.rs`, pinning
the two edges the warning's rustdoc states: `[::ffff:127.0.0.1]` never
warns, and an attach refused after stop returns `InvalidArgument` and
warns nothing even for a non-loopback endpoint. The seventeenth makes
`emit` in `crates/apps/reticulumd/tests/support/python_shared_instance_client.py`
write under `RNS.logging_lock` (`RNS/__init__.py:89`, held by `RNS.log` at
`:129-133`) so a report cannot interleave with an RNS log line, adds the
zero-hop assertion on the Python client's announce report to
`crates/apps/reticulumd/tests/python_channel_interop_parts/python_shared_instance_live.rs`,
adds `RNS/Interfaces/LocalInterface.py:109` and `RNS/Transport.py:1938` to
the README cite list in `crates/apps/reticulumd/tests/fork_release_docs_contract.rs`,
and makes this section's closing sentence plural. The eighteenth adopts
`crates/apps/reticulumd/tests/python_channel_interop_parts/python_shared_instance_client_emit.rs`,
included from `crates/apps/reticulumd/tests/python_channel_interop.rs`,
which imports the Python client helper as a module through
`run_shared_instance_client_helper_script` in
`crates/apps/reticulumd/tests/support/python_channel_process.rs` and races
`emit` against four `RNS.log` threads for a second, asserting every stdout
line is a whole log line or a whole report. The nineteenth adds the
`iface=<hash>` field to the non-loopback endpoint warning in
`crates/libs/rns-transport/src/transport/links_parts/transport_sections/shared_instance.rs`,
as its sibling lines carry, and rewords
`crates/libs/rns-transport/src/transport/tests_parts/two_transport_shared_instance_endpoint_warn.rs`
to say the detach and the stop add no warning on the links target; makes
the `--app-name`, `--aspect`, `--request-path` and `--request-data` arguments of
`crates/apps/reticulumd/tests/support/python_shared_instance_client.py`
required and names the mismatch report's hash `derived`; pins the
tag-guard mismatch cases in `crates/apps/reticulumd/tests/fork_publish_contract.rs`
to exit 1; and drops the redundant port wait in
`crates/apps/reticulumd/tests/python_channel_interop_parts/python_shared_instance_live.rs`,
which followed rnsd's "Started shared instance interface" line, logged
after the listener is already bound.
The twentieth adopts
`crates/libs/rns-transport/src/transport/tests_parts/two_transport_shared_instance_probe14.rs`,
included from `crates/libs/rns-transport/src/transport/tests.rs`, pinning
the `iface=<hash>` token the nineteenth added: it is the interface the
attach returned, whole, placed before the `endpoint <addr>` clause, equal
to the token on the attach debug line, and a second attach after a detach
names its own interface; the same commit makes the header of
`crates/apps/reticulumd/tests/python_channel_interop_parts/python_shared_instance_client_emit.rs`
state the measured glued-line rate of the unlocked `emit` as under one per cent.
The ledger-only commits that complete this section are
omitted by convention.

## 0.15.0 (upstream 0.14.0)

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
`Python-in-the-loop request and proof interop` patch carries no other id
and is fork commits `60e74ffb`, `6e583b50`, `ea6edfaa`, `9223ef43`,
`6e9eedf2`, `3f615a3b`, `28bacbbe`, `dbd4ff7e`, `a0ba6f4a`, `b5dd3255`,
`e4eb444f` and `fdb20280`; the `release 0.15.0` patch carries no other id
and is fork commits `d4a3b2d2`, `5075d765`, `eb0e19f2`, `06068516`,
`ac9c49af`, `8b3ce9e1`, `23bdc1a8`, `ba92e2ba`, `8d70f3e1`, `ccfba204`,
`af1b7e62`, `061141f9`, `0bbccb83`, `eb854ab6`, `c5048353`, `93f417c6`,
`2d2d88e1`, `2432cff5`, `e2a1a28a`, `77024043`, `f326b062` and `44c98d93`.

| File | Patch |
| --- | --- |
| `.github/workflows/ci.yml` | release 0.15.0 |
| `.github/workflows/codex-review.yml` | release 0.15.0 |
| `Cargo.lock` | release 0.15.0 |
| `Cargo.toml` | release 0.15.0 |
| `crates/apps/reticulumd/Cargo.toml` | release 0.15.0 |
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
| `crates/apps/reticulumd/tests/fork_publish_contract.rs` | release 0.15.0 |
| `crates/apps/reticulumd/tests/fork_release_docs_contract.rs` | release 0.15.0 |
| `crates/apps/reticulumd/tests/fuzz_campaign_contract.rs` | request wire frames and fuzz |
| `crates/apps/reticulumd/tests/python_channel_interop.rs` | Python-in-the-loop request and proof interop |
| `crates/apps/reticulumd/tests/python_channel_interop_parts/ax25_kiss_pty.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/apps/reticulumd/tests/python_channel_interop_parts/ifac_kiss_pty.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/apps/reticulumd/tests/python_channel_interop_parts/ifac_python_interop.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/apps/reticulumd/tests/python_channel_interop_parts/module_prelude.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/apps/reticulumd/tests/python_channel_interop_parts/multi_segment_python_interop.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/apps/reticulumd/tests/python_channel_interop_parts/python_request_edges.rs` | Python-in-the-loop request and proof interop |
| `crates/apps/reticulumd/tests/python_channel_interop_parts/python_request_forms_matrix.rs` | Python-in-the-loop request and proof interop |
| `crates/apps/reticulumd/tests/python_channel_interop_parts/python_request_late_reply.rs` | Python-in-the-loop request and proof interop |
| `crates/apps/reticulumd/tests/python_channel_interop_parts/python_request_size_limits.rs` | Python-in-the-loop request and proof interop |
| `crates/apps/reticulumd/tests/python_channel_interop_parts/python_request_unknown_path.rs` | Python-in-the-loop request and proof interop |
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
| `crates/apps/rns-tools/Cargo.toml` | release 0.15.0 |
| `crates/apps/rns-tools/src/bin/independent_interop_node/events.rs` | link proof event and receipt link id, outbound outcomes with link fast fail and accept hook |
| `crates/apps/rns-tools/src/bin/independent_interop_node/node.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/apps/rns-tools/src/bin/rncp_parts/client.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/apps/rns-tools/src/bin/rncp_parts/server.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/apps/rns-tools/src/bin/rngit_parts/native_client.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/apps/rns-tools/src/bin/rnsh_parts/network.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/libs/lxmf-runtime/src/link_delivery.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/libs/lxmf-runtime/src/tests.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/libs/lxmf-runtime/src/tests_link_delivery.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/CHANGELOG.md` | request wire frames and fuzz, request handlers and dispatch, request receipts and timeouts, link proof event and receipt link id, outbound outcomes with link fast fail and accept hook, Python-in-the-loop request and proof interop, release 0.15.0 |
| `crates/libs/rns-transport/Cargo.toml` | release 0.15.0 |
| `crates/libs/rns-transport/README.md` | request handlers and dispatch, request receipts and timeouts, link proof event and receipt link id, outbound outcomes with link fast fail and accept hook, release 0.15.0 |
| `crates/libs/rns-transport/fuzz/.gitignore` | request wire frames and fuzz |
| `crates/libs/rns-transport/fuzz/Cargo.lock` | request wire frames and fuzz, release 0.15.0 |
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
| `crates/libs/rns-transport/src/destination/request/allow.rs` | request handlers and dispatch, release 0.15.0 |
| `crates/libs/rns-transport/src/destination/request/dispatch.rs` | request handlers and dispatch |
| `crates/libs/rns-transport/src/destination/request/frame.rs` | request wire frames and fuzz, release 0.15.0 |
| `crates/libs/rns-transport/src/destination/request/handler.rs` | request handlers and dispatch, release 0.15.0 |
| `crates/libs/rns-transport/src/destination/request/id.rs` | request wire frames and fuzz, release 0.15.0 |
| `crates/libs/rns-transport/src/destination/request/inbound.rs` | request handlers and dispatch, release 0.15.0 |
| `crates/libs/rns-transport/src/destination/request/mod.rs` | request wire frames and fuzz, request handlers and dispatch, request receipts and timeouts, link proof event and receipt link id |
| `crates/libs/rns-transport/src/destination/request/receipt.rs` | request receipts and timeouts, release 0.15.0 |
| `crates/libs/rns-transport/src/destination/request/receipt_order_tests.rs` | link proof event and receipt link id |
| `crates/libs/rns-transport/src/destination/request/receipt_pending.rs` | request receipts and timeouts |
| `crates/libs/rns-transport/src/destination/request/receipt_tests.rs` | request receipts and timeouts, link proof event and receipt link id |
| `crates/libs/rns-transport/src/destination/request/tests.rs` | request wire frames and fuzz |
| `crates/libs/rns-transport/src/destination/tests.rs` | request handlers and dispatch |
| `crates/libs/rns-transport/src/destination_parts/destination.rs` | request handlers and dispatch |
| `crates/libs/rns-transport/src/destination_parts/module_core.rs` | request handlers and dispatch |
| `crates/libs/rns-transport/src/destination_parts/module_prelude.rs` | request handlers and dispatch |
| `crates/libs/rns-transport/src/error.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/resource.rs` | request receipts and timeouts, outbound outcomes with link fast fail and accept hook, release 0.15.0 |
| `crates/libs/rns-transport/src/resource/manager.rs` | request receipts and timeouts, outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/resource/manager_cleanup.rs` | request receipts and timeouts, outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/resource/manager_polling.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/resource/manager_proof.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/resource/manager_segments.rs` | request receipts and timeouts, outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/resource/manager_start.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/resource/outcome.rs` | outbound outcomes with link fast fail and accept hook, release 0.15.0 |
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
| `crates/libs/rns-transport/src/transport/mod.rs` | request handlers and dispatch, request receipts and timeouts, link proof event and receipt link id, outbound outcomes with link fast fail and accept hook, release 0.15.0 |
| `crates/libs/rns-transport/src/transport/path.rs` | request handlers and dispatch |
| `crates/libs/rns-transport/src/transport/request_dispatch.rs` | request handlers and dispatch, outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/transport/resource_accept_hook.rs` | outbound outcomes with link fast fail and accept hook, release 0.15.0 |
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
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_link_packet_proven_probe2.rs` | link proof event and receipt link id, release 0.15.0 |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_link_packet_proven_probe3.rs` | link proof event and receipt link id |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests.rs` | request handlers and dispatch, request receipts and timeouts |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_errors.rs` | request receipts and timeouts, outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_isolation.rs` | request handlers and dispatch |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_malformed.rs` | request handlers and dispatch |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_probe.rs` | request handlers and dispatch, request receipts and timeouts |
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_receipt.rs` | request receipts and timeouts, release 0.15.0 |
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
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_resource_form_sizes.rs` | request receipts and timeouts, outbound outcomes with link fast fail and accept hook, release 0.15.0 |
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
| `crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_requests_resource_form_sizes_probe15.rs` | request receipts and timeouts, release 0.15.0 |
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
| `crates/libs/rns-transport/tests/breaking_changes_compile_fail.rs` | release 0.15.0 |
| `crates/libs/rns-transport/tests/link_event_packet_proven_api.rs` | link proof event and receipt link id |
| `crates/libs/rns-transport/tests/outbound_outcome_api.rs` | outbound outcomes with link fast fail and accept hook |
| `crates/libs/rns-transport/tests/request_frame_wire.rs` | request wire frames and fuzz |
| `crates/libs/rns-transport/tests/request_handler_api.rs` | request handlers and dispatch |
| `crates/libs/rns-transport/tests/request_receipt_api.rs` | request receipts and timeouts, release 0.15.0 |
| `crates/libs/rns-transport/tests/request_receipt_readme.rs` | request receipts and timeouts, outbound outcomes with link fast fail and accept hook |
| `docs/fork/PATCHSET.md` | request wire frames and fuzz, request handlers and dispatch, request receipts and timeouts, link proof event and receipt link id, outbound outcomes with link fast fail and accept hook, Python-in-the-loop request and proof interop, release 0.15.0 |
| `docs/getting-started.md` | release 0.15.0 |
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
typed reason rather than to the variant alone; `resource_faults.rs` gains
the top-of-file `use rns_transport::resource::RejectReason;` for it.

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

Fork commit `28bacbbe`: new fixture
`python_channel_interop_parts/python_request_unknown_path.rs` (237 lines,
three ignored tests, one `include!` line in `python_channel_interop.rs`)
requests a path the responder never registered, in both directions.
Python's `Link.handle_request` does nothing when the path hash is not in
`destination.request_handlers` (`RNS/Link.py:810`), not even a log line,
and the Rust `request-dispatch` worker drops it with a debug log and no
reply, so the requester observes only the absence of a response.
`python_to_rust_request_handler_unregistered_path_is_dropped_and_destination_keeps_serving`
has the Python client request `/test/missing` in packet form against a
Rust destination serving `/test/request` under `RequestAllow::All`: the
handler never runs, the client reports `request_no_response` with status
`SENT`, no client line carries a `response_size`, and a second client on
`/test/request` is still answered.
`rust_to_python_request_receipt_unregistered_path_packet_request_times_out_undelivered`
sends a packet-form request for `/test/missing` under a 3 s
`RequestOptions::with_timeout`: the receipt's status trail is exactly
`[Sent, Failed]`, `response()` is `Err(RequestError::Timeout)`, the
Python endpoint prints no `request_received` and `resource_events()`
reports nothing.
`rust_to_python_request_receipt_unregistered_path_resource_request_is_delivered_then_times_out`
sends a 48 KiB request Resource: Python accepts it on size alone because
the destination has a handler registered (`RNS/Link.py:1036-1040`), so
the receipt reaches `Delivered` first; the path-hash miss at `:810` then
leaves it unanswered and the timeout fails it. The trail is exactly
`[Sent, Delivered, Failed]`, the elapsed time is at least the timeout
because the request clock starts at delivery, and exactly one
`OutboundComplete` is observed. The fixture reuses
`RequestHandlerFixture`, `RustRequestReceiptFixture` and the matrix
helpers of its sibling fixtures. The Python endpoint gains
`--request-path` (default `/test/request`), threaded from the
`python_channel_endpoint.py` argparse through
`ChannelClient.run(request_path=...)` into
`python_channel_request_forms.run_client(..., request_path)` and used as
the `Link.request` path. Every async fn but the lock-taking test fns
carries `#[deny(clippy::await_holding_invalid_type)]`.

Fork commit `dbd4ff7e`: the wire-form pins tightened. In
`python_request_forms_matrix.rs` the two Resource-request with
Resource-response cells assert the exact terminal-form order,
`[InboundRequest, Outbound]` Python to Rust and `[Outbound,
InboundResponse]` Rust to Python, where they checked `contains` before;
the MDU guard moves into `request_forms_matrix_assert_large_exceeds_mdu`,
called by all three Rust-to-Python Resource cells and by the Resource
test of `python_request_unknown_path.rs`, each logging `rust link mdu=`;
`..._none_response_leaves_the_packet_request_unanswered` logs its id
pair before comparing, and the concurrent test logs the Python id
unquoted. `python_to_rust_request_handler.rs` defines
`REQUEST_HANDLER_PY_SENT = 0x01` and `REQUEST_HANDLER_PY_FAILED = 0x00`
(`RNS/Link.py:1318-1319`), used by all three fixture files: the matrix
file's own twins are removed and the silent-denial test's literal `1`
replaced. Both `python_channel_endpoint.py` and
`python_channel_request_forms.py` start with `from __future__ import
annotations`, without which their `float | None` annotations need
Python 3.10 or later; `run_client` computes `request_deadline =
time.time() + max(timeout, request_timeout or 0.0)` so a
`--request-timeout` longer than `--timeout` cannot cut the wait short of
RNS's `failed_callback`.

Fork commit `a0ba6f4a`: new fixture
`python_channel_interop_parts/python_request_size_limits.rs` (240 lines,
three ignored tests, one `include!` line in `python_channel_interop.rs`)
pins what a consumer sees when one side's size policy refuses the other
side's Resource, and the Resource-form `None` answer.
`python_to_rust_request_handler_resource_request_over_max_request_size_is_rejected_promptly`
sets `SingleInputDestination::set_max_request_size(1024)` on the Rust
destination, below a 48 KiB Python request Resource: Rust refuses the
advertisement with a receiver cancel, so the Python sender's Resource
ends `REJECTED` and `request_resource_concluded` fails the receipt at
once through `failed_callback` (`RNS/Link.py:1367-1385`), the client
reporting `request_failed` with status 0 in well under its 25 s request
timeout; the handler never runs, no `InboundRequest` or `Outbound`
terminal form is observed, and a second in-limit packet-form client is
still answered.
`rust_to_python_request_receipt_resource_response_over_max_response_size_fails_oversize`
sends `RequestOptions::with_max_response_size(1024)`, below the Python
handler's 48 KiB Resource reply: the receipt fails `Oversize` from the
advertisement size gate well before its 15 s timeout, Python's
`Resource.reject` path is benign, no `InboundResponse` terminal form is
observed, and a following in-limit request on the same link concludes
`Ready`.
`rust_to_python_request_receipt_resource_request_none_response_is_delivered_then_times_out`
sends a 48 KiB request Resource the Python handler answers with `None`
(`RNS/Link.py:844` sends nothing): the status trail is exactly `[Sent,
Delivered, Failed]`, `response()` is `Err(RequestError::Timeout)`,
exactly one `OutboundComplete` is observed, and the Python report shows
the handler ran once. The fixture reuses `RequestHandlerFixture`,
`RequestHandlerRecord`, `RustRequestReceiptFixture`,
`request_unknown_path_status_trail` and the matrix helpers of its
sibling fixtures. Every async fn but the lock-taking test fns carries
`#[deny(clippy::await_holding_invalid_type)]`.

Fork commit `b5dd3255`: the Resource-form timeout anchored at delivery,
and the Resource-form unknown path pinned on the Rust responder. In
`python_request_unknown_path.rs` (236 to 311 lines)
`request_unknown_path_status_trail` records a `tokio::time::Instant`
beside every status it observes, projected by
`request_unknown_path_statuses` and `request_unknown_path_instant_of`;
the two Resource-form timeout cells,
`rust_to_python_request_receipt_unregistered_path_resource_request_is_delivered_then_times_out`
and the size-limit fixture's
`..._resource_request_none_response_is_delivered_then_times_out`
(`python_request_size_limits.rs`, 240 to 242 lines), assert
`failed_at.duration_since(delivered_at) >= timeout` for their 3 s
timeout in place of an elapsed-since-send check, which a clock anchored
at the send would also have passed; the Delivered-to-Failed interval
only covers the timeout when the clock starts at delivery. New test
`python_to_rust_request_handler_unregistered_path_resource_request_fails_on_python`
has the Python client request `/test/missing` as a 48 KiB Resource
against a Rust destination serving `/test/request` under
`RequestAllow::All`: Rust accepts the request Resource on size alone as
Python does at `RNS/Link.py:1036-1040` and assembles it, the
`request-dispatch` worker drops it on the path-hash miss, the handler
never runs, the terminal forms from `resource_events()` are exactly
`[InboundRequest]`, and Python's Resource-form receipt reaches
`DELIVERED` (`:1372`) so `failed_callback` fires (`:1398`): the client
reports `request_failed` with status 0 and `request_form` `resource`.
In `python_channel_endpoint.py` the two `request-forms` call sites
(`ChannelClient.run` into `python_channel_request_forms.run_client`, and
`main` into `ChannelClient.run`) pass `request_size`, `identify`,
`request_timeout` and `request_path` (the first also `timeout` and
`deadline`) as keyword arguments. Every async fn but the lock-taking
test fns carries `#[deny(clippy::await_holding_invalid_type)]`.

Fork commit `e4eb444f`: new fixture
`python_channel_interop_parts/python_request_late_reply.rs` (325 lines,
four ignored tests, one `include!` line in `python_channel_interop.rs`)
pins a reply that arrives after the requester gave up, the Resource-form
`None` answer on the Rust responder, and `PROVE_ALL` beside a request on
one link.
`python_to_rust_request_handler_none_response_leaves_the_resource_request_delivered_then_failed`
has the Python client send a 48 KiB request Resource to a Rust
destination whose `RequestAllow::All` handler returns `None`: Rust
accepts the Resource on size alone (`RNS/Link.py:1036-1040`), runs the
handler exactly once with the full body and sends nothing (the `:844`
twin), so Python's receipt reaches `DELIVERED` (`:1372`) and
`request_timed_out` fails it (`:1398`); the client reports
`request_failed` with status 0 and no `response_size`, the terminal
forms are exactly `[InboundRequest]`, and the id the handler saw equals
the id Python logged.
`rust_to_python_request_receipt_late_packet_response_after_timeout_is_ignored_and_link_serves_next`
sends a packet-form request under a 2 s `RequestOptions::with_timeout`
that the Python handler answers after 5 s: the status trail is exactly
`[Sent, Failed]` and `response()` is `Err(RequestError::Timeout)` before
the reply arrives, the late packet reply revives nothing, a following
request on the same link concludes `Ready` with its own body under a
fresh id, and no Resource terminal form is observed.
`rust_to_python_request_receipt_late_resource_response_after_timeout_is_dropped_unsolicited`
is the same with a 48 KiB late reply: the response advertisements,
counted on the interface tap, do reach Rust and are dropped as
unsolicited, so the reply is not received (no `InboundResponse`
terminal form), and a following request still concludes `Ready`.
`rust_to_python_request_receipt_under_prove_all_only_the_plain_data_packet_is_proven`
runs a request round trip on a `--prove-all` link and observes no
`PacketProven`, Python proving only context-`None` Data
(`RNS/Link.py:950`, `:961`), while a plain Data packet sent on the same
link is proven. The `serve_request` handler in
`python_channel_request_forms.py` takes a `delay` key: a dict request
carrying it sleeps that many seconds before answering, on the handler's
own daemon thread (`RNS/Link.py:892-893`, `:1004-1005`), so the link
keeps receiving while the Rust timeout runs out. The fixture reuses
`RequestHandlerFixture`, `RequestHandlerRecord`,
`RustRequestReceiptFixture` and the matrix, unknown-path and receipt
helpers of its sibling fixtures. Every async fn but the lock-taking test
fns carries `#[deny(clippy::await_holding_invalid_type)]`.

Fork commit `fdb20280`: new fixture
`python_channel_interop_parts/python_request_edges.rs` (373 lines, five
ignored tests, one `include!` line in `python_channel_interop.rs`) pins
the close path, the envelope types and the file response against Python,
plus the packet-form request over `max_request_size`.
`rust_to_python_request_receipt_python_teardown_fails_the_pending_request_link_closed`
has the Python handler hold its answer and tear the link down a second
later (`Link.teardown` on the inbound link from
`Transport.active_links_map`, `RNS/Transport.py:2934-2937`): the status
trail is exactly `[Sent, Failed]`, `response()` is
`Err(RequestError::LinkClosed)` well inside a 30 s timeout,
`LinkEvent::Closed` follows on the Rust link and Python reports the
teardown.
`rust_to_python_request_receipt_dict_reply_packet_is_read_as_a_map` and
`..._dict_reply_resource_is_read_as_a_map` have Python answer a dict of
one member per umsgpack type family (str, int, bool, list, bytes), bare
and padded to 48 KiB: `value()` is a `Value::Map` in insertion order,
`data()` and `metadata()` are `None`, the map re-packed through
`rmpv::encode::write_value` hashes to the bytes Python packed, and the
terminal forms are `[]` for the packet and `[InboundResponse]` for the
Resource.
`rust_to_python_request_receipt_file_reply_is_read_through_data_and_metadata`
has Python answer a 64 byte `(BufferedReader, metadata)` tuple
(`RNS/Link.py:835-845`): the reply is a response Resource even below the
link MDU, `data()` is the file body, `metadata()` the string Python sent,
`value()` and `into_value()` are `None`.
`python_to_rust_request_handler_packet_request_over_max_request_size_is_dropped_silently`
sends an 8000 byte packet-form request at a destination whose
`max_request_size` is 1024: the handler never runs, nothing is answered,
the Python receipt stays `SENT` with `request_no_response`, no Resource
terminal form is observed and a fresh client is still served. The
`serve_request` handler in `python_channel_request_forms.py` (200 lines)
gains `response: map`, `response: file` (`file_size`) and
`teardown_after` keys and now takes the link id from the endpoint's
`_on_request`; it reports `request_received` before the `delay` sleep
and a separate `request_answered` line after it, and the `delay` comment
cites both daemon-thread sites (`RNS/Link.py:892-893`, `:1004-1005`).
`DEFAULT_REQUEST_PATH` in the same module replaces the five
`/test/request` literals in `python_channel_endpoint.py`. In
`python_request_late_reply.rs` the two Rust-to-Python late tests wait on
`request_answered` through a new `late_reply_wait_answered` helper
before asserting that the late reply changed nothing, the Resource cell
asserts `failed_after >= LATE_REPLY_RUST_TIMEOUT` as its packet twin
does, and the `PROVE_ALL` comment names the in-crate loopback tests as
the pin of the Rust record gate. Every async fn but the lock-taking test
fns carries `#[deny(clippy::await_holding_invalid_type)]`.

Fork commits `d4a3b2d2`, `5075d765`, `eb0e19f2`, `06068516`, `ac9c49af`,
`8b3ce9e1`, `23bdc1a8`, `ba92e2ba`, `8d70f3e1`, `ccfba204`, `af1b7e62`,
`061141f9`, `0bbccb83`, `eb854ab6`, `c5048353`, `93f417c6`, `2d2d88e1`,
`2432cff5`, `e2a1a28a`, `77024043`, `f326b062` and `44c98d93`:
release 0.15.0. The first gates the inherited Codex review workflow
(`.github/workflows/codex-review.yml`) on the repository variable
`CODEX_REVIEW_ENABLED`, so the check is skipped until the variable and the
`OPENAI_API_KEY` secret exist, adds the `fork_publish_contract` test to the
contracts job of the PR-path CI (`.github/workflows/ci.yml`), and makes the
old-name walk in `crates/apps/reticulumd/tests/fork_publish_contract.rs`
skip dot-directories other than `.github`. The second adds or extends the
doc comments on the public request-layer and outcome items so each cites
its Python counterpart or states that it is fork-only: `frame.rs`,
`handler.rs`, `id.rs`, `inbound.rs` and `receipt.rs` under
`crates/libs/rns-transport/src/destination/request/`,
`crates/libs/rns-transport/src/resource/outcome.rs`,
`crates/libs/rns-transport/src/transport/mod.rs` and
`crates/libs/rns-transport/src/transport/resource_accept_hook.rs`. The
third consolidates the request-layer sections of the crate README into one
guide and adds its "Known divergences from Python RNS" section. The fourth
bumps the crate version at every manifest, consumer-snippet and
contract-test site, regenerates `Cargo.lock`, promotes `## Unreleased` to
`## 0.15.0 - 2026-10-20` in the crate `CHANGELOG.md` with a `### Breaking`
subsection, rewrites the README patch list for 0.15.0, and in
`fork_publish_contract.rs` derives the legend patch ids from the legend
instead of hard-coding `B1` to `B5`, accepts any interop fixture row
attributed to a Python-in-the-loop patch, and resolves release-notes cites
under `crates/apps/rns-tools/src` as well. The fifth refits the two doc
comments the second added to `crates/libs/rns-transport/src/transport/mod.rs`
so the file stays within the 500-line module budget. The sixth settles the
README version policy after the first minor bump (the fork takes the first
version above both the upstream number and its own last published one),
qualifies the opener's upstream-compatibility promise with the `### Breaking`
list, and aligns the `started_at()`, timeout and register wording. The
seventh adopts the release-review probe tests: five in
`crates/apps/reticulumd/tests/fork_publish_contract.rs` (the divergence
register cites Python lines, a minor bump opens with `### Breaking`, PR CI
runs the publish contract with the Codex review opt-in, README snippets are
pinned verbatim, documented `#[non_exhaustive]` enums carry the attribute)
and one in `crates/libs/rns-transport/tests/request_receipt_api.rs` (the
request-layer enums and `AnnounceEvent` hold the wildcard-arm contract
outside the crate). The eighth extends the doc-comment cites of the second
to every public method and enum variant of the request layer (`allow.rs`,
`frame.rs`, `handler.rs`, `id.rs`, `inbound.rs` and `receipt.rs`); the
ninth reaches the remaining variants and fields (`RequestError`,
`FrameDecodeError`, `RejectReason`, `OutboundFailure`, `OutboundResult` in
`receipt.rs`, `frame.rs` and `crates/libs/rns-transport/src/resource/outcome.rs`,
the `OutboundRejected` reason field in `crates/libs/rns-transport/src/resource.rs`).
The tenth regenerates `crates/libs/rns-transport/fuzz/Cargo.lock` to 0.15.0,
which the release had left at 0.14.1, pins both locks in
`release_sites_and_ledgers_track_the_crate_version`, names the second lock in
the README rebase recipe, adopts the probe's
`request_layer_public_items_cite_python_or_say_fork_only` contract test,
lets the breaking-changes test skip a leading `## Unreleased` stub, widens
the out-of-crate wildcard-arm test to `FrameDecodeError`, and names the
request-layer enums as `#[non_exhaustive]` in the `CHANGELOG.md`. The
eleventh moves the release-documentation contracts (rustdoc cites, the
divergence register, the release-notes shape, the README snippet pins and the
`#[non_exhaustive]` sites) out of the full `fork_publish_contract.rs` into
`crates/apps/reticulumd/tests/fork_release_docs_contract.rs`, run by the same
CI step (`.github/workflows/ci.yml`) and README recipe line; the cite
contract now walks public fields, holds every variant to its own cite, and
checks by name the new items that share a file with upstream code. It also
adopts the probe's two lock tests (the recipe regenerates both locks; the
fuzz lock resolves the crate from its path dependency), adds
`FrameDecodeError` to the `#[non_exhaustive]` sites, cites the RCL on the
`OutboundRejected` variant doc in `crates/libs/rns-transport/src/resource.rs`,
and names both contract files in the `CHANGELOG.md` version bullet. The
twelfth adds `fork_new_public_structs_keep_their_fields_private` to
`crates/apps/reticulumd/tests/fork_release_docs_contract.rs`: the nine
public structs the fork introduced in the request and outcome files are
pinned by name and keep their fields private, `DeliveryReceipt::link_id` is
private, and `AnnounceEvent::is_path_response` is the one public fork field,
on a struct that carries `#[non_exhaustive]`; in the same file the
breaking-changes test reads each `- ` bullet with its indented continuation
lines, and the divergence-register test reuses the rustdoc contract's
`cites_python_line` predicate. The thirteenth scopes the README divergence
register (`crates/libs/rns-transport/README.md`) to the divergences the fork
introduced and registers the first inherited one a fork change surfaced: the
sender spends its retry budget in `Transferring`, where Python waits a single
RTT-scaled `max_wait` (`RNS/Resource.py:643-650`). The doc comments of
`OutboundFailure::RetriesExhausted` and `OutboundOutcome` in
`crates/libs/rns-transport/src/resource/outcome.rs`, of `MAX_DEPTH` in
`request/frame.rs` and of `RequestError` and its `Timeout` variant in
`request/receipt.rs` gain the matching cites; the 0.15.0 notes in the crate
`CHANGELOG.md` drop their pre-release phrasing and name `FrameDecodeError` as
`#[non_exhaustive]`, and the README Release hygiene bullet names
`fork_release_docs_contract`. The fourteenth hoists the two release locks of
`crates/apps/reticulumd/tests/fork_publish_contract.rs` into a
`RELEASE_LOCKS` constant that the version pin, the recipe check and the
file-table check all iterate, so the fuzz lock is a required row of the
table above. The fifteenth renames the contracts step of
`.github/workflows/ci.yml` to "Fork publish and release-docs contracts" and
makes the `.github/workflows/codex-review.yml` job also require a pull
request head from this repository, since a fork head cannot read the
`OPENAI_API_KEY` secret. The sixteenth iterates the required rows of the
file-table check in `fork_publish_contract.rs` by reference, so the
workspace clippy run stays clean. The seventeenth adds
`patch_set_ledger_release_legend_hashes_each_have_a_patch_header` to
`fork_publish_contract.rs`: every commit the legend attributes to the
release patch must appear in a patch header of the same section, which is
the rule the twelfth to fifteenth commits above had slipped. The eighteenth
holds the CI hygiene test to the same-repository head clause of
`.github/workflows/codex-review.yml` (whose comment, and the README release
hygiene bullet, now say fork-originated pull requests are skipped), requires
every manifest, consumer snippet and README the version bump touches as a
row of the table above, teaches the rustdoc cite walker in
`fork_release_docs_contract.rs` to enter `pub trait` bodies and check their
methods, and cites the whole file-response branch (`RNS/Link.py:837-846`)
on the `RequestResponse` enum doc in `request/handler.rs`. The nineteenth
adds `crates/libs/rns-transport/tests/breaking_changes_compile_fail.rs`,
which drives `rustc` against the `librns_transport` rlib the test binary
linked (no `trybuild`, no nested cargo): the migrated consumer the crate
`CHANGELOG.md` `### Breaking` block describes compiles, and each shape it
replaces (an `AnnounceEvent` struct literal, a `DeliveryReceipt`,
`RequestOptions`, `RequestReceipt` or `RequestFrame` struct literal, an
`OutboundRejected` unit pattern, a `PathHash` tuple literal) fails with
rustc's diagnostic for that reason; the contracts job of
`.github/workflows/ci.yml` runs it as its own step. The twentieth makes
the README divergence register name the Python Reticulum repository and
commit (`ea98db4f`, the ref `.github/workflows/verify.yml` checks out)
its cites resolve against, ends the `### Breaking` block of the crate
`CHANGELOG.md` with a clause naming the compile-fail harness, starts the
`RequestResponse` response-form cite there at `RNS/Link.py:837`, and points
two doc comments in `tests/request_receipt_api.rs` at the harness. The
twenty-first makes the cap fixture of
`crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_link_packet_proven_probe2.rs`
collect its 1024 `PacketProven` events and 1027 receipts on progress: each
wait resets its deadline when a new proof or receipt arrives and fails only
when none lands within `LOOPBACK_TIMEOUT` of the previous one, after the
ubuntu nextest lane of the release pull request delivered 854 proofs inside
the fixed budget; the pinned counts are unchanged, and the crate
`CHANGELOG.md` says so under the `PacketProven` pin clause. The
twenty-second holds the responder of the three fixtures that assert a
resource-form receipt passes `Delivered`
(`two_transport_loopback_requests_receipt.rs`,
`two_transport_loopback_requests_resource_form_sizes.rs` and
`two_transport_loopback_requests_resource_form_sizes_probe15.rs`) on a
`Notify` the status subscriber releases once it has read `Delivered`, in
place of a 300 ms handler sleep, after the same lane read the response
advertisement's `Receiving` in its place on the receipt's `watch`; no
assertion is loosened, and the exact-order fixtures are unchanged. The
ledger-only commits that complete this section are omitted by convention.
The patches of the `## 0.14.1` section below are still carried; the patches
above build on their files rather than replacing them, and the publish
contract test they added is extended by the first and fourth commits above.

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
