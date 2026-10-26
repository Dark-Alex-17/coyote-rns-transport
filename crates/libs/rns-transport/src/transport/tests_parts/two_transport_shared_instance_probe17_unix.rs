// The unix arm of `two_transport_shared_instance_probe17.rs`: the attach over a filesystem
// socket spawns a `LocalUnixClient` that applies `with_forced_bitrate` on its own code path,
// and the unix client sleeps the same `raw_len * 8 / bitrate` before each write, so the
// oracle is the TCP arm's, read on a raw `UnixListener`. Helpers and the probe17 constants
// come from the sibling `tests_parts` files included into this module.

/// Attaches `name` to `listener`'s socket with `options`, accepts its connection and
/// returns how long the first byte took to arrive at the listener after the accept, plus
/// the transport for teardown.
#[cfg(unix)]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_probe17_unix_time_to_first_bytes(
    name: &str,
    socket: &std::path::Path,
    listener: &tokio::net::UnixListener,
    options: SharedInstanceOptions,
) -> (Transport, Duration) {
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let transport = Transport::new(TransportConfig::new(name, &identity, false));
    let iface = transport
        .attach_shared_instance(
            SharedInstanceEndpoint::Unix(LocalUnixEndpoint::filesystem(socket.to_path_buf())),
            options,
        )
        .await
        .expect("a filesystem-socket attach is accepted");
    shared_instance_assert_attached(&transport, iface).await;
    let (mut stream, _) = timeout(LOOPBACK_TIMEOUT, listener.accept())
        .await
        .expect("timed out waiting for the unix client to connect")
        .expect("accept");
    let accepted = Instant::now();

    let mut buf = [0u8; 1];
    let read = timeout(
        SHARED_INSTANCE_PROBE17_ARRIVAL_CAP,
        tokio::io::AsyncReadExt::read(&mut stream, &mut buf),
    )
    .await
    .expect("the tunnel-synthesis packet reaches the shared instance within the cap")
    .expect("read the first byte");
    assert_eq!(read, 1, "the stream delivered a byte");
    (transport, accepted.elapsed())
}

/// With the default options the first frame reaches the unix shared instance well inside
/// the paced floor; with `with_forced_bitrate(400)` its first byte arrives no sooner than
/// the floor, since the spawned unix client sleeps `raw_len * 8 / 400` before the write.
#[cfg(unix)]
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_probe17_forced_bitrate_paces_the_first_frame_on_the_new_unix_stream() {
    install_test_logger();
    let temp = tempfile::tempdir().expect("tempdir");
    let socket = temp.path().join("shared.sock");
    let listener = tokio::net::UnixListener::bind(&socket).expect("bind the socket");

    let unpaced_name = "shared-instance-probe17-unix-unpaced";
    let (unpaced, unpaced_arrival) = shared_instance_probe17_unix_time_to_first_bytes(
        unpaced_name,
        &socket,
        &listener,
        SharedInstanceOptions::default(),
    )
    .await;
    assert!(
        unpaced_arrival < SHARED_INSTANCE_PROBE17_PACED_FLOOR,
        "without a forced bitrate the first frame arrived {unpaced_arrival:?} after the accept: \
         it should go out as soon as the stream is up"
    );
    timeout(STOP_TIMEOUT, unpaced.stop()).await.expect("unpaced stop completes");

    let paced_name = "shared-instance-probe17-unix-paced";
    let (paced, paced_arrival) = shared_instance_probe17_unix_time_to_first_bytes(
        paced_name,
        &socket,
        &listener,
        SharedInstanceOptions::default()
            .with_forced_bitrate(SHARED_INSTANCE_PROBE17_FORCED_BITRATE_BPS),
    )
    .await;
    assert!(
        paced_arrival >= SHARED_INSTANCE_PROBE17_PACED_FLOOR,
        "with a forced bitrate of {SHARED_INSTANCE_PROBE17_FORCED_BITRATE_BPS} bps the first \
         frame arrived {paced_arrival:?} after the accept: the spawned unix client did not pace \
         the write"
    );
    timeout(STOP_TIMEOUT, paced.stop()).await.expect("paced stop completes");
    shared_instance_assert_never_warned(unpaced_name);
    shared_instance_assert_never_warned(paced_name);
}
