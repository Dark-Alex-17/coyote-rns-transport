// The outcome future handed out by `track_prepared_with_outcome`: one test
// per failure path, plus `Complete`, `Rejected` and `Cancelled`, each
// checking the verdict the future resolves to and that the verdict leaves
// no outbound state behind.

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

#[deny(clippy::await_holding_invalid_type)]
async fn settled(outcome: OutboundOutcome) -> OutboundResult {
    tokio::time::timeout(OUTCOME_WAIT, outcome).await.expect("outcome resolves")
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn outcome_resolves_not_sent_when_the_advertisement_write_fails() {
    let mut manager = ResourceManager::new_with_config(Duration::from_secs(1), 2);
    let (_link, hash, outcome) = tracked_send_with_outcome(&mut manager);

    manager.confirm_outbound_dispatch(hash, false);

    assert_eq!(settled(outcome).await, OutboundResult::Failed(OutboundFailure::NotSent));
    assert!(manager.has_no_outbound_state());
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn outcome_resolves_link_closed_when_the_link_state_is_removed() {
    let mut manager = ResourceManager::new_with_config(Duration::from_secs(1), 2);
    let (link, hash, outcome) = tracked_send_with_outcome(&mut manager);
    manager.confirm_outbound_dispatch(hash, true);

    manager.remove_link_state(*link.id());

    assert_eq!(settled(outcome).await, OutboundResult::Failed(OutboundFailure::LinkClosed));
    assert!(manager.has_no_outbound_state());
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn outcome_resolves_rejected_on_a_receiver_cancel() {
    let mut manager = ResourceManager::new_with_config(Duration::from_secs(1), 2);
    let (mut link, hash, outcome) = tracked_send_with_outcome(&mut manager);
    manager.confirm_outbound_dispatch(hash, true);

    let cancel =
        resource_packet(PacketContext::ResourceReceiverCancel, hash.as_slice(), *link.id());
    assert!(manager.handle_packet(&cancel, &mut link).is_empty());

    assert_eq!(settled(outcome).await, OutboundResult::Rejected(RejectReason::PeerCancelled));
    assert!(manager.has_no_outbound_state());
    let events = manager.drain_events();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].hash, hash);
    assert!(matches!(
        events[0].kind,
        ResourceEventKind::OutboundRejected { reason: RejectReason::PeerCancelled }
    ));
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn outcome_resolves_cancelled_on_a_local_cancel() {
    let mut manager = ResourceManager::new_with_config(Duration::from_secs(1), 2);
    let (link, hash, outcome) = tracked_send_with_outcome(&mut manager);
    manager.confirm_outbound_dispatch(hash, true);

    let cancel = manager.cancel_outgoing(hash, &link).expect("cancel packet");
    assert!(cancel.is_some());

    assert_eq!(settled(outcome).await, OutboundResult::Cancelled);
    assert!(manager.has_no_outbound_state());
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn outcome_resolves_link_closed_when_the_manager_is_dropped_without_a_verdict() {
    let mut manager = ResourceManager::new_with_config(Duration::from_secs(1), 2);
    let (_link, hash, outcome) = tracked_send_with_outcome(&mut manager);
    manager.confirm_outbound_dispatch(hash, true);

    drop(manager);

    assert_eq!(settled(outcome).await, OutboundResult::Failed(OutboundFailure::LinkClosed));
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
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

/// A sender that failed on its own, as one does when the next segment of a
/// split transfer cannot be built, is reported by the retry poll as
/// `Failed(Segmenting)`, distinct from a sender that ran out of retries.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn outcome_resolves_segmenting_when_the_sender_has_failed() {
    let mut manager = ResourceManager::new_with_config(Duration::from_secs(1), 2);
    let (_link, hash, outcome) = tracked_send_with_outcome(&mut manager);
    manager.confirm_outbound_dispatch(hash, true);

    manager.outgoing.get_mut(&hash).expect("sender").status = ResourceStatus::Failed;
    assert!(manager.poll_outgoing(Instant::now()).is_empty());

    assert_eq!(settled(outcome).await, OutboundResult::Failed(OutboundFailure::Segmenting));
    assert!(manager.has_no_outbound_state());
    let events = manager.drain_events();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].hash, hash);
    assert!(matches!(events[0].kind, ResourceEventKind::OutboundFailed));
}

/// The proof of the first segment is where the next one gets built; a reader
/// that fails there resolves the outcome `Failed(Segmenting)` and leaves no
/// segment chain behind.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn outcome_resolves_segmenting_when_the_next_segment_cannot_be_built() {
    let (sender_link, _) = resource_link_pair();
    let data = vec![0x6d; MAX_EFFICIENT_SIZE + 1];
    let mut manager = ResourceManager::new_with_config(Duration::from_secs(1), 2);
    let prepared = ResourceManager::prepare_send_from_reader(
        &sender_link,
        FailingReader { data: data.clone(), offset: 0, fail_at: MAX_EFFICIENT_SIZE },
        data.len() as u64,
        None,
        None,
        false,
        DEFAULT_RESOURCE_INTERFACE_MTU,
        false,
    )
    .expect("first reader segment should be available");
    let (original_hash, _advert, outcome) = manager.track_prepared_with_outcome(prepared);
    manager.confirm_outbound_dispatch(original_hash, true);

    let expected_proof = manager.outgoing.get(&original_hash).expect("first sender").expected_proof;
    let proof = ResourceProof { resource_hash: original_hash, proof: expected_proof };
    let mut link = sender_link;
    let packets = manager.handle_packet(
        &resource_packet(PacketContext::ResourceProof, &proof.encode(), *link.id()),
        &mut link,
    );

    assert!(packets.is_empty());
    assert_eq!(settled(outcome).await, OutboundResult::Failed(OutboundFailure::Segmenting));
    assert!(manager.has_no_outbound_state());
    let events = manager.drain_events();
    assert!(
        events.iter().any(|event| event.hash == original_hash
            && matches!(event.kind, ResourceEventKind::OutboundFailed)),
        "got {events:?}"
    );
}
