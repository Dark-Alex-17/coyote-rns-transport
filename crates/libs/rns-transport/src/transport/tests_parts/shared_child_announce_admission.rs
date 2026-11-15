/// A limiter that holds on any measurable frequency, so an admitted announce proves the
/// interface was exempted rather than merely under threshold.
fn hold_everything_rate_limit() -> AnnounceRateLimit {
    AnnounceRateLimit {
        incoming_freq_samples: 3,
        max_held_announces: 8,
        new_time: Duration::from_secs(3600),
        burst_freq_new: 0.1,
        burst_freq: 0.1,
        burst_hold: Duration::from_millis(20),
        burst_penalty: Duration::from_millis(20),
        held_release_interval: Duration::from_millis(10),
    }
}

/// A transport-enabled node with one channel (flagged as the shared instance when asked)
/// and one virtual child registered on it, the way rnsd holds a listener and a per-client
/// child.
async fn node_with_child(shared_instance: bool) -> (Transport, AddressHash, AddressHash) {
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let transport = Transport::new(TransportConfig::new("shared-child-admission", &identity, true));
    let (parent, child) = {
        let manager = transport.iface_manager();
        let mut manager = manager.lock().await;
        let parent = *manager.new_channel(8).address();
        if shared_instance {
            assert!(manager.set_shared_instance(parent, true));
        }
        let child = manager
            .register_virtual_iface(parent, IfaceRole::Unicast)
            .expect("requesting local client");
        (parent, child)
    };
    transport.get_handler().lock().await.announce_limits =
        AnnounceLimits::with_rate_limit(hold_everything_rate_limit());
    (transport, parent, child)
}

fn three_fresh_announces() -> Vec<Packet> {
    (0..3)
        .map(|_| {
            SingleInputDestination::new(
                PrivateIdentity::new_from_rand(OsRng),
                DestinationName::new("lxmf", "delivery"),
            )
            .announce(OsRng, None)
            .expect("announce")
        })
        .collect()
}

async fn admissions(transport: &Transport, iface: AddressHash) -> Vec<bool> {
    let handler = transport.get_handler();
    let mut admitted = Vec::new();
    for announce in three_fresh_announces() {
        admitted.push(
            super::announce::admit_announce_before_queue(
                &announce,
                &handler,
                iface,
                IfaceSource::None,
            )
            .await,
        );
    }
    admitted
}

/// Python's local client interface never ingress-limits, both as a client program's
/// interface and as the child rnsd spawns per accepted client
/// (`RNS/Interfaces/LocalInterface.py:130`, consulted from `RNS/Transport.py:1823`). A
/// child of a flagged channel, and the flagged channel itself, admit back-to-back announces
/// for unknown destinations under a limiter that would otherwise hold on the third.
#[tokio::test]
async fn shared_child_admits_back_to_back_announces() {
    let (transport, parent, child) = node_with_child(true).await;

    assert_eq!(admissions(&transport, child).await, vec![true; 3]);
    assert_eq!(admissions(&transport, parent).await, vec![true; 3]);
}

/// The same limiter on an unflagged channel holds the third announce: the frequency reads 0
/// until more than `IC_DEQUE_MIN_SAMPLE` = 2 samples exist (`RNS/Interfaces/Interface.py:84`,
/// `:346-348`), so the first two pass and the third trips the burst.
#[tokio::test]
async fn unflagged_interface_still_holds_the_third_announce() {
    let (transport, _parent, child) = node_with_child(false).await;

    assert_eq!(admissions(&transport, child).await, vec![true, true, false]);
}
