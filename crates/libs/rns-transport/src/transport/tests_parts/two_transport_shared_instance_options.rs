// Pins the fork-only `SharedInstanceOptions` builders beyond `with_reconnect_wait`: each
// getter reads back what its builder stored, the defaults are the client's own, and the
// interface mode and the MTU reach the interface the attach registers with the iface
// manager. The forced bitrate lives inside the spawned client, where the manager cannot
// read it, so it is pinned at the getter only. Helpers come from the sibling `tests_parts`
// files included into this module; nothing answers on port 1 of the loopback address.

use crate::iface::InterfaceMode as OptionsInterfaceMode;

/// The defaults leave every client setting to the client and refuse a remote endpoint;
/// each builder stores what it is given, and the options are a plain value the builders
/// chain on.
#[test]
fn shared_instance_options_builders_store_what_their_getters_read_back() {
    let defaults = SharedInstanceOptions::default();
    assert!(!defaults.remote_endpoint_allowed(), "a remote endpoint is refused by default");
    assert_eq!(defaults.mtu(), None, "the MTU defaults to the client's own");
    assert_eq!(defaults.forced_bitrate_bps(), None, "no bitrate is forced by default");
    assert_eq!(defaults.interface_mode(), OptionsInterfaceMode::Full, "the mode defaults to Full");

    let options = SharedInstanceOptions::default()
        .allow_remote_endpoint(true)
        .with_mtu(1_024)
        .with_forced_bitrate(9_600)
        .with_interface_mode(OptionsInterfaceMode::Gateway);
    assert!(options.remote_endpoint_allowed(), "the opt-in is stored");
    assert_eq!(options.mtu(), Some(1_024), "the MTU is stored");
    assert_eq!(options.forced_bitrate_bps(), Some(9_600), "the forced bitrate is stored");
    assert_eq!(options.interface_mode(), OptionsInterfaceMode::Gateway, "the mode is stored");
    assert!(
        !options.allow_remote_endpoint(false).remote_endpoint_allowed(),
        "the opt-in can be withdrawn again"
    );
}

/// The mode and the MTU the options carry are the ones the iface manager registers the
/// attached interface under; a default attach registers `Full` and the TCP client's own
/// MTU, as before the options existed.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_tcp_attach_registers_the_interface_mode_and_mtu_from_the_options() {
    install_test_logger();
    let name = "shared-instance-options-mode-mtu";
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let transport = Transport::new(TransportConfig::new(name, &identity, false));
    let iface = transport
        .attach_shared_instance(
            SharedInstanceEndpoint::Tcp(SHARED_INSTANCE_V4_LOOPBACK_ENDPOINT),
            SharedInstanceOptions::default()
                .with_interface_mode(OptionsInterfaceMode::Gateway)
                .with_mtu(1_024),
        )
        .await
        .expect("a loopback attach is accepted");
    shared_instance_assert_attached(&transport, iface).await;
    let (mode, mtu) = {
        let manager = transport.iface_manager();
        let manager = manager.lock().await;
        (manager.mode(&iface), manager.mtu(&iface))
    };
    assert_eq!(mode, Some(OptionsInterfaceMode::Gateway), "the mode reaches the manager");
    assert_eq!(mtu, Some(1_024), "the MTU reaches the manager");
    assert!(timeout(STOP_TIMEOUT, transport.detach_shared_instance()).await.expect("detach"));
    shared_instance_assert_released(&transport, iface).await;

    let iface = transport
        .attach_shared_instance(
            SharedInstanceEndpoint::Tcp(SHARED_INSTANCE_V4_LOOPBACK_ENDPOINT),
            SharedInstanceOptions::default(),
        )
        .await
        .expect("a loopback attach is accepted");
    let (mode, mtu) = {
        let manager = transport.iface_manager();
        let manager = manager.lock().await;
        (manager.mode(&iface), manager.mtu(&iface))
    };
    assert_eq!(mode, Some(OptionsInterfaceMode::Full), "the default mode is Full");
    assert_eq!(mtu, Some(TcpClient::DEFAULT_MTU), "the default MTU is the client's own");
    assert!(timeout(STOP_TIMEOUT, transport.detach_shared_instance()).await.expect("detach"));
    shared_instance_assert_released(&transport, iface).await;
    timeout(STOP_TIMEOUT, transport.stop()).await.expect("stop completes");
    shared_instance_assert_never_warned(name);
}
