include!("tests_parts/module_prelude.rs");

include!("tests_parts/path_table_api.rs");

include!("tests_parts/reticulum_path_restore_bad_cache.rs");

include!("tests_parts/unchanged_path_cache.rs");

include!("tests_parts/blackholed_path_restore.rs");

include!("tests_parts/expired_route_restart_recovery.rs");

include!("tests_parts/path_request_duplicate_scoping.rs");

include!("tests_parts/unknown_path_request_answered_by_announce.rs");

include!("tests_parts/pending_out_link_rediscovery.rs");
include!("tests_parts/pending_out_link_establishment_timeout.rs");
include!("tests_parts/receipt_handler_on_shared_transport.rs");

include!("tests_parts/tunnel_restore_freshness.rs");

include!("tests_parts/held_udp_announce_preserves_peer_sou.rs");

include!("tests_parts/roaming_path_response_suppression.rs");

include!("tests_parts/announce_identity_drift.rs");

include!("tests_parts/announce_broadcast_policy.rs");
include!("tests_parts/announce_table_retransmission_gate.rs");
include!("tests_parts/pinned_python_local_announce_schedule.rs");
include!("tests_parts/disabled_transport_shared_child_local_destination.rs");
include!("tests_parts/enabled_transport_shared_child_local_destination.rs");
include!("tests_parts/disabled_shared_child_local_announce.rs");
include!("tests_parts/enabled_shared_child_local_announce.rs");
include!("tests_parts/shared_child_remote_link_request.rs");
include!("tests_parts/shared_owner_unknown_link_request.rs");

include!("tests_parts/encrypted_resource_control_packet.rs");

include!("tests_parts/transport_register_channel_handler_d.rs");

include!("tests_parts/inbound_proof_link_selection.rs");

include!("tests_parts/transport_register_channel_handler_e.rs");

include!("tests_parts/unicast_iface_for_source_returns_non.rs");

include!("tests_parts/inbound_link_request_registers_unicast.rs");

include!("tests_parts/blackhole_path_eviction.rs");

include!("tests_parts/rns_1_5_ingress_admission.rs");
include!("tests_parts/ordinary_proof_duplicate.rs");
include!("tests_parts/plain_group_packet_duplicates.rs");
include!("tests_parts/duplicate_filter_python_matrix.rs");
include!("tests_parts/packet_replay_restart.rs");
include!("tests_parts/accepted_child_ifac_rotation.rs");

include!("tests_parts/reticulum_runtime_management.rs");

include!("tests_parts/packet_proof_correlation.rs");

include!("tests_parts/single_destination_delivery_proof.rs");

include!("tests_parts/link_broadcast_helpers_route_via_bound_iface.rs");

include!("tests_parts/two_transport_loopback.rs");

include!("tests_parts/two_transport_loopback_resources.rs");

include!("tests_parts/two_transport_loopback_resources_directions.rs");

include!("tests_parts/resource_retry_link_lookup.rs");

include!("tests_parts/two_transport_loopback_resources_retries.rs");

include!("tests_parts/two_transport_loopback_resources_receipt_less_receiving.rs");

include!("tests_parts/two_transport_loopback_resources_gone_link.rs");

include!("tests_parts/two_transport_loopback_resources_reject.rs");

include!("tests_parts/two_transport_loopback_resources_reject_response.rs");

include!("tests_parts/two_transport_loopback_lifecycle.rs");

include!("tests_parts/two_transport_loopback_lifecycle_probe.rs");

include!("tests_parts/two_transport_loopback_requests.rs");

include!("tests_parts/two_transport_loopback_requests_isolation.rs");

include!("tests_parts/two_transport_loopback_requests_probe.rs");

include!("tests_parts/two_transport_loopback_requests_routing.rs");

include!("tests_parts/two_transport_loopback_requests_malformed.rs");

include!("tests_parts/two_transport_loopback_requests_resource_form.rs");

