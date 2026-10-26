const ALLOW_LIST_REQUEST_PATH: &str = "/test/request";

/// The Python endpoint registers its request handler with `ALLOW_LIST` and an
/// empty `allowed_list` that it fills from `set_remote_identified_callback`
/// (`RNS/Link.py:1248`). `register_request_handler` stores the list by
/// reference (`RNS/Destination.py:397`) and `Link.handle_request` consults it
/// when each request arrives (`RNS/Link.py:820-822`), so the same link is
/// refused before identifying and answered after. A refused request draws no
/// reply: the `else` branch at `RNS/Link.py:853-855` only logs at DEBUG.
#[tokio::test]
#[ignore = "requires local Python Reticulum checkout"]
async fn rust_identify_python_allow_list_answers_only_the_identified_peer() {
    let _interop_guard = python_interop_guard().await;
    run_rust_identify_python_allow_list_answers_only_the_identified_peer().await;
}

#[deny(clippy::await_holding_invalid_type)]
async fn run_rust_identify_python_allow_list_answers_only_the_identified_peer() {
    let paths = python_channel_interop_paths();

    let server_port = free_tcp_port();
    let temp = tempfile::tempdir().expect("tempdir");
    let py_config_dir = temp.path().join("python-rns-allow-list-identify");
    fs::create_dir_all(&py_config_dir).expect("python config dir");
    write_python_config(&py_config_dir, server_port);

    let mut child =
        paths.spawn_endpoint_with_args(&py_config_dir, "request", &["--allow-identified"]);
    let ready = read_ready(&mut child).expect("python endpoint ready");
    let _guard = ChildGuard { child: Some(child) };
    wait_for_port(server_port, Duration::from_secs(5)).await;

    let target_hash =
        AddressHash::new_from_hex_string(&ready.destination_hash).expect("destination hash");
    let rust_identity = PrivateIdentity::new_from_rand(OsRng);
    let rust_identity = to_transport_private_identity(&rust_identity);
    let mut config =
        TransportConfig::new("python-allow-list-identify-interop", &rust_identity, true);
    config.set_path_request_timeout_secs(2);
    let transport = Transport::new(config);
    transport
        .iface_manager()
        .lock()
        .await
        .spawn(TcpClient::new(format!("127.0.0.1:{server_port}")), TcpClient::spawn);

    let destination = wait_for_announce(&transport, target_hash, Duration::from_secs(8)).await;
    let mut link_events = transport.out_link_events();
    let link = transport.link(destination).await.expect("link request sent");
    let link_id = wait_for_out_link_active(&mut link_events, &link, Duration::from_secs(8)).await;
    let mut received = transport.received_data_events();
    sleep(Duration::from_millis(100)).await;

    let seen = Arc::new(StdMutex::new(Vec::<(String, String)>::new()));
    let seen_clone = seen.clone();
    transport
        .channel(link_id)
        .register_handler(MSG_TYPE, move |envelope| {
            if let Ok(decoded) = rmp_serde::from_slice::<(String, String)>(&envelope.payload) {
                seen_clone.lock().expect("seen lock").push(decoded);
                true
            } else {
                false
            }
        })
        .await
        .expect("register channel handler");

    let request_id = send_allow_list_request(&transport, &link, "before-identify").await;
    // The inner wait panics on its own deadline, so the outer one must fire first.
    let refused = tokio::time::timeout(
        Duration::from_secs(3),
        wait_for_request_response(&mut received, link_id, request_id, Duration::from_secs(8)),
    )
    .await;
    assert!(refused.is_err(), "Python answered a request from an unidentified peer");

    let payload = build_link_identify_payload(&rust_identity, &link_id);
    send_link_context_packet(&transport, &link, PacketContext::LinkIdentify, &payload)
        .await
        .expect("send link identify");
    wait_for_identify_ack(&seen, Duration::from_secs(8)).await;

    let request_id = send_allow_list_request(&transport, &link, "after-identify").await;
    let response =
        wait_for_request_response(&mut received, link_id, request_id, Duration::from_secs(8)).await;
    assert_eq!(rmpv_to_string(&response).as_deref(), Some("reply:after-identify"));
}

async fn send_allow_list_request(
    transport: &Transport,
    link: &Arc<tokio::sync::Mutex<rns_transport::destination::link::Link>>,
    text: &str,
) -> [u8; 16] {
    let payload =
        build_link_request_payload(ALLOW_LIST_REQUEST_PATH, rmpv::Value::String(text.into()))
            .expect("request payload");
    send_link_context_packet(transport, link, PacketContext::Request, &payload)
        .await
        .expect("send request")
        .expect("request id")
}
