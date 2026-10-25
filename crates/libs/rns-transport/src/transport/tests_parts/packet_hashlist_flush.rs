// The shutdown flush of `packet_hashlist.raw` must survive three things that happen on a busy
// node: an inbound packet holding the packet cache at the moment the flush starts, a signal
// landing mid-write, and the debounced persistence worker writing at the same time. The first
// test holds the cache from outside and expects the save to wait rather than fail; the second
// checks the file is replaced whole without touching a stranger's temporary; the third runs
// two saves at once and expects both to land. The last two cover the temporaries themselves:
// a failed save removes its own, and a load sweeps the ones a crashed process left behind.

/// The save blocks on the packet cache while another holder has it, then completes once the
/// holder lets go. Holding the guard across the yields is the point of this test.
#[tokio::test]
async fn packet_hashlist_save_waits_for_a_busy_packet_cache() {
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let mut config = TransportConfig::new("hashlist-busy", &identity, true);
    config.set_retransmit(true);
    let transport = Arc::new(Transport::new(config));
    let packet_cache = transport.get_handler().lock().await.packet_cache.clone();
    packet_cache.lock().await.update(&Packet::default());
    let temp = tempfile::tempdir().expect("transport storage");

    let held = packet_cache.lock().await;
    let save = tokio::spawn({
        let transport = transport.clone();
        let storage = temp.path().to_path_buf();
        async move { transport.save_packet_hashlist(storage).await }
    });
    for _ in 0..8 {
        tokio::task::yield_now().await;
    }
    assert!(!save.is_finished(), "save must wait for the packet cache, not fail while it is held");
    drop(held);

    let saved = timeout(Duration::from_secs(5), save)
        .await
        .expect("save completes once the cache is released")
        .expect("save task")
        .expect("save");
    assert_eq!(saved, 1);
}

#[deny(clippy::await_holding_invalid_type)]
async fn transport_with_cached_packets(name: &str, payloads: &[&[u8]]) -> Transport {
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let mut config = TransportConfig::new(name, &identity, true);
    config.set_retransmit(true);
    let transport = Transport::new(config);
    let packet_cache = transport.get_handler().lock().await.packet_cache.clone();
    let mut packet_cache = packet_cache.lock().await;
    for payload in payloads {
        packet_cache.update(&Packet {
            data: PacketDataBuffer::new_from_slice(payload),
            ..Default::default()
        });
    }
    transport
}

#[deny(clippy::await_holding_invalid_type)]
async fn sorted_cached_hashes(transport: &Transport) -> Vec<Hash> {
    let packet_cache = transport.get_handler().lock().await.packet_cache.clone();
    let mut hashes = packet_cache.lock().await.hashes();
    hashes.sort_unstable_by_key(Hash::to_bytes);
    hashes
}

async fn sorted_loaded_hashes(storage: &std::path::Path) -> Vec<Hash> {
    let mut hashes =
        ReticulumPacketDiskCache::new(storage).load_packet_hashlist(storage).await.expect("load");
    hashes.sort_unstable_by_key(Hash::to_bytes);
    hashes
}

fn temporary_hashlist_names(storage: &std::path::Path) -> Vec<String> {
    let mut names = std::fs::read_dir(storage)
        .expect("storage listing")
        .map(|entry| entry.expect("storage entry").file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("packet_hashlist.raw.tmp"))
        .collect::<Vec<_>>();
    names.sort_unstable();
    names
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn packet_hashlist_save_replaces_the_file_whole() {
    let transport = transport_with_cached_packets("hashlist-atomic", &[b"one", b"two"]).await;
    let temp = tempfile::tempdir().expect("transport storage");
    std::fs::write(temp.path().join("packet_hashlist.raw"), [0x5a; crate::hash::HASH_SIZE])
        .expect("stale hashlist");
    let stranger_tmp = temp.path().join("packet_hashlist.raw.tmp");
    std::fs::write(&stranger_tmp, b"not ours").expect("stranger temporary");

    assert_eq!(transport.save_packet_hashlist(temp.path()).await.expect("save"), 2);

    assert_eq!(
        temporary_hashlist_names(temp.path()),
        ["packet_hashlist.raw.tmp"],
        "our temporary is renamed over the target; the stranger's is the only one left"
    );
    assert_eq!(
        std::fs::read(&stranger_tmp).expect("stranger temporary"),
        b"not ours",
        "a temporary we did not create is never written to"
    );
    assert_eq!(
        sorted_loaded_hashes(temp.path()).await,
        sorted_cached_hashes(&transport).await,
        "the saved file reads back as exactly the cached hashes"
    );
}

/// The debounced persistence worker and the shutdown flush can save at the same moment. Each
/// must write its own temporary, or the slower rename finds its source already moved away.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn concurrent_packet_hashlist_saves_both_land() {
    let transport =
        transport_with_cached_packets("hashlist-race", &[b"one", b"two", b"three"]).await;
    let temp = tempfile::tempdir().expect("transport storage");

    let (first, second) = tokio::join!(
        transport.save_packet_hashlist(temp.path()),
        transport.save_packet_hashlist(temp.path())
    );
    assert_eq!(first.expect("first save"), 3);
    assert_eq!(second.expect("second save"), 3);

    assert_eq!(
        sorted_loaded_hashes(temp.path()).await,
        sorted_cached_hashes(&transport).await,
        "whichever save renamed last, the file reads back as the cached hashes"
    );
    assert!(
        temporary_hashlist_names(temp.path()).is_empty(),
        "both temporaries are renamed over the target, none is left behind"
    );
}

/// Per-call temporary names stop a failed save from being overwritten by the next one, so
/// the failing save has to clean up after itself.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn failed_packet_hashlist_save_removes_its_temporary() {
    let transport = transport_with_cached_packets("hashlist-failed", &[b"one"]).await;
    let temp = tempfile::tempdir().expect("transport storage");
    std::fs::create_dir(temp.path().join("packet_hashlist.raw")).expect("target as directory");
    std::fs::write(temp.path().join("packet_hashlist.raw").join("occupant"), b"busy")
        .expect("directory occupant");

    transport.save_packet_hashlist(temp.path()).await.expect_err("rename over a directory");

    assert!(
        temporary_hashlist_names(temp.path()).is_empty(),
        "the failed save removes the temporary it wrote"
    );
}

/// A process killed between write and rename leaves `packet_hashlist.raw.tmp-<pid>-<n>`
/// behind; nothing in that process can clean it, so the next startup load does.
#[tokio::test]
async fn packet_hashlist_load_sweeps_orphaned_temporaries() {
    let temp = tempfile::tempdir().expect("transport storage");
    let hash = [0x5a; crate::hash::HASH_SIZE];
    std::fs::write(temp.path().join("packet_hashlist.raw"), hash).expect("hashlist");
    std::fs::write(temp.path().join("packet_hashlist.raw.tmp-4242-0"), b"half").expect("orphan");
    std::fs::write(temp.path().join("packet_hashlist.raw.tmp-4242-7"), b"").expect("orphan");
    let stranger_tmp = temp.path().join("packet_hashlist.raw.tmp");
    std::fs::write(&stranger_tmp, b"not ours").expect("stranger temporary");

    assert_eq!(sorted_loaded_hashes(temp.path()).await, [Hash::new(hash)]);

    assert_eq!(
        temporary_hashlist_names(temp.path()),
        ["packet_hashlist.raw.tmp"],
        "only names of the form packet_hashlist.raw.tmp-<pid>-<n> are swept"
    );
    assert_eq!(std::fs::read(&stranger_tmp).expect("stranger temporary"), b"not ours");
}
