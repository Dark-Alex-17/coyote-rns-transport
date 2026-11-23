// The outbound establishment timeout against the Python sizing rule:
//
//   timeout = first_hop_timeout + ESTABLISHMENT_TIMEOUT_PER_HOP * max(1, hops)
//   first_hop_timeout = Reticulum.MTU * 8 / bitrate + DEFAULT_PER_HOP_TIMEOUT
//
// (`RNS/Link.py:282-283`, `RNS/Transport.py:3194-3197`, `RNS/Reticulum.py:93`,
// `:142`). With the 500-byte MTU at the 62500 bps default bitrate the first
// hop is 6.064 s, so a 1-hop path times out at 12.064 s and a 3-hop path at
// 24.064 s, whatever hardware MTU the next hop interface carries. The pins
// below sit 4 ms below and 6 ms above those values: a 262144-byte MTU would
// move the 1-hop value to about 45.6 s (the shared-instance client
// interface carries that MTU), and even a 547-byte one past 12.07 s.

/// A pending out-link to a destination filed `hops` away over a fresh
/// channel interface of `mtu` bytes, with the request already drained.
async fn pending_out_link_over(
    transport: &Transport,
    mtu: usize,
    hops: u8,
) -> (Arc<Mutex<Link>>, crate::iface::InterfaceChannel) {
    let mut iface_channel = transport.iface_manager().lock().await.new_channel_with_role_mode_mtu(
        16,
        crate::iface::IfaceRole::default(),
        crate::iface::InterfaceMode::default(),
        mtu,
    );
    let iface = *iface_channel.address();
    let remote_identity = PrivateIdentity::new_from_rand(OsRng);
    let destination =
        SingleInputDestination::new(remote_identity, DestinationName::new("lxmf", "delivery"));
    let destination_hash = destination.desc.address_hash;
    {
        let handler = transport.get_handler();
        let mut guard = handler.lock().await;
        assert!(guard.path_table.restore_tunnel_path(
            destination_hash,
            destination_hash,
            hops,
            iface,
            Hash::new_from_slice(b"packet"),
            std::time::Instant::now(),
        ));
    }
    let link = transport.link(destination.desc).await.expect("link request sent");
    let _initial_request = timeout(Duration::from_millis(200), iface_channel.tx_channel.recv())
        .await
        .expect("initial link request should be queued")
        .expect("tx channel open");
    (link, iface_channel)
}

/// Asserts the link's timeout lies in (`not_yet`, `by`] measured from a
/// start the test sets.
async fn assert_timeout_between(
    link: &Arc<Mutex<Link>>,
    not_yet: Duration,
    by: Duration,
    what: &str,
) {
    let start = std::time::Instant::now();
    let mut link = link.lock().await;
    link.set_establishment_start_for_test(start);
    assert!(!link.establishment_timed_out(start + not_yet), "{what}: timed out before {not_yet:?}");
    assert!(link.establishment_timed_out(start + by), "{what}: not timed out by {by:?}");
}

/// 1 hop at 62500 bps: 6.064 s for the first hop plus 6 s for the hop,
/// 12.064 s pinned over the 262144-byte shared-instance MTU.
#[tokio::test]
async fn a_one_hop_link_over_a_262144_byte_interface_times_out_at_twelve_point_zero_six_seconds() {
    let local_identity = PrivateIdentity::new_from_rand(OsRng);
    let transport = Transport::new(TransportConfig::new("test", &local_identity, true));
    let (link, _iface) = pending_out_link_over(&transport, 262_144, 1).await;
    assert_timeout_between(
        &link,
        Duration::from_millis(12_060),
        Duration::from_millis(12_070),
        "1 hop over 262144 bytes",
    )
    .await;
}

/// The per-hop term is untouched by the fix: 3 hops add 18 s to the same
/// 6.064 s first hop, 24.064 s, over the same large interface.
#[tokio::test]
async fn a_three_hop_link_over_a_262144_byte_interface_times_out_at_twenty_four_point_zero_six_seconds(
) {
    let local_identity = PrivateIdentity::new_from_rand(OsRng);
    let transport = Transport::new(TransportConfig::new("test", &local_identity, true));
    let (link, _iface) = pending_out_link_over(&transport, 262_144, 3).await;
    assert_timeout_between(
        &link,
        Duration::from_millis(24_060),
        Duration::from_millis(24_070),
        "3 hops over 262144 bytes",
    )
    .await;
}

/// Python sizes every first hop with `Reticulum.MTU`, so interfaces of
/// 500, 16384 and 524288 bytes give one and the same 1-hop timeout.
#[tokio::test]
async fn interfaces_of_different_hardware_mtus_give_the_same_establishment_timeout() {
    let local_identity = PrivateIdentity::new_from_rand(OsRng);
    let transport = Transport::new(TransportConfig::new("test", &local_identity, true));
    let mut held = Vec::new();
    for mtu in [500usize, 16_384, 524_288] {
        let (link, iface) = pending_out_link_over(&transport, mtu, 1).await;
        assert_timeout_between(
            &link,
            Duration::from_millis(12_060),
            Duration::from_millis(12_070),
            &format!("1 hop over {mtu} bytes"),
        )
        .await;
        held.push(iface);
    }
}

/// Seen from the maintenance job a consumer observes: 11 s into a 1-hop
/// attempt over the large interface the link is still pending, 13 s in it
/// is closed with `Timeout` (the live roaming part sees 12.9 s), where the
/// hardware-MTU sizing left it pending until about 45 s. The pending step
/// stays a second inside the 12.064 s so the job's own clock read cannot
/// cross it.
#[tokio::test]
async fn the_maintenance_job_closes_a_one_hop_link_over_a_large_interface_thirteen_seconds_in() {
    let local_identity = PrivateIdentity::new_from_rand(OsRng);
    let transport = Transport::new(TransportConfig::new("test", &local_identity, true));
    let mut link_events = transport.out_link_events();
    let handler = transport.get_handler();
    let (link, _iface) = pending_out_link_over(&transport, 262_144, 1).await;

    link.lock()
        .await
        .set_establishment_start_for_test(std::time::Instant::now() - Duration::from_secs(11));
    super::jobs::handle_check_links(handler.lock().await).await;
    assert_eq!(link.lock().await.status(), LinkStatus::Pending, "11 s is inside the 12.064 s");

    link.lock()
        .await
        .set_establishment_start_for_test(std::time::Instant::now() - Duration::from_secs(13));
    super::jobs::handle_check_links(handler.lock().await).await;
    assert_eq!(link.lock().await.status(), LinkStatus::Closed, "13 s is past it");
    let close_event = timeout(Duration::from_millis(200), link_events.recv())
        .await
        .expect("link timeout publishes a close event")
        .expect("out-link event stream remains open");
    assert_eq!(close_event.id, *link.lock().await.id());
    assert!(matches!(close_event.event, crate::destination::link::LinkEvent::Closed));
    assert_eq!(close_event.close_reason, Some(crate::destination::link::LinkCloseReason::Timeout));
}
