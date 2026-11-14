// Two transports under one transport identity attached to a live Python
// rnsd. The tunnel id is the hash of the transport public key and the
// interface hash, and the interface hash comes from a per-transport counter,
// so two opted-in attaches under one identity that are each their
// transport's first interface synthesize one tunnel id; rnsd then takes the
// reappeared branch for the second attach and moves the first transport's
// path onto the second connection (`RNS/Transport.py:2820-2868`). Under the
// default neither synthesizes a tunnel and rnsd keeps a path per connection.
// The Rust `TcpServer` form of the same flow is the library's
// `two_transport_shared_instance_tunnel_identity.rs`.

const TWO_CLIENTS_APP_NAME: &str = "test";
const TWO_CLIENTS_RNSD_START: Duration = Duration::from_secs(30);
const TWO_CLIENTS_RNSD_LOG: Duration = Duration::from_secs(20);
const TWO_CLIENTS_ANNOUNCE: Duration = Duration::from_secs(20);
const TWO_CLIENTS_LINK: Duration = Duration::from_secs(20);
const TWO_CLIENTS_RESTORE: Duration = Duration::from_secs(30);
// The link request takes one hop to rnsd and one to a local client; a
// request that lands on the first transport activates well inside this.
const TWO_CLIENTS_NO_LINK: Duration = Duration::from_secs(3);
const TWO_CLIENTS_TEARDOWN: Duration = Duration::from_secs(5);

/// One of the two transports under the shared identity, with the one
/// destination it registered and its in-link events subscribed before the
/// attach.
struct TwoClientsRegistrant {
    transport: Transport,
    iface: AddressHash,
    desc: DestinationDesc,
    hex: String,
    in_links: tokio::sync::broadcast::Receiver<LinkEventData>,
}

/// Builds a transport under `shared`, registers one destination under a
/// fresh identity, attaches to rnsd with `options`, announces that
/// destination once and waits for rnsd to file its path.
#[deny(clippy::await_holding_invalid_type)]
async fn two_clients_attach_registrant(
    name: &str,
    aspect: &str,
    shared: &rns_transport::identity::PrivateIdentity,
    endpoint: SharedInstanceEndpoint,
    options: SharedInstanceOptions,
    rnsd_log: &ProcessLogTail,
) -> TwoClientsRegistrant {
    let mut config = TransportConfig::new(name, shared, false);
    config.set_path_request_timeout_secs(2);
    let transport = Transport::new(config);
    let in_links = transport.in_link_events();

    // Registered before the attach: a destination added while the posture is
    // set is announced again 250 ms later by the registration task, and the
    // explicit announce below must be the only one.
    let identity = to_transport_private_identity(&PrivateIdentity::new_from_rand(OsRng));
    let destination = transport
        .add_destination(identity, DestinationName::new(TWO_CLIENTS_APP_NAME, aspect))
        .await;
    let desc = { destination.lock().await.desc };
    let hex = hex::encode(desc.address_hash.as_slice());
    let iface = transport
        .attach_shared_instance(endpoint, options)
        .await
        .expect("attach to the python shared instance");
    assert_eq!(transport.announce_destinations_to_shared_instance().await, 1);

    // A local client's own announce is filed at zero hops
    // (`RNS/Transport.py:1938`, `:2461`).
    let path_learned = format!("Destination <{hex}> is now 0 hops away");
    rnsd_log
        .wait_for(&format!("{name}'s destination path"), TWO_CLIENTS_RNSD_LOG, |line| {
            (line.contains(&path_learned) && line.contains("on LocalInterface[")).then_some(())
        })
        .await;
    TwoClientsRegistrant { transport, iface, desc, hex, in_links }
}

/// The interface hash under which `registrant`'s shared-instance interface
/// is registered; the manager guard is held for the read alone.
#[deny(clippy::await_holding_invalid_type)]
async fn two_clients_interface_hash(
    registrant: &TwoClientsRegistrant,
) -> Option<rns_transport::hash::Hash> {
    let manager = registrant.transport.iface_manager();
    let manager = manager.lock().await;
    manager.full_hash(&registrant.iface)
}

#[deny(clippy::await_holding_invalid_type)]
async fn two_clients_learn_announce(
    announces: &mut tokio::sync::broadcast::Receiver<AnnounceEvent>,
    hash: &AddressHash,
) -> DestinationDesc {
    tokio::time::timeout(TWO_CLIENTS_ANNOUNCE, async {
        loop {
            match announces.recv().await {
                Ok(event) => {
                    let desc = event.destination.lock().await.desc;
                    if desc.address_hash == *hash {
                        return desc;
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    panic!("announce channel closed")
                }
            }
        }
    })
    .await
    .expect("timed out waiting for the announce relayed by rnsd")
}

#[deny(clippy::await_holding_invalid_type)]
async fn two_clients_await_in_link_activated(
    events: &mut tokio::sync::broadcast::Receiver<LinkEventData>,
    link_id: AddressHash,
) {
    tokio::time::timeout(TWO_CLIENTS_LINK, async {
        loop {
            match events.recv().await {
                Ok(event) if matches!(event.event, LinkEvent::Activated) => {
                    assert_eq!(event.id, link_id, "the activated in-link is the third client's");
                    return;
                }
                Ok(_) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    panic!("in-link event channel closed")
                }
            }
        }
    })
    .await
    .expect("timed out waiting for the in-link activation")
}

