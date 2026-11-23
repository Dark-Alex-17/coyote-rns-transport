// `AnnounceEvent::emitted` and `AnnounceEvent::random_blob` against a live
// Python rnsd acting as the shared instance: the unsolicited relay of a local
// client's announce, the path response rnsd replays from its path table, and
// a re-announce in a later wall-clock second. The in-crate loopback twin is
// the library's `two_transport_loopback_announce_emitted.rs`.

const ANNOUNCE_EMITTED_RNSD_START: Duration = Duration::from_secs(30);
const ANNOUNCE_EMITTED_RNSD_LOG: Duration = Duration::from_secs(20);
const ANNOUNCE_EMITTED_EVENT: Duration = Duration::from_secs(20);

/// Whole seconds of the local clock, the reading the announcing destination
/// stamps into its random blob.
fn announce_emitted_clock_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock after the epoch")
        .as_secs()
}

/// The emission second a random blob carries: bytes 5..10 big-endian
/// (`RNS/Transport.py:3725-3726`), decoded here independently of the
/// transport's own helper.
fn announce_emitted_blob_second(
    random_blob: &[u8; rns_transport::destination::RAND_HASH_LENGTH],
) -> u64 {
    let mut emitted = [0u8; 8];
    emitted[3..].copy_from_slice(&random_blob[5..]);
    u64::from_be_bytes(emitted)
}

