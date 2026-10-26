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

#[tokio::test]
async fn concurrent_peer_registrations_keep_one_virtual_iface() {
    let mut manager = InterfaceManager::new(8);
    let host = manager.new_channel(8).address;
    let manager = Arc::new(tokio::sync::Mutex::new(manager));
    let socket =
        Arc::new(tokio::net::UdpSocket::bind("127.0.0.1:0").await.expect("bind peer socket"));
    let peer_addr: SocketAddr = "192.0.2.11:9735".parse().expect("parse peer addr");
    let bridge = AutoInterfaceTransportBridge {
        host_iface: host,
        iface_manager: Arc::clone(&manager),
        rx_channel: tokio::sync::mpsc::channel(1).0,
        ifac_state: Arc::new(std::sync::RwLock::new(None)),
        ifac_violations: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        peer_routes: Arc::new(tokio::sync::Mutex::new(PeerRouteTable::default())),
    };

    // Holding the manager lets both callers miss the first table lookup
    // before either can register, the interleaving the discovery and data
    // tasks produce when a peer is first heard on both sockets.
    let gate = manager.lock().await;
    let callers = (0..2)
        .map(|_| {
            let bridge = bridge.clone();
            let route = AutoPeerOutboundRoute { socket: Arc::clone(&socket), destination: peer_addr };
            tokio::spawn(async move { bridge.ensure_peer_iface(peer_addr, route).await })
        })
        .collect::<Vec<_>>();
    for _ in 0..8 {
        tokio::task::yield_now().await;
    }
    drop(gate);
    let mut registered = Vec::new();
    for caller in callers {
        registered.push(caller.await.expect("caller task").expect("register peer"));
    }

    assert_eq!(registered[0], registered[1]);
    let winner = registered[0];
    let hashes = manager.lock().await.interface_hashes();
    assert_eq!(hashes.len(), 2, "host plus exactly one virtual peer iface: {hashes:?}");
    assert!(hashes.contains(&winner));
    let table = bridge.peer_routes.lock().await;
    assert_eq!(table.peer_ifaces.get(&peer_addr), Some(&winner));
    assert_eq!(table.outbound_routes.len(), 1);
    assert!(table.outbound_routes.contains_key(&winner));
}