fn two_clients_assert_no_in_link_event(
    events: &mut tokio::sync::broadcast::Receiver<LinkEventData>,
    who: &str,
) {
    assert!(
        matches!(events.try_recv(), Err(tokio::sync::broadcast::error::TryRecvError::Empty)),
        "{who} saw an in-link event for a destination it did not register"
    );
}

/// The id out of `Tunnel endpoint <id> established.`
/// (`RNS/Transport.py:2823`); `RNS.prettyhexrep` renders a hash as `<hex>`.
fn two_clients_established_tunnel_id(line: &str) -> Option<String> {
    let (_, rest) = line.split_once("Tunnel endpoint <")?;
    let (id, rest) = rest.split_once('>')?;
    rest.starts_with(" established.").then(|| id.to_string())
}

#[deny(clippy::await_holding_invalid_type)]
async fn two_clients_detach_and_stop(transport: &Transport, who: &str) {
    let detached = tokio::time::timeout(TWO_CLIENTS_TEARDOWN, transport.detach_shared_instance())
        .await
        .unwrap_or_else(|_| panic!("{who}'s detach concludes"));
    assert!(detached, "{who}'s shared instance was attached");
    tokio::time::timeout(TWO_CLIENTS_TEARDOWN, transport.stop())
        .await
        .unwrap_or_else(|_| panic!("{who} stops"));
}

#[deny(clippy::await_holding_invalid_type)]
async fn run_python_rnsd_shared_instance_two_clients(tunnel_synthesis: bool) {
    let paths = python_channel_interop_paths();
    let temp = tempfile::tempdir().expect("tempdir");
    let instance = live_tcp_shared_instance();
    let rns_config = instance.config(free_tcp_port());
    let rnsd_config_dir = temp.path().join("python-rnsd-two-clients-rnsd");
    write_live_shared_instance_config(&rnsd_config_dir, &rns_config);

    let mut rnsd = paths.spawn_rnsd(&rnsd_config_dir);
    let rnsd_log = ProcessLogTail::follow("rnsd", &mut rnsd);
    let _rnsd_guard = ChildGuard { child: Some(rnsd) };
    rnsd_log
        .wait_for("the shared instance interface", TWO_CLIENTS_RNSD_START, |line| {
            line.contains("Started shared instance interface").then_some(())
        })
        .await;

    let options = || SharedInstanceOptions::default().with_tunnel_synthesis(tunnel_synthesis);
    let shared = to_transport_private_identity(&PrivateIdentity::new_from_rand(OsRng));

    // The third client's connect is served before a dials, and rnsd registers
    // each accepted local client before it reads from it (the epoll backend
    // at `RNS/Interfaces/LocalInterface.py:416-444`, the threaded server at
    // `:447-460`), so c is in `local_client_interfaces` before a's announce
    // is read; rnsd relays a local client's announce only to the local
    // clients registered at that moment (`RNS/Transport.py:2400-2429`).
    let c_identity = to_transport_private_identity(&PrivateIdentity::new_from_rand(OsRng));
    let mut c_config = TransportConfig::new("python-rnsd-two-clients-c", &c_identity, false);
    c_config.set_path_request_timeout_secs(2);
    let c = Transport::new(c_config);
    let mut c_announces = c.recv_announces().await;
    c.attach_shared_instance(
        instance.endpoint.clone(),
        SharedInstanceOptions::default().with_first_connect_timeout(TWO_CLIENTS_RNSD_LOG),
    )
    .await
    .expect("attach the third client to the python shared instance");

    let mut a = two_clients_attach_registrant(
        "python-rnsd-two-clients-a",
        "two-clients-a",
        &shared,
        instance.endpoint.clone(),
        options(),
        &rnsd_log,
    )
    .await;
    let tunnel_id = if tunnel_synthesis {
        let tunnel_id = rnsd_log
            .wait_for("a's tunnel", TWO_CLIENTS_RNSD_LOG, two_clients_established_tunnel_id)
            .await;
        let associated = format!("Path to <{}> associated with tunnel <{tunnel_id}>", a.hex);
        rnsd_log
            .wait_for("a's path under the tunnel", TWO_CLIENTS_RNSD_LOG, |line| {
                line.contains(&associated).then_some(())
            })
            .await;
        Some(tunnel_id)
    } else {
        None
    };
    let a_learned = two_clients_learn_announce(&mut c_announces, &a.desc.address_hash).await;

    let mut b = two_clients_attach_registrant(
        "python-rnsd-two-clients-b",
        "two-clients-b",
        &shared,
        instance.endpoint.clone(),
        options(),
        &rnsd_log,
    )
    .await;
    assert_ne!(
        a.desc.address_hash, b.desc.address_hash,
        "the two registrants hold distinct destinations"
    );
    let a_iface_hash = two_clients_interface_hash(&a).await;
    let b_iface_hash = two_clients_interface_hash(&b).await;
    assert!(a_iface_hash.is_some(), "a's attached interface is registered");
    assert_eq!(
        a_iface_hash, b_iface_hash,
        "both attaches are each transport's first interface, so the interface-hash counter agrees"
    );
    if let Some(tunnel_id) = &tunnel_id {
        let reappeared = format!("Tunnel endpoint <{tunnel_id}> reappeared. Restoring paths...");
        let reappeared_line = rnsd_log
            .wait_for("the reappeared tunnel", TWO_CLIENTS_RESTORE, |line| {
                line.contains(&reappeared).then(|| line.to_string())
            })
            .await;
        eprintln!("rnsd: {reappeared_line}");
        let restored = format!("Restored path to <{}>", a.hex);
        let restored_line = rnsd_log
            .wait_for("a's restored path", TWO_CLIENTS_RESTORE, |line| {
                line.contains(&restored).then(|| line.to_string())
            })
            .await;
        eprintln!("rnsd: {restored_line}");
    }

    let mut c_out = c.out_link_events();
    let link = c.link(a_learned).await.expect("link request sent");
    if tunnel_synthesis {
        match tokio::time::timeout(TWO_CLIENTS_NO_LINK, a.in_links.recv()).await {
            Err(_elapsed) => {}
            Ok(Ok(event)) => panic!(
                "in-link event for link {} reached a, whose path the restore moved to b's connection",
                event.id
            ),
            Ok(Err(err)) => panic!("a's in-link event channel failed: {err:?}"),
        }
        // The request reached b, which registered no such destination and drops it.
        two_clients_assert_no_in_link_event(&mut b.in_links, "b");
    } else {
        let link_id = wait_for_out_link_active(&mut c_out, &link, TWO_CLIENTS_LINK).await;
        two_clients_await_in_link_activated(&mut a.in_links, link_id).await;
        two_clients_assert_no_in_link_event(&mut b.in_links, "b");
        assert!(
            !rnsd_log.snapshot().contains("Tunnel endpoint "),
            "neither default attach synthesized a tunnel"
        );
    }

    two_clients_detach_and_stop(&c, "c").await;
    two_clients_detach_and_stop(&a.transport, "a").await;
    two_clients_detach_and_stop(&b.transport, "b").await;
}

