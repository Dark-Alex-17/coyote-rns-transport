// The shutdown flush of `packet_hashlist.raw` must survive two things that happen on a busy
// node: an inbound packet holding the packet cache at the moment the flush starts, and a
// second writer (the debounced persistence worker) or a signal landing mid-write. The first
// test holds the cache from outside and expects the save to wait rather than fail; the second
// checks the file is replaced whole, with no temporary left behind.

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

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn packet_hashlist_save_replaces_the_file_whole() {
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let mut config = TransportConfig::new("hashlist-atomic", &identity, true);
    config.set_retransmit(true);
    let transport = Transport::new(config);
    let packet_cache = transport.get_handler().lock().await.packet_cache.clone();
    let packets = [
        Packet { data: PacketDataBuffer::new_from_slice(b"one"), ..Default::default() },
        Packet { data: PacketDataBuffer::new_from_slice(b"two"), ..Default::default() },
    ];
    {
        let mut packet_cache = packet_cache.lock().await;
        for packet in &packets {
            packet_cache.update(packet);
        }
    }
    let temp = tempfile::tempdir().expect("transport storage");
    std::fs::write(temp.path().join("packet_hashlist.raw"), [0x5a; crate::hash::HASH_SIZE])
        .expect("stale hashlist");

    assert_eq!(transport.save_packet_hashlist(temp.path()).await.expect("save"), 2);

    assert!(
        !temp.path().join("packet_hashlist.raw.tmp").exists(),
        "the temporary file is renamed over the target, not left behind"
    );
    let mut expected = packet_cache.lock().await.hashes();
    let mut loaded = ReticulumPacketDiskCache::new(temp.path())
        .load_packet_hashlist(temp.path())
        .await
        .expect("load");
    expected.sort_unstable_by_key(Hash::to_bytes);
    loaded.sort_unstable_by_key(Hash::to_bytes);
    assert_eq!(loaded, expected, "the saved file reads back as the cached hashes");
}
