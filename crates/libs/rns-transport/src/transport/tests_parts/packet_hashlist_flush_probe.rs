// The startup load of `packet_hashlist.raw` on the two first-boot shapes the orphan sweep
// must not break: a process that crashed before its very first save landed (an orphan
// beside no target) and a node whose storage directory does not exist yet. Both must load
// as empty, and the first must still sweep the orphan.

/// A crash between the first write and its rename leaves an orphan and no target; the next
/// startup loads an empty list and still removes the orphan, instead of failing or keeping
/// it around.
#[tokio::test]
async fn packet_hashlist_load_sweeps_an_orphan_when_no_list_was_ever_saved() {
    let temp = tempfile::tempdir().expect("transport storage");
    std::fs::write(temp.path().join("packet_hashlist.raw.tmp-4242-0"), b"half").expect("orphan");

    let loaded = ReticulumPacketDiskCache::new(temp.path())
        .load_packet_hashlist(temp.path())
        .await
        .expect("a first boot with an orphan still loads");

    assert!(loaded.is_empty(), "nothing was ever saved");
    assert!(
        temporary_hashlist_names(temp.path()).is_empty(),
        "the orphan is swept even when there is no list to load"
    );
}

/// A storage directory that does not exist yet is not an error: the sweep has nothing to
/// visit and the load is empty, as it was before the sweep existed.
#[tokio::test]
async fn packet_hashlist_load_tolerates_a_missing_storage_directory() {
    let temp = tempfile::tempdir().expect("transport storage");
    let missing = temp.path().join("not-created-yet");

    let loaded = ReticulumPacketDiskCache::new(&missing)
        .load_packet_hashlist(&missing)
        .await
        .expect("a missing storage directory loads as empty");

    assert!(loaded.is_empty());
    assert!(!missing.exists(), "the load creates nothing");
}
