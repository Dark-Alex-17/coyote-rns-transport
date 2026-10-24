include!("bootstrap_interface_startup_parts/module_prelude.rs");

include!("bootstrap_interface_startup_parts/shared_instance_attach.rs");

#[cfg(test)]
#[path = "bootstrap_interface_startup_parts/shared_instance_attach_tests.rs"]
mod shared_instance_attach_tests;

include!("bootstrap_interface_startup_parts/startup_udp.rs");

#[cfg(all(test, unix))]
#[path = "bootstrap_interface_startup_parts/auto_activation_tests.rs"]
mod auto_activation_tests;

include!("bootstrap_interface_startup_parts/startup_vrn76_kiss_ble.rs");
