// Pins `shared_instance_options_for`, the one place a config entry becomes the
// `SharedInstanceOptions` the attach hands the library: the MTU and the forced bitrate are
// read back through the options' public getters. The mode builder has no getter; that half
// is pinned by the attach-backed `local_client_attach_registers_the_mode_and_mtu_from_the_entry`
// in the parent module, which goes through the same function.

use super::*;

/// An entry without `mtu` asks for the TCP client's default, and without
/// `force_shared_instance_bitrate` asks for no forced bitrate at all; the config normaliser
/// fills `mtu` for the local kinds, so the fallback is exercised on a bare entry.
#[test]
fn options_for_a_bare_entry_take_the_client_default_mtu_and_no_forced_bitrate() {
    let bare = InterfaceConfig { kind: "local_client".to_string(), ..InterfaceConfig::default() };
    assert_eq!(bare.mtu, None);
    assert_eq!(bare.force_shared_instance_bitrate, None);

    let options = shared_instance_options_for(&bare);
    assert_eq!(options.mtu(), Some(TcpClient::DEFAULT_MTU), "the TCP client's default MTU");
    assert_eq!(options.forced_bitrate_bps(), None, "no bitrate is forced unless the entry asks");
}

/// Both knobs reach the options when the entry carries them, by the values the entry gave.
#[test]
fn options_for_an_entry_with_the_knobs_carry_its_mtu_and_forced_bitrate() {
    let config = DaemonConfig::from_toml(
        r#"
interfaces = [
  { type = "LocalClientInterface", enabled = true, name = "knobbed", shared_instance_type = "tcp", shared_instance_port = 37428, mtu = 600, force_shared_instance_bitrate = 9600 }
]
"#,
    )
    .expect("local_client with an mtu and a forced bitrate");
    let iface = &config.interfaces[0];
    assert_eq!(iface.mtu, Some(600));
    assert_eq!(iface.force_shared_instance_bitrate, Some(9600));

    let options = shared_instance_options_for(iface);
    assert_eq!(options.mtu(), Some(600), "the entry's mtu reaches the options");
    assert_eq!(options.forced_bitrate_bps(), Some(9600), "the entry's bitrate reaches the options");
}
