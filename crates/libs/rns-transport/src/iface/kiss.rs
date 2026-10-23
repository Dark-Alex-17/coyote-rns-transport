include!("kiss_parts/module_prelude.rs");

include!("kiss_parts/flush_pending_kiss.rs");

include!("kiss_parts/kisstcpclientinterface.rs");

#[cfg(test)]
#[path = "kiss_parts/ifac_runtime_tests.rs"]
mod ifac_runtime_tests;
#[cfg(test)]
#[path = "kiss_parts/shared_receiver_lock_span_probe_tests.rs"]
mod shared_receiver_lock_span_probe_tests;
#[cfg(test)]
#[path = "kiss_parts/tx_queue_survives_reconnect_tests.rs"]
mod tx_queue_survives_reconnect_tests;
