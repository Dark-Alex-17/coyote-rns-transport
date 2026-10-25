/// A `TcpClient` whose connect is still in flight against a black-holed endpoint must
/// not hold `stop` for its connect timeout. 192.0.2.1 (TEST-NET-1) is never routed, so
/// the SYN gets no answer and only cancellation or the 30 s timeout ends the connect.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn transport_stop_cancels_a_client_connect_still_in_flight() {
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let transport = Transport::new(TransportConfig::new("black-hole-connect", &identity, false));
    let client = TcpClient::new("192.0.2.1:1").with_connect_timeout(Duration::from_secs(30));
    let status = client.runtime_status_handle();
    transport.iface_manager().lock().await.spawn(client, TcpClient::spawn);

    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(
        status.to_json()["stream_state"].as_str(),
        Some("connecting"),
        "the connect to TEST-NET-1 must still be in flight (a fast failure would not exercise the cancel arm)"
    );
    assert_eq!(transport.interface_hashes().await.len(), 1, "client is attached before stop");

    timeout(Duration::from_secs(4), transport.stop())
        .await
        .expect("stop returns before the connect timeout and the stop budget");

    assert!(transport.interface_hashes().await.is_empty(), "stop detaches the client");
    assert_eq!(status.to_json()["stream_state"].as_str(), Some("closed"));
}
