// The second status gate of `send_response_resource`. The first gate reads the link status
// under a link guard that is released for the interface MTU lookup; the response is then
// prepared under a second link guard. A close that lands between the two has already reset
// the derived key to the empty key, so without a re-check the advertisement would be
// encrypted under the all-zero key and sent. This fixture parks the call in that window by
// holding the interface manager the MTU lookup needs, closes the link while it waits, and
// expects the call to refuse with `ConnectionError`, record the drop once, and publish no
// resource event for a transfer that never started. The companion test lets the lookup
// through with the link still active so the stall itself is shown not to refuse anything.

/// Holds `manager`'s lock on a blocking thread until `release` fires or its sender drops,
/// signalling `stalled` once the lock is held. The guard never spans an await.
fn stall_iface_manager_until_released(
    manager: Arc<Mutex<InterfaceManager>>,
    stalled: tokio::sync::oneshot::Sender<()>,
    release: tokio::sync::oneshot::Receiver<()>,
) {
    let _guard = manager.blocking_lock();
    let _ = stalled.send(());
    let _ = release.blocking_recv();
}

/// Starts a response Resource send on `pair`'s responder while its interface manager is held,
/// so the send parks in the MTU lookup between the two link guards, and returns the parked
/// send, the release for the stall and the link the responder tracks. The caller decides
/// what happens to the link before the stall lifts.
#[deny(clippy::await_holding_invalid_type)]
async fn park_response_send_in_the_mtu_window(
    pair: &LoopbackRequestPair,
    request_id: &[u8],
) -> (
    tokio::task::JoinHandle<Result<(Hash, crate::resource::OutboundOutcome), RnsError>>,
    tokio::sync::oneshot::Sender<()>,
    Arc<Mutex<Link>>,
) {
    let link_id = pair.handles.link_id;
    let responder = &pair.handles.listener.transport;
    let in_link =
        responder.find_in_link(&link_id).await.expect("responder tracks the inbound link");
    let handler = responder.get_handler();
    let manager = handler.lock().await.iface_manager.clone();

    let (stalled_tx, stalled_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = tokio::sync::oneshot::channel();
    tokio::task::spawn_blocking(move || {
        stall_iface_manager_until_released(manager, stalled_tx, release_rx)
    });
    stalled_rx.await.expect("stall task acquired the interface manager lock");

    let send = tokio::spawn({
        let request_id = request_id.to_vec();
        async move {
            request_dispatch::send_response_resource(
                &handler,
                &link_id,
                request_id,
                vec![0x5a; 64],
                None,
                true,
            )
            .await
        }
    });
    // The send passes the first gate and parks on the held interface manager; the test runs
    // on the current thread, so these yields are what lets it get there.
    for _ in 0..16 {
        tokio::task::yield_now().await;
    }
    assert!(!send.is_finished(), "the send must wait for the interface manager, not finish");
    (send, release_tx, in_link)
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_response_is_refused_when_the_link_closes_during_the_mtu_lookup(
) {
    install_test_logger();
    let pair = loopback_request_pair("request-closed-link-mtu-window").await;
    let link_id = pair.handles.link_id;
    let mut responder_events = pair.handles.listener.transport.resource_events();
    let before = closed_link_response_records_for(link_id).len();

    let (send, release, in_link) =
        park_response_send_in_the_mtu_window(&pair, b"closed-in-window").await;
    in_link.lock().await.close();
    release.send(()).expect("the stall is still waiting for its release");

    let result =
        timeout(LOOPBACK_TIMEOUT, send).await.expect("the send returns").expect("send task");
    assert!(
        matches!(result, Err(RnsError::ConnectionError)),
        "a link closed between the two guards refuses the response: {result:?}"
    );
    assert_eq!(
        closed_link_response_records_for(link_id).len(),
        before + 1,
        "the drop is recorded once for the link"
    );
    assert!(
        matches!(responder_events.try_recv(), Err(broadcast::error::TryRecvError::Empty)),
        "no resource event is published for a response that never left"
    );
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_response_survives_the_mtu_lookup_stall_on_an_active_link(
) {
    install_test_logger();
    let pair = loopback_request_pair("request-active-link-mtu-window").await;
    let link_id = pair.handles.link_id;
    let request_id = b"active-in-window";
    let before = closed_link_response_records_for(link_id).len();
    solicit_response(&pair.handles.connector.transport, &link_id, request_id).await;

    let (send, release, _in_link) = park_response_send_in_the_mtu_window(&pair, request_id).await;
    release.send(()).expect("the stall is still waiting for its release");

    let result =
        timeout(LOOPBACK_TIMEOUT, send).await.expect("the send returns").expect("send task");
    assert!(result.is_ok(), "an active link sends once the lookup completes: {result:?}");
    assert_eq!(
        closed_link_response_records_for(link_id).len(),
        before,
        "nothing is dropped for a link that stayed active"
    );
}