include!("tests_parts/two_transport_loopback_requests_resource_form_probe.rs");

include!("tests_parts/two_transport_loopback_requests_resource_form_probe2.rs");

include!("tests_parts/two_transport_loopback_requests_resource_form_probe3.rs");

include!("tests_parts/two_transport_loopback_requests_receipt.rs");

include!("tests_parts/two_transport_loopback_requests_receipt_timeouts.rs");

include!("tests_parts/two_transport_loopback_requests_receipt_probe.rs");

include!("tests_parts/two_transport_loopback_requests_receipt_probe2.rs");

include!("tests_parts/two_transport_loopback_requests_receipt_probe3.rs");

include!("tests_parts/two_transport_loopback_requests_receipt_stop.rs");

include!("tests_parts/two_transport_loopback_requests_receipt_probe4.rs");

include!("tests_parts/two_transport_loopback_requests_receipt_probe5.rs");

include!("tests_parts/two_transport_loopback_requests_receipt_probe6.rs");

include!("tests_parts/two_transport_loopback_requests_receipt_probe7.rs");

include!("tests_parts/two_transport_loopback_requests_resource_form_sizes.rs");

include!("tests_parts/two_transport_loopback_requests_errors.rs");

include!("tests_parts/two_transport_loopback_requests_resource_form_sizes_probe.rs");

include!("tests_parts/two_transport_loopback_requests_resource_form_sizes_cancel.rs");

include!("tests_parts/two_transport_loopback_requests_resource_form_sizes_probe2.rs");

include!("tests_parts/two_transport_loopback_requests_resource_form_sizes_cancel_probe.rs");

include!("tests_parts/two_transport_loopback_requests_resource_form_sizes_probe3.rs");

include!("tests_parts/two_transport_loopback_requests_resource_form_sizes_probe4.rs");

include!("tests_parts/two_transport_loopback_requests_resource_form_sizes_probe5.rs");

include!("tests_parts/two_transport_loopback_requests_resource_form_sizes_probe6.rs");

include!("tests_parts/two_transport_loopback_requests_resource_form_sizes_probe7.rs");

include!("tests_parts/two_transport_loopback_requests_resource_form_sizes_cancel2.rs");

include!("tests_parts/two_transport_loopback_requests_resource_form_sizes_probe8.rs");

include!("tests_parts/two_transport_loopback_requests_resource_form_sizes_probe9.rs");

include!("tests_parts/two_transport_loopback_requests_resource_form_sizes_probe10.rs");

include!("tests_parts/two_transport_loopback_requests_resource_form_sizes_probe11.rs");

include!("tests_parts/two_transport_loopback_requests_resource_form_sizes_probe12.rs");

include!("tests_parts/two_transport_loopback_requests_resource_form_sizes_probe13.rs");

include!("tests_parts/two_transport_loopback_requests_resource_form_sizes_probe14.rs");

include!("tests_parts/two_transport_loopback_requests_resource_form_sizes_probe15.rs");

include!("tests_parts/two_transport_loopback_requests_resource_form_sizes_probe16.rs");

include!("tests_parts/two_transport_loopback_link_packet_proven.rs");

include!("tests_parts/two_transport_loopback_requests_receipt_orders.rs");

include!("tests_parts/two_transport_loopback_link_packet_proven_probe.rs");

include!("tests_parts/two_transport_loopback_link_packet_proven_probe2.rs");

include!("tests_parts/two_transport_loopback_link_packet_proven_probe3.rs");

include!("tests_parts/two_transport_loopback_resources_outcome.rs");

include!("tests_parts/link_no_path_fast_fail.rs");

include!("tests_parts/two_transport_loopback_announce_path_response.rs");

include!("tests_parts/two_transport_loopback_resources_accept_hook.rs");

include!("tests_parts/two_transport_loopback_resources_outcome_stop.rs");

include!("tests_parts/two_transport_loopback_resources_outcome_probe.rs");
