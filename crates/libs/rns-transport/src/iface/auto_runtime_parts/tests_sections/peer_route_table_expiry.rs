#[tokio::test]
async fn expired_peer_is_rerouted_to_a_fresh_virtual_iface() {
    let mut manager = InterfaceManager::new(8);
    let host = manager.new_channel(8).address;
    let expired_iface = manager
        .register_virtual_iface(host, IfaceRole::VirtualUnicast)
        .expect("register expiring virtual peer");
    let manager = Arc::new(tokio::sync::Mutex::new(manager));
    let socket =
        Arc::new(tokio::net::UdpSocket::bind("127.0.0.1:0").await.expect("bind peer socket"));
    let peer_addr: SocketAddr = "192.0.2.10:9735".parse().expect("parse peer addr");
    let bridge = AutoInterfaceTransportBridge {
        host_iface: host,
        iface_manager: Arc::clone(&manager),
        rx_channel: tokio::sync::mpsc::channel(1).0,
        ifac_state: Arc::new(std::sync::RwLock::new(None)),
        ifac_violations: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        peer_routes: Arc::new(tokio::sync::Mutex::new(PeerRouteTable {
            peer_ifaces: BTreeMap::from([(peer_addr, expired_iface)]),
            outbound_routes: BTreeMap::from([(
                expired_iface,
                AutoPeerOutboundRoute { socket: Arc::clone(&socket), destination: peer_addr },
            )]),
        })),
    };

    bridge.remove_expired_peer_routes(&["192.0.2.10".to_string()]).await;
    let fresh_iface = bridge
        .ensure_peer_iface(peer_addr, AutoPeerOutboundRoute { socket, destination: peer_addr })
        .await
        .expect("re-register expired peer");

    assert_ne!(fresh_iface, expired_iface);
    let hashes = manager.lock().await.interface_hashes();
    assert!(hashes.contains(&fresh_iface));
    assert!(!hashes.contains(&expired_iface));
    let table = bridge.peer_routes.lock().await;
    assert_eq!(table.peer_ifaces.get(&peer_addr), Some(&fresh_iface));
    assert!(table.outbound_routes.contains_key(&fresh_iface));
    assert!(!table.outbound_routes.contains_key(&expired_iface));
}
