// Loopback coverage for `AnnounceEvent::is_path_response`: the announce a path request
// draws out of the listener carries `PacketContext::PathResponse` and the flag, an
// unsolicited one carries neither. Helpers come from the sibling `tests_parts` files
// included into this module.

/// Consumes announces until the one for `hash` arrives and returns the whole event.
#[deny(clippy::await_holding_invalid_type)]
async fn await_announce_for(
    announces: &mut broadcast::Receiver<AnnounceEvent>,
    hash: &AddressHash,
) -> AnnounceEvent {
    timeout(LOOPBACK_TIMEOUT, async {
        loop {
            match announces.recv().await {
                Ok(event) => {
                    if event.destination.lock().await.desc.address_hash == *hash {
                        return event;
                    }
                }
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => panic!("announce channel closed"),
            }
        }
    })
    .await
    .expect("timed out waiting for the announce")
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn unsolicited_announce_is_not_a_path_response() {
    let listener = listener("loopback-listener", TcpClient::DEFAULT_MTU).await;
    let mut connector =
        connector("loopback-connector", listener.port, TcpClient::DEFAULT_MTU).await;
    listener.await_accepted_connections(1).await;

    let (_destination, desc) =
        register_in_destination(&listener.transport, "loopback", "announce-fresh").await;
    let event = await_announce_for(&mut connector.announces, &desc.address_hash).await;

    assert!(!event.is_path_response, "a fresh announce is unsolicited");
}

/// The destination is registered without announcing, so the first announce the connector
/// hears for it is the listener's direct answer to the path request
/// (`RNS/Transport.py:2524-2528` passes the same flag to Python announce handlers).
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn announce_drawn_by_a_path_request_is_a_path_response() {
    let listener = listener("loopback-listener", TcpClient::DEFAULT_MTU).await;
    let mut connector =
        connector("loopback-connector", listener.port, TcpClient::DEFAULT_MTU).await;
    listener.await_accepted_connections(1).await;

    let identity = PrivateIdentity::new_from_rand(OsRng);
    let destination = listener
        .transport
        .add_destination(identity, DestinationName::new("loopback", "announce-path-response"))
        .await;
    let address_hash = destination.lock().await.desc.address_hash;

    connector.transport.request_path(&address_hash, None, None).await;
    let event = await_announce_for(&mut connector.announces, &address_hash).await;

    assert!(event.is_path_response, "the announce answered the path request");
}
