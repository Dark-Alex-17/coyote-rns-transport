// The orphan sweep's error arms: an orphan that vanished between the directory listing and
// its removal is silent (another process sharing the storage swept it first), and any other
// removal failure is warned about and left in place while the list beside it still loads.
// The vanished-orphan arm cannot be driven from one process without a race, so this fixture
// pins the arm beside it: a directory carrying the temporary prefix cannot be removed as a
// file, and the load warns about it and returns the list anyway.

/// A saved list beside an entry the sweep cannot remove: the load warns, leaves the entry,
/// and returns the list.
#[tokio::test]
async fn packet_hashlist_load_warns_and_continues_when_an_orphan_cannot_be_removed() {
    install_test_logger();
    let temp = tempfile::tempdir().expect("transport storage");
    let cache = ReticulumPacketDiskCache::new(temp.path());
    let saved = [Hash::new([0x11; HASH_SIZE])];
    cache.save_packet_hashlist(temp.path(), &saved).await.expect("a list saves");
    let stuck = temp.path().join("packet_hashlist.raw.tmp-stuck");
    std::fs::create_dir(&stuck).expect("an unremovable orphan");

    let loaded = cache.load_packet_hashlist(temp.path()).await;

    assert_eq!(
        loaded.expect("an orphan the sweep cannot remove does not fail the load"),
        saved,
        "the list beside the orphan is returned"
    );
    let warnings = hashlist_orphan_warnings_for(&stuck);
    assert_eq!(
        warnings.len(),
        1,
        "exactly one warning names the orphan the sweep could not remove: {warnings:?}"
    );
    assert!(stuck.is_dir(), "the entry the sweep could not remove is left in place");
    assert!(
        temp.path().join("packet_hashlist.raw").is_file(),
        "the saved list is untouched by the load"
    );
}

/// Once the entry the sweep could not remove is gone the same list loads without a further
/// warning, so the warning above is the sweep's and not the list's.
#[tokio::test]
async fn packet_hashlist_load_stops_warning_once_the_unremovable_orphan_is_gone() {
    install_test_logger();
    let temp = tempfile::tempdir().expect("transport storage");
    let cache = ReticulumPacketDiskCache::new(temp.path());
    let saved = [Hash::new([0x22; HASH_SIZE]), Hash::new([0x33; HASH_SIZE])];
    cache.save_packet_hashlist(temp.path(), &saved).await.expect("a list saves");
    let stuck = temp.path().join("packet_hashlist.raw.tmp-stuck");
    std::fs::create_dir(&stuck).expect("an unremovable orphan");
    cache.load_packet_hashlist(temp.path()).await.expect("the list loads past the orphan");
    assert_eq!(hashlist_orphan_warnings_for(&stuck).len(), 1, "the first load warned once");

    std::fs::remove_dir(&stuck).expect("the operator clears the entry");
    let loaded = cache.load_packet_hashlist(temp.path()).await.expect("the list loads");

    assert_eq!(loaded, saved, "the list saved before the first load is intact");
    assert_eq!(
        hashlist_orphan_warnings_for(&stuck).len(),
        1,
        "a load with nothing left to sweep warns about nothing"
    );
}
