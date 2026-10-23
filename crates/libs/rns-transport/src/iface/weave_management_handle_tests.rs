// Usage probe written from the CHANGELOG Unreleased sentence: "once a weave
// driver has stopped, `WeaveManagementHandle::try_set_remote_display`
// reports the closed queue at once instead of buffering commands nobody will
// read". Driven through the public surface only: a `WeaveInterface` on a
// device that does not exist, spawned and stopped through the
// `InterfaceManager`, with the handle taken the way an operator would.

/// Before spawn the `WeaveInterface` value holds the receiver, so a command
/// queues for the driver to read once the port opens; after `stop_interface`
/// the same call fails at once with `WouldBlock` naming the closed queue.
#[tokio::test]
async fn weave_management_handle_reports_the_closed_queue_once_the_driver_has_stopped() {
    let manager = Arc::new(tokio::sync::Mutex::new(InterfaceManager::new(4)));
    let missing_device = std::env::temp_dir()
        .join(format!("rns-weave-stopped-handle-{}", std::process::id()));
    let weave = WeaveInterface::new(missing_device.to_string_lossy(), manager.clone());
    let handle = weave.weave_management_handle();
    let switch = [0x0a, 0x0b, 0x0c, 0x0d];

    let queued = handle
        .try_set_remote_display(Some(switch), true)
        .expect("a command queues while the interface value still holds the receiver");
    assert_eq!(queued, switch);

    let address = manager.lock().await.spawn(weave, WeaveInterface::spawn);
    // The driver takes the management receiver on its first poll and then
    // sits in the reconnect wait on the missing device; the queued command
    // stays with it.
    tokio::task::yield_now().await;
    assert!(manager.lock().await.stop_interface(address), "the weave interface was registered");

    let error = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            match handle.try_set_remote_display(Some(switch), false) {
                Err(error) => break error,
                Ok(_) => tokio::task::yield_now().await,
            }
        }
    })
    .await
    .expect("once the driver has stopped the handle stops accepting commands");
    assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
    assert!(
        error.to_string().contains("closed"),
        "the refusal names the closed queue, got {error}"
    );
    // Not a one-off: every later command is refused the same way.
    let again = handle.try_set_remote_display(Some(switch), true).expect_err("still refused");
    assert_eq!(again.kind(), io::ErrorKind::WouldBlock);
}
