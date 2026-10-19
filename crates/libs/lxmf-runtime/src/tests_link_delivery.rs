// Link delivery tests, `include!`d from `tests.rs` so that file stays under
// the module-size budget. Same module, same imports.

#[tokio::test]
async fn link_delivery_without_path_fails_before_the_first_activation_wait() {
    let identity = rns_transport::identity::PrivateIdentity::new_from_rand(OsRng);
    let transport =
        Transport::new(TransportConfig::new("lxmf-runtime-link-no-path-test", &identity, false));
    let mut iface = transport.iface_manager().lock().await.new_channel(4);
    let remote = *rns_transport::identity::PrivateIdentity::new_from_rand(OsRng).as_identity();
    let destination = rns_transport::destination::DestinationDesc {
        identity: remote,
        address_hash: remote.address_hash,
        name: DestinationName::new("lxmf", "delivery"),
    };

    let started = std::time::Instant::now();
    let error = send_link_payload(
        &transport,
        destination,
        b"payload",
        MessageId("message-no-path".to_owned()),
        DeliveryMethod::Direct,
        MessageMethod::Packet,
        Duration::from_secs(12),
        3,
        Duration::from_secs(1),
        None,
    )
    .await
    .expect_err("no path and no broadcast");

    assert!(started.elapsed() < Duration::from_secs(1), "took {:?}", started.elapsed());
    assert_eq!(error.category, lxmf_sdk::ErrorCategory::Transport);
    assert!(
        error.message.contains("link request not sent"),
        "unexpected message: {}",
        error.message
    );
    assert!(iface.tx_channel.try_recv().is_err(), "no packet left the interface");
}

/// A pending link that never activates expires its path and queues a
/// rediscovery (`jobs_parts/link_maintenance.rs`), so the second attempt can
/// find no path even though the first one sent a request. That expiry only
/// fires after the link's own establishment timeout, far past a short
/// `link_timeout`, so the test expires the path by hand once the first request
/// has left the interface. The retry must then wait for a path, not abort.
#[tokio::test]
async fn link_delivery_waits_for_a_path_that_vanished_between_attempts() {
    let identity = rns_transport::identity::PrivateIdentity::new_from_rand(OsRng);
    let transport =
        Transport::new(TransportConfig::new("lxmf-runtime-link-path-lost-test", &identity, false));
    let mut iface = transport.iface_manager().lock().await.new_channel(4);
    let mut remote = SingleInputDestination::new(
        rns_transport::identity::PrivateIdentity::new_from_rand(OsRng),
        DestinationName::new("lxmf", "delivery"),
    );
    let destination = remote.desc;
    let announce = remote.announce(OsRng, None).expect("valid announce");
    iface
        .rx_channel
        .send(RxMessage { address: iface.address, packet: announce, source: IfaceSource::None })
        .await
        .expect("interface rx open");
    for _ in 0..40 {
        if transport.has_path(&destination.address_hash).await {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert!(transport.has_path(&destination.address_hash).await, "announce seeded a path");

    let timeout = Duration::from_millis(300);
    let started = std::time::Instant::now();
    let (result, path_requested) = tokio::join!(
        send_link_payload(
            &transport,
            destination,
            b"payload",
            MessageId("message-path-lost".to_owned()),
            DeliveryMethod::Direct,
            MessageMethod::Packet,
            timeout,
            2,
            Duration::from_secs(1),
            None,
        ),
        async {
            let first = iface.tx_channel.recv().await.expect("first link request leaves");
            assert_eq!(first.packet.header.packet_type, PacketType::LinkRequest);
            assert!(transport.expire_path(&destination.address_hash).await);
            // Bounded so that a retry which gives up instead of asking for a
            // path fails the test rather than parking it.
            match tokio::time::timeout(
                2 * timeout + Duration::from_secs(1),
                iface.tx_channel.recv(),
            )
            .await
            {
                Ok(Some(second)) => {
                    second.packet.data.as_slice()[..rns_transport::hash::ADDRESS_HASH_SIZE]
                        == destination.address_hash.as_slice()[..]
                }
                Ok(None) | Err(_) => false,
            }
        }
    );

    let error = result.expect_err("no peer answers the request and no path returns");
    assert!(
        started.elapsed() < 2 * timeout + Duration::from_secs(2),
        "took {:?}",
        started.elapsed()
    );
    assert_eq!(error.category, lxmf_sdk::ErrorCategory::Transport);
    assert!(
        error.message.contains("link activation") && error.message.contains("no path"),
        "unexpected message: {}",
        error.message
    );
    assert!(path_requested, "the retry asked the network for a path");
}
