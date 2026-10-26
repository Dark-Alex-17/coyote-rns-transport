// Pins that `SharedInstanceOptions::with_forced_bitrate` reaches the client the attach
// spawns, which `two_transport_shared_instance_options.rs` pins at the getter only, since
// the iface manager cannot read it back from the spawned client. The client sleeps
// `raw_len * 8 / bitrate` before each write, so the bitrate is observable from the shared
// instance's side as the time the first frame takes to arrive on a freshly accepted
// stream: the attach synthesizes a tunnel on the interface and that packet (about 200 raw
// bytes) goes out as soon as the stream is up with the default options, and no sooner than
// its paced delay with a bitrate low enough to tell apart. Helpers come from the sibling
// `tests_parts` files included into this module.

/// A bitrate at which the tunnel-synthesis packet (over 100 bytes raw) is paced for more
/// than 2 s, so a loaded host cannot blur it with the unpaced arrival.
const SHARED_INSTANCE_PROBE17_FORCED_BITRATE_BPS: u64 = 400;

/// A first frame sooner than this after the accept was not paced by the forced bitrate
/// above; an unpaced one lands within milliseconds of the accept.
const SHARED_INSTANCE_PROBE17_PACED_FLOOR: Duration = Duration::from_secs(2);

/// How long the first bytes may take to arrive at all.
const SHARED_INSTANCE_PROBE17_ARRIVAL_CAP: Duration = Duration::from_secs(20);

/// Attaches `name` to `listener`'s address with `options`, accepts its connection and
/// returns how long the first byte took to arrive at the listener after the accept, plus
/// the transport for teardown.
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_probe17_time_to_first_bytes(
    name: &str,
    listener: &Probe10TcpListener,
    options: SharedInstanceOptions,
) -> (Transport, Duration) {
    let addr = listener.local_addr().expect("listener addr");
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let transport = Transport::new(TransportConfig::new(name, &identity, false));
    let iface = transport
        .attach_shared_instance(SharedInstanceEndpoint::Tcp(addr), options)
        .await
        .expect("a loopback attach is accepted");
    shared_instance_assert_attached(&transport, iface).await;
    let (mut stream, _) = timeout(LOOPBACK_TIMEOUT, listener.accept())
        .await
        .expect("timed out waiting for the client to connect")
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

/// With the default options the first frame reaches the shared instance well inside the
/// paced floor; with `with_forced_bitrate(400)` its first byte arrives no sooner than the
/// floor, since the spawned client sleeps `raw_len * 8 / 400` before the write.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_probe17_forced_bitrate_paces_the_first_frame_on_the_new_stream() {
    install_test_logger();
    let listener = Probe10TcpListener::bind("127.0.0.1:0").await.expect("bind listener");

    let unpaced_name = "shared-instance-probe17-unpaced";
    let (unpaced, unpaced_arrival) = shared_instance_probe17_time_to_first_bytes(
        unpaced_name,
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

    let paced_name = "shared-instance-probe17-paced";
    let (paced, paced_arrival) = shared_instance_probe17_time_to_first_bytes(
        paced_name,
        &listener,
        SharedInstanceOptions::default()
            .with_forced_bitrate(SHARED_INSTANCE_PROBE17_FORCED_BITRATE_BPS),
    )
    .await;
    assert!(
        paced_arrival >= SHARED_INSTANCE_PROBE17_PACED_FLOOR,
        "with a forced bitrate of {SHARED_INSTANCE_PROBE17_FORCED_BITRATE_BPS} bps the first \
         frame arrived {paced_arrival:?} after the accept: the spawned client did not pace the \
         write"
    );
    timeout(STOP_TIMEOUT, paced.stop()).await.expect("paced stop completes");
    shared_instance_assert_never_warned(unpaced_name);
    shared_instance_assert_never_warned(paced_name);
}
