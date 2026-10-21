// Coverage for what a shared-instance client does when its connection comes back: the
// reconnect re-synthesizes the tunnel and re-announces every registered destination as a
// path response after the reconnect wait plus two seconds. Helpers come from the sibling
// `tests_parts` files included into this module.

const SHARED_INSTANCE_RECONNECT_WAIT: Duration = Duration::from_millis(100);

/// The re-announce is due `reconnect_wait + 2 s` after the reconnect; the rest is
/// scheduling margin. Only this upper bound is asserted.
const SHARED_INSTANCE_RECONNECT_REANNOUNCE_BOUND: Duration =
    Duration::from_millis(100 + 2_000 + 3_000);

/// Attaches with the short reconnect wait so the restart below is observed quickly.
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_reconnect_client(
    name: &str,
    server: &SharedInstanceServer,
) -> SharedInstanceClient {
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let transport = Transport::new(TransportConfig::new(name, &identity, false));
    let announces = transport.recv_announces().await;
    let iface = transport
        .attach_shared_instance(
            server.endpoint(),
            SharedInstanceOptions::default().with_reconnect_wait(SHARED_INSTANCE_RECONNECT_WAIT),
        )
        .await
        .expect("attach to the shared instance");
    SharedInstanceClient { transport, announces, iface }
}

/// Registers a destination on the client without announcing it from the test and waits
/// for the client's own registration announce to reach the shared instance, so that the
/// announces observed later are the reconnect's alone.
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_reconnect_register(
    client: &SharedInstanceClient,
    server_announces: &mut broadcast::Receiver<AnnounceEvent>,
    aspect: &str,
) -> AddressHash {
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let destination = client
        .transport
        .add_destination(identity, DestinationName::new("shared-instance-reconnect", aspect))
        .await;
    let address_hash = destination.lock().await.desc.address_hash;
    let event = shared_instance_parity_await_announce(server_announces, &address_hash).await;
    assert!(event.is_path_response, "the registration announce is a path response");
    address_hash
}

/// Stops `server` and brings a fresh shared instance up on the same port; returns it with
/// its announce receiver (subscribed before any client can reconnect) and the instant its
/// listener accepted the client again.
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_reconnect_restart(
    server: SharedInstanceServer,
) -> (SharedInstanceServer, broadcast::Receiver<AnnounceEvent>, Instant) {
    let port = server.port;
    timeout(STOP_TIMEOUT, server.transport.stop()).await.expect("shared instance stop completes");
    let restarted = shared_instance_server_on(port).await;
    let announces = restarted.transport.recv_announces().await;
    restarted.await_accepted_connections(1).await;
    (restarted, announces, Instant::now())
}

/// Consumes announces until a path-response announce has arrived for every entry of
/// `expected`; returns the instant the last one arrived.
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_reconnect_await_reannounces(
    announces: &mut broadcast::Receiver<AnnounceEvent>,
    mut expected: Vec<AddressHash>,
) -> Instant {
    timeout(LOOPBACK_TIMEOUT, async {
        while !expected.is_empty() {
            match announces.recv().await {
                Ok(event) => {
                    let address_hash = event.destination.lock().await.desc.address_hash;
                    if event.is_path_response {
                        expected.retain(|hash| *hash != address_hash);
                    }
                }
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => panic!("announce channel closed"),
            }
        }
        Instant::now()
    })
    .await
    .expect("timed out waiting for the re-announces")
}

/// The shared instance goes away and comes back on the same port. The reconnect
/// re-announces both client destinations as path responses within the bound and lands them
/// in the new shared instance's tunnel table, while the attach stays in place.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_reconnect_reannounces_on_return() {
    let server = shared_instance_server().await;
    let mut server_announces = server.transport.recv_announces().await;
    let client = shared_instance_reconnect_client("shared-instance-reconnect", &server).await;
    server.await_accepted_connections(1).await;
    shared_instance_assert_attached(&client.transport, client.iface).await;

    let first = shared_instance_reconnect_register(&client, &mut server_announces, "first").await;
    let second = shared_instance_reconnect_register(&client, &mut server_announces, "second").await;

    let (restarted, mut restarted_announces, reaccepted_at) =
        shared_instance_reconnect_restart(server).await;

    let last_reannounce_at =
        shared_instance_reconnect_await_reannounces(&mut restarted_announces, vec![first, second])
            .await;
    let elapsed = last_reannounce_at.saturating_duration_since(reaccepted_at);
    assert!(
        elapsed <= SHARED_INSTANCE_RECONNECT_REANNOUNCE_BOUND,
        "both re-announces arrive within {SHARED_INSTANCE_RECONNECT_REANNOUNCE_BOUND:?} of the re-accept, took {elapsed:?}"
    );
    restarted.await_tunneled_destinations(vec![first, second]).await;
    assert!(
        client.transport.connected_to_shared_instance().await,
        "the connected posture survives the restart"
    );
    assert_eq!(client.transport.shared_instance(), Some(client.iface), "the slot keeps the iface");

    timeout(STOP_TIMEOUT, client.transport.stop()).await.expect("client stop completes");
    timeout(STOP_TIMEOUT, restarted.transport.stop())
        .await
        .expect("restarted shared instance stop completes");
}