/// A Python rnsd (RNS 1.5.2) is the shared instance and two transports under
/// distinct identities attach as local clients, B before A. Each announces
/// its destination the way a Python shared-instance client does, as a path
/// response (`RNS/Transport.py:2911-2916`). Proven: rnsd relays A's announce
/// to B with the announce data unchanged and context `NONE`
/// (`RNS/Transport.py:2400-2429`), so B's event is not a path response and
/// its `emitted` is bytes 5..10 of its `random_blob`
/// (`RNS/Transport.py:3725-3726`) inside the clock bracket around A's
/// attach; after B expires the path and asks for it again, rnsd answers a
/// local client's path request from its path table by replaying the cached
/// announce bytes with the hops set from the table (`RNS/Transport.py:3459-3473`),
/// so B's event is a path response carrying the identical blob and emission
/// second, and the path table holds that second again; a re-announce in a
/// later wall-clock second, which rnsd files at equal hops only when its
/// emission second is strictly later (`RNS/Transport.py:2238`), reaches B
/// with a strictly greater `emitted` and a different blob.
#[tokio::test]
#[ignore = "requires local Python Reticulum checkout"]
async fn python_rnsd_shared_instance_relayed_path_response_keeps_announce_emitted() {
    let _interop_guard = python_interop_guard().await;
    let paths = python_channel_interop_paths();
    let temp = tempfile::tempdir().expect("tempdir");
    let instance = live_tcp_shared_instance();
    let rns_config = instance.config(free_tcp_port());
    let rnsd_config_dir = temp.path().join("python-rnsd-announce-emitted-rnsd");
    write_live_shared_instance_config(&rnsd_config_dir, &rns_config);

    let mut rnsd = paths.spawn_rnsd(&rnsd_config_dir);
    let rnsd_log = ProcessLogTail::follow("rnsd", &mut rnsd);
    let _rnsd_guard = ChildGuard { child: Some(rnsd) };
    rnsd_log
        .wait_for("the shared instance interface", ANNOUNCE_EMITTED_RNSD_START, |line| {
            line.contains("Started shared instance interface").then_some(())
        })
        .await;

    // B subscribes before A attaches: rnsd relays a local client's announce
    // only to the local clients registered when it reads the announce
    // (`RNS/Transport.py:2400-2429`), and the attach helper announces inside.
    let b_identity = to_transport_private_identity(&PrivateIdentity::new_from_rand(OsRng));
    let b = two_clients_attach_registrant(
        "python-rnsd-announce-emitted-b",
        "announce-emitted-b",
        &b_identity,
        instance.endpoint.clone(),
        SharedInstanceOptions::default(),
        &rnsd_log,
    )
    .await;
    let mut b_announces = b.transport.recv_announces().await;

    let a_identity = to_transport_private_identity(&PrivateIdentity::new_from_rand(OsRng));
    let before = announce_emitted_clock_secs();
    let a = two_clients_attach_registrant(
        "python-rnsd-announce-emitted-a",
        "announce-emitted-a",
        &a_identity,
        instance.endpoint.clone(),
        SharedInstanceOptions::default(),
        &rnsd_log,
    )
    .await;
    let a_announced = tokio::time::Instant::now();
    let after = announce_emitted_clock_secs();
    let a_hash = a.desc.address_hash;

    let original = wait_for_announce_event(&mut b_announces, a_hash, ANNOUNCE_EMITTED_EVENT).await;
    assert!(!original.is_path_response, "the relayed announce is unsolicited");
    assert_eq!(
        announce_emitted_blob_second(&original.random_blob),
        original.emitted,
        "the blob encodes emitted"
    );
    assert!(
        before <= original.emitted && original.emitted <= after,
        "emitted {} lies in the clock bracket {before}..={after}",
        original.emitted
    );
    assert_eq!(b.transport.latest_announce_emitted(&a_hash).await, Some(original.emitted));

    // rnsd queues no rebroadcast of a path-response announce
    // (`RNS/Transport.py:2351`), so no second copy of the original can land
    // on B between the expiry and the path response; the only announce for A
    // that B hears next is rnsd's answer.
    assert!(b.transport.expire_path(&a_hash).await, "the path table entry is removed");
    assert_eq!(b.transport.latest_announce_emitted(&a_hash).await, None);
    b.transport.request_path(&a_hash, None, None).await;
    let answering = format!("Answering path request for <{}>", a.hex);
    rnsd_log
        .wait_for("rnsd's path request answer", ANNOUNCE_EMITTED_RNSD_LOG, |line| {
            (line.contains(&answering) && line.contains("path is known")).then_some(())
        })
        .await;
    let replayed = wait_for_announce_event(&mut b_announces, a_hash, ANNOUNCE_EMITTED_EVENT).await;
    assert!(replayed.is_path_response, "rnsd answered the path request");
    assert_eq!(replayed.random_blob, original.random_blob, "rnsd replays the stored blob");
    assert_eq!(replayed.emitted, original.emitted, "a replay is not a fresh emission");
    assert_eq!(b.transport.latest_announce_emitted(&a_hash).await, Some(original.emitted));

    // Equal hops need a strictly newer emission second (`RNS/Transport.py:2238`)
    // and rnsd ignores an older or equal one silently, so the re-announce
    // waits a full second past the first one's return. With B's path request
    // pending, rnsd also queues the re-announce for one plain rebroadcast
    // (`RNS/Transport.py:2375-2396`); B drops that copy as a known blob.
    tokio::time::sleep_until(a_announced + Duration::from_millis(1100)).await;
    assert_eq!(a.transport.announce_destinations_to_shared_instance().await, 1);
    let second = wait_for_announce_event(&mut b_announces, a_hash, ANNOUNCE_EMITTED_EVENT).await;
    assert!(!second.is_path_response, "the re-announce is relayed unsolicited");
    assert!(
        second.emitted > original.emitted,
        "re-announce emitted {} is newer than {}",
        second.emitted,
        original.emitted
    );
    assert_ne!(second.random_blob, original.random_blob, "a fresh announce draws a fresh blob");
    assert_eq!(announce_emitted_blob_second(&second.random_blob), second.emitted);
    assert_eq!(b.transport.latest_announce_emitted(&a_hash).await, Some(second.emitted));

    two_clients_detach_and_stop(&a.transport, "a").await;
    two_clients_detach_and_stop(&b.transport, "b").await;
}
