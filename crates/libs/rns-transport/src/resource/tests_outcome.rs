// The outcome future handed out by `track_prepared_with_outcome`: one test
// per terminal path, each checking the verdict the future resolves to.

const OUTCOME_WAIT: Duration = Duration::from_secs(1);

fn tracked_send_with_outcome(manager: &mut ResourceManager) -> (Link, Hash, OutboundOutcome) {
    let signer = PrivateIdentity::new_from_rand(OsRng);
    let identity = *signer.as_identity();
    let destination = DestinationDesc {
        identity,
        address_hash: identity.address_hash,
        name: DestinationName::new("lxmf", "resource"),
    };
    let (tx, _) = tokio::sync::broadcast::channel(1);
    let link = Link::new(destination, tx);
    let prepared = ResourceManager::prepare_send(
        &link,
        vec![0x5a; 512],
        None,
        None,
        false,
        DEFAULT_RESOURCE_INTERFACE_MTU,
    )
    .expect("prepare");
    let (hash, _advert, outcome) = manager.track_prepared_with_outcome(prepared);
    (link, hash, outcome)
}

async fn settled(outcome: OutboundOutcome) -> OutboundResult {
    tokio::time::timeout(OUTCOME_WAIT, outcome).await.expect("outcome resolves")
}

#[tokio::test]
async fn outcome_resolves_not_sent_when_the_advertisement_write_fails() {
    let mut manager = ResourceManager::new_with_config(Duration::from_secs(1), 2);
    let (_link, hash, outcome) = tracked_send_with_outcome(&mut manager);

    manager.confirm_outbound_dispatch(hash, false);

    assert_eq!(settled(outcome).await, OutboundResult::Failed(OutboundFailure::NotSent));
    assert!(manager.has_no_outbound_state());
}

#[tokio::test]
async fn outcome_resolves_link_closed_when_the_link_state_is_removed() {
    let mut manager = ResourceManager::new_with_config(Duration::from_secs(1), 2);
    let (link, hash, outcome) = tracked_send_with_outcome(&mut manager);
    manager.confirm_outbound_dispatch(hash, true);

    manager.remove_link_state(*link.id());

    assert_eq!(settled(outcome).await, OutboundResult::Failed(OutboundFailure::LinkClosed));
}

#[tokio::test]
async fn outcome_resolves_rejected_on_a_receiver_cancel() {
    let mut manager = ResourceManager::new_with_config(Duration::from_secs(1), 2);
    let (mut link, hash, outcome) = tracked_send_with_outcome(&mut manager);
    manager.confirm_outbound_dispatch(hash, true);

    let cancel =
        resource_packet(PacketContext::ResourceReceiverCancel, hash.as_slice(), *link.id());
    assert!(manager.handle_packet(&cancel, &mut link).is_empty());

    assert_eq!(settled(outcome).await, OutboundResult::Rejected(RejectReason::PeerCancelled));
    let events = manager.drain_events();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].hash, hash);
    assert!(matches!(
        events[0].kind,
        ResourceEventKind::OutboundRejected { reason: RejectReason::PeerCancelled }
    ));
}

#[tokio::test]
async fn outcome_resolves_cancelled_on_a_local_cancel() {
    let mut manager = ResourceManager::new_with_config(Duration::from_secs(1), 2);
    let (link, hash, outcome) = tracked_send_with_outcome(&mut manager);
    manager.confirm_outbound_dispatch(hash, true);

    let cancel = manager.cancel_outgoing(hash, &link).expect("cancel packet");
    assert!(cancel.is_some());

    assert_eq!(settled(outcome).await, OutboundResult::Cancelled);
}

#[tokio::test]
async fn outcome_resolves_link_closed_when_the_manager_is_dropped_without_a_verdict() {
    let mut manager = ResourceManager::new_with_config(Duration::from_secs(1), 2);
    let (_link, hash, outcome) = tracked_send_with_outcome(&mut manager);
    manager.confirm_outbound_dispatch(hash, true);

    drop(manager);

    assert_eq!(settled(outcome).await, OutboundResult::Failed(OutboundFailure::LinkClosed));
}

#[tokio::test]
async fn outcome_resolves_retries_exhausted_when_the_advertisement_is_never_answered() {
    let retry_interval = Duration::from_millis(10);
    let mut manager = ResourceManager::new_with_config(retry_interval, 1);
    let (_link, hash, outcome) = tracked_send_with_outcome(&mut manager);
    manager.confirm_outbound_dispatch(hash, true);

    // One retry is budgeted, so the second overdue poll exhausts it; the
    // bound only guards against a regression that never fails the sender.
    let mut now = Instant::now();
    for _ in 0..8 {
        if manager.has_no_outbound_state() {
            break;
        }
        now += retry_interval * 2;
        manager.poll_outgoing(now);
    }

    assert!(manager.has_no_outbound_state());
    assert_eq!(settled(outcome).await, OutboundResult::Failed(OutboundFailure::RetriesExhausted));
    let events = manager.drain_events();
    assert_eq!(events.len(), 1);
    assert!(matches!(events[0].kind, ResourceEventKind::OutboundFailed));
}