/// A Python rnsd (RNS 1.5.2) is the shared instance on a free TCP port
/// (`RNS/Reticulum.py:417` logs the interface start) and two transports
/// built from one identity attach as local clients the way a second Python
/// program does (`RNS/Reticulum.py:420-441`), each registering a destination
/// of its own under the default options, after a third client has attached.
/// Proven: rnsd files both destinations at zero hops (`RNS/Transport.py:2461`),
/// the third client's link to the first transport's destination activates on
/// that transport, the second transport's in-link events stay empty, and rnsd
/// logs no `Tunnel endpoint` line at `LOG_PATHING` (`RNS/__init__.py:66`):
/// Python's local client synthesizes none (`RNS/Interfaces/LocalInterface.py`
/// has no tunnel code) and neither does the default attach.
#[tokio::test]
#[ignore = "requires local Python Reticulum checkout"]
async fn python_rnsd_shared_instance_two_clients_link_lands_on_the_registering_transport() {
    let _interop_guard = python_interop_guard().await;
    run_python_rnsd_shared_instance_two_clients(false).await;
}

/// The negative control for
/// [`python_rnsd_shared_instance_two_clients_link_lands_on_the_registering_transport`]
/// and the reason `with_tunnel_synthesis` is off by default. Both transports
/// opt in, and each tunnel id hashes the shared public key with an interface
/// hash both draw from their own first-interface counter, so the ids agree.
/// rnsd logs `Tunnel endpoint <id> established.` for the first attach
/// (`RNS/Transport.py:2823`) and files the first transport's destination
/// under that tunnel on its announce (`Path to <hash> associated with tunnel
/// <id>`, `:2467-2476`); the second attach takes the reappeared branch
/// (`:2829-2834`), which re-points the tunnel at the newest connection and
/// logs `Restored path to <first destination>` (`:2868`). The third client's
/// link request is then routed to the second transport, which registered no
/// such destination, and the first transport's in-link events stay empty for
/// a 3 s window.
#[tokio::test]
#[ignore = "requires local Python Reticulum checkout"]
async fn python_rnsd_shared_instance_two_clients_opted_in_tunnels_collide_in_rnsd() {
    let _interop_guard = python_interop_guard().await;
    run_python_rnsd_shared_instance_two_clients(true).await;
}
