fn is_terminal_outbound_status(status: &Value) -> bool {
    matches!(status["state_name"].as_str(), Some("delivered" | "rejected" | "cancelled" | "failed"))
}

#[test]
#[ignore = "requires local Python Reticulum/LXMF repos and daemon runtime"]
fn python_shared_instance_two_peer_relay_recovers_after_daemon_restart_e2e() {
    let lxmd_bin = resolve_test_binary("lxmd", option_env!("CARGO_BIN_EXE_lxmd"));
    let reticulumd_bin = resolve_test_binary("reticulumd", option_env!("CARGO_BIN_EXE_reticulumd"));
    let workspace_root =
        Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(3).expect("workspace root");

    let python_bin = env::var("LXMF_PYTHON_BIN").unwrap_or_else(|_| "python3".to_string());
    let reticulum_repo = env::var("RETICULUM_PY_REPO").unwrap_or_else(|_| {
        workspace_root.parent().expect("workspace parent").join("reticulum").display().to_string()
    });
    let lxmf_repo = env::var("LXMF_PY_REPO").unwrap_or_else(|_| {
        workspace_root.parent().expect("workspace parent").join("lxmf").display().to_string()
    });
    let helper_script = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("support")
        .join("python_lxmf_endpoint.py");

    assert!(Path::new(&reticulum_repo).exists(), "reticulum repo not found: {reticulum_repo}");
    assert!(Path::new(&lxmf_repo).exists(), "lxmf repo not found: {lxmf_repo}");
    assert!(helper_script.exists(), "python helper script not found: {}", helper_script.display());

    let temp = tempfile::tempdir().expect("tempdir");
    let shared_a = ReservedPort::reserve();
    let shared_b = ReservedPort::reserve();
    let relay_a_rpc = ReservedPort::reserve();
    let relay_a_transport = ReservedPort::reserve();
    let relay_b_rpc = ReservedPort::reserve();
    let relay_b_transport = ReservedPort::reserve();
    let python_control_a = ReservedPort::reserve();
    let python_control_b = ReservedPort::reserve();
    let rns_rpc_a = ReservedPort::reserve();
    let rns_rpc_b = ReservedPort::reserve();
    let shared_a_port = shared_a.port();
    let shared_b_port = shared_b.port();
    let relay_a_rpc_port = relay_a_rpc.port();
    let relay_a_transport_port = relay_a_transport.port();
    let relay_b_rpc_port = relay_b_rpc.port();
    let relay_b_transport_port = relay_b_transport.port();
    let python_control_a_port = python_control_a.port();
    let python_control_b_port = python_control_b.port();
    let rns_rpc_a_port = rns_rpc_a.port();
    let rns_rpc_b_port = rns_rpc_b.port();

    let relay_a_dir = temp.path().join("rust-shared-peer-relay-a");
    let relay_b_dir = temp.path().join("rust-shared-peer-relay-b");
    let python_storage_a = temp.path().join("python-shared-peer-a-storage");
    let python_storage_b = temp.path().join("python-shared-peer-b-storage");
    let python_rns_a = temp.path().join("python-shared-peer-a-rns");
    let python_rns_b = temp.path().join("python-shared-peer-b-rns");
    write_python_shared_instance_rns_config_with_control_port(
        &python_rns_a,
        shared_a_port,
        rns_rpc_a_port,
    );
    write_python_shared_instance_rns_config_with_control_port(
        &python_rns_b,
        shared_b_port,
        rns_rpc_b_port,
    );

    let relay_a_config = format!(
        "{}\n[reticulum]\nenable_transport = true\n",
        rust_node_config(
            "rust-shared-peer-relay-a",
            relay_a_rpc_port,
            Some(relay_a_transport_port),
            &[local_client_interface("python-shared-a", shared_a_port)],
        )
    );
    let relay_b_config = format!(
        "{}\n[reticulum]\nenable_transport = true\n",
        rust_node_config(
            "rust-shared-peer-relay-b",
            relay_b_rpc_port,
            Some(relay_b_transport_port),
            &[
                local_client_interface("python-shared-b", shared_b_port),
                tcp_client_interface("relay-a-uplink", relay_a_transport_port),
            ],
        )
    );
    write_rust_config(&relay_a_dir, &relay_a_config);
    write_rust_config(&relay_b_dir, &relay_b_config);

    let mut python_peer_a = Some(spawn_python_endpoint(
        &python_bin,
        &reticulum_repo,
        &lxmf_repo,
        &helper_script,
        "python-shared-peer-a",
        "Python shared peer A",
        &python_rns_a,
        &python_storage_a,
        python_control_a_port,
        &mut [shared_a, python_control_a, rns_rpc_a],
    ));
    let mut python_peer_b = Some(spawn_python_endpoint(
        &python_bin,
        &reticulum_repo,
        &lxmf_repo,
        &helper_script,
        "python-shared-peer-b",
        "Python shared peer B",
        &python_rns_b,
        &python_storage_b,
        python_control_b_port,
        &mut [shared_b, python_control_b, rns_rpc_b],
    ));
    let mut relay_a = None;
    let mut relay_b = None;
    let mut destination_hashes = None;

    let outcome: Result<(), String> = (|| {
        wait_for_python_endpoint_ready(
            python_control_a_port,
            python_peer_a.as_mut().expect("Python shared peer A"),
            "python-shared-peer-a",
        )?;
        wait_for_python_endpoint_ready(
            python_control_b_port,
            python_peer_b.as_mut().expect("Python shared peer B"),
            "python-shared-peer-b",
        )?;

        let python_status_a = python_control_call(python_control_a_port, "status", None)?;
        let python_status_b = python_control_call(python_control_b_port, "status", None)?;
        for (label, status) in [
            ("Python shared peer A", &python_status_a),
            ("Python shared peer B", &python_status_b),
        ] {
            if status
                .get("reticulum")
                .and_then(|reticulum| reticulum.get("is_shared_instance"))
                != Some(&Value::Bool(true))
            {
                return Err(format!("{label} did not own its shared instance: {status}"));
            }
        }
        let hash_a = python_status_a
            .get("delivery_destination_hash")
            .and_then(Value::as_str)
            .filter(|hash| !hash.is_empty())
            .ok_or_else(|| format!("Python peer A has no delivery hash: {python_status_a}"))?
            .to_string();
        let hash_b = python_status_b
            .get("delivery_destination_hash")
            .and_then(Value::as_str)
            .filter(|hash| !hash.is_empty())
            .ok_or_else(|| format!("Python peer B has no delivery hash: {python_status_b}"))?
            .to_string();
        destination_hashes = Some((hash_a.clone(), hash_b.clone()));

        relay_a = Some(spawn_lxmd(
            &lxmd_bin,
            &reticulumd_bin,
            relay_a_rpc_port,
            &relay_a_dir,
            &mut [relay_a_rpc, relay_a_transport],
        ));
        wait_for_ready(
            relay_a_rpc_port,
            relay_a.as_mut().expect("Rust relay A"),
            "rust-shared-peer-relay-a-before-restart",
        )?;
        require_attached_local_client(relay_a_rpc_port, "Rust relay A before restart")?;

        relay_b = Some(spawn_lxmd(
            &lxmd_bin,
            &reticulumd_bin,
            relay_b_rpc_port,
            &relay_b_dir,
            &mut [relay_b_rpc, relay_b_transport],
        ));
        wait_for_ready(
            relay_b_rpc_port,
            relay_b.as_mut().expect("Rust relay B"),
            "rust-shared-peer-relay-b-before-restart",
        )?;
        require_attached_local_client(relay_b_rpc_port, "Rust relay B before restart")?;
        wait_for_connected_tcp_client(relay_b_rpc_port, "relay-a-uplink")?;

        // Only the Python peers announce: a relay's own announce_now puts two
        // announces on the uplink at once, which trips the peer relay's announce
        // ingress control and holds the Python announce that follows.
        python_control_call(python_control_a_port, "announce", None)?;
        python_control_call(python_control_b_port, "announce", None)?;
        wait_for_known_path_without_announce(relay_a_rpc_port, &hash_b)?;
        wait_for_known_path_without_announce(relay_b_rpc_port, &hash_a)?;

        for (sender, receiver, destination, before, after) in [
            (
                python_control_a_port,
                python_control_b_port,
                &hash_b,
                "shared-a-to-b-before-restart",
                "shared-a-to-b-before-restart",
            ),
            (
                python_control_b_port,
                python_control_a_port,
                &hash_a,
                "shared-b-to-a-before-restart",
                "shared-b-to-a-before-restart",
            ),
        ] {
            python_control_call(
                sender,
                "wait_path",
                Some(json!({ "destination": destination, "timeout": 5.0 })),
            )?;
            python_control_call(
                sender,
                "send_message",
                Some(json!({ "destination": destination, "title": "", "content": before })),
            )?;
            python_control_call(
                receiver,
                "wait_message",
                Some(json!({ "content": after, "timeout": 30.0 })),
            )?;
        }

        python_control_call(
            python_control_a_port,
            "arm_outbound_packet_drop",
            Some(json!({ "destination": &hash_b })),
        )?;
        let queued_message = python_control_call(
            python_control_a_port,
            "send_message",
            Some(json!({
                "destination": &hash_b,
                "title": "",
                "content": "retried-a-to-b-across-restart",
                "wait_for_path": false,
                "method": "opportunistic"
            })),
        )?;
        let queued_message_hash = queued_message
            .get("message_hash")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("Python peer A returned no retry-test message hash: {queued_message}"))?
            .to_string();
        let dropped = python_control_call(
            python_control_a_port,
            "wait_outbound_packet_drop",
            Some(json!({ "timeout": 10.0 })),
        )?;
        if dropped.get("dropped_packets").and_then(Value::as_u64) != Some(1)
            || dropped.get("destination").and_then(Value::as_str) != Some(hash_b.as_str())
        {
            return Err(format!("first LXMF packet was not deterministically dropped: {dropped}"));
        }
        let queued_status = python_control_call(
            python_control_a_port,
            "outbound_status",
            Some(json!({ "message_hash": &queued_message_hash })),
        )?;
        if is_terminal_outbound_status(&queued_status)
            || queued_status.get("delivery_attempts").and_then(Value::as_u64) != Some(1)
        {
            return Err(format!(
                "faulted LXMF send did not remain retryable after exactly one attempt: {queued_status}"
            ));
        }

        terminate_child(&mut relay_a.as_mut().expect("Rust relay A").child);
        terminate_child(&mut relay_b.as_mut().expect("Rust relay B").child);
        relay_a = Some(spawn_lxmd(
            &lxmd_bin,
            &reticulumd_bin,
            relay_a_rpc_port,
            &relay_a_dir,
            &mut [],
        ));
        wait_for_ready(
            relay_a_rpc_port,
            relay_a.as_mut().expect("restarted Rust relay A"),
            "rust-shared-peer-relay-a-after-restart",
        )?;
        require_attached_local_client(relay_a_rpc_port, "Rust relay A after restart")?;
        // The replacement relay A pushes its restored paths to peer A as soon as
        // it attaches, and the LXMRouter retry can fire within seconds of the
        // drop, so the retry window opens here: from this point every outbound
        // packet for peer B can only leave through a replacement relay.
        python_control_call(python_control_a_port, "mark_outbound_retry_window", None)?;
        relay_b = Some(spawn_lxmd(
            &lxmd_bin,
            &reticulumd_bin,
            relay_b_rpc_port,
            &relay_b_dir,
            &mut [],
        ));
        wait_for_ready(
            relay_b_rpc_port,
            relay_b.as_mut().expect("restarted Rust relay B"),
            "rust-shared-peer-relay-b-after-restart",
        )?;
        require_attached_local_client(relay_b_rpc_port, "Rust relay B after restart")?;
        wait_for_connected_tcp_client(relay_b_rpc_port, "relay-a-uplink")?;

        python_control_call(python_control_a_port, "announce", None)?;
        python_control_call(python_control_b_port, "announce", None)?;
        for (rpc_port, label) in [
            (relay_a_rpc_port, "Rust relay A relearned"),
            (relay_b_rpc_port, "Rust relay B relearned"),
        ] {
            for destination in [&hash_a, &hash_b] {
                require_known_path(rpc_port, destination, label)?;
            }
        }
        let retry_packet = python_control_call(
            python_control_a_port,
            "wait_outbound_retry_packet",
            Some(json!({ "timeout": 30.0 })),
        )?;
        if retry_packet.get("transmitted_packets").and_then(Value::as_u64).unwrap_or(0) == 0
            || retry_packet.get("destination").and_then(Value::as_str) != Some(hash_b.as_str())
        {
            return Err(format!(
                "LXMF retry did not traverse the replacement Rust relays: {retry_packet}"
            ));
        }

        python_control_call(
            python_control_b_port,
            "wait_message",
            Some(json!({
                "content": "retried-a-to-b-across-restart",
                "timeout": 45.0
            })),
        )?;
        let delivered = python_control_call(
            python_control_a_port,
            "wait_outbound_state",
            Some(json!({
                "message_hash": &queued_message_hash,
                "state": "delivered",
                "timeout": 45.0
            })),
        )?;
        let attempts = delivered.get("delivery_attempts").and_then(Value::as_u64).unwrap_or(0);
        if attempts < 2 {
            return Err(format!(
                "LXMF delivery completed without retrying the faulted first attempt: {delivered}"
            ));
        }
        let inbox = python_control_call(python_control_b_port, "list_messages", None)?;
        let matching = inbox
            .get("messages")
            .and_then(Value::as_array)
            .map(|messages| {
                messages
                    .iter()
                    .filter(|message| {
                        message.get("content").and_then(Value::as_str)
                            == Some("retried-a-to-b-across-restart")
                    })
                    .count()
            })
            .unwrap_or_default();
        if matching != 1 {
            return Err(format!(
                "retried LXMF message was delivered {matching} times instead of exactly once: {inbox}"
            ));
        }

        for (sender, receiver, destination, content) in [
            (
                python_control_a_port,
                python_control_b_port,
                &hash_b,
                "raw-a-to-b-after-restart",
            ),
            (
                python_control_b_port,
                python_control_a_port,
                &hash_a,
                "raw-b-to-a-after-restart",
            ),
        ] {
            python_control_call(
                sender,
                "wait_path",
                Some(json!({ "destination": destination, "timeout": 10.0 })),
            )?;
            python_control_call(
                sender,
                "open_raw_link",
                Some(json!({ "destination": destination, "timeout": 20.0 })),
            )?;
            python_control_call(sender, "send_raw", Some(json!({ "content": content })))?;
            python_control_call(
                receiver,
                "wait_raw_message",
                Some(json!({ "content": content, "timeout": 10.0 })),
            )?;
        }

        let python_link_a = python_control_call(python_control_a_port, "raw_link_status", None)?;
        let python_link_b = python_control_call(python_control_b_port, "raw_link_status", None)?;
        if python_link_a.get("status_name").and_then(Value::as_str) != Some("active") {
            return Err(format!("Python peer A raw link was not active: {python_link_a}"));
        }
        if python_link_b.get("status_name").and_then(Value::as_str) != Some("active") {
            return Err(format!("Python peer B raw link was not active: {python_link_b}"));
        }
        Ok(())
    })();

    let failure_details = if let Err(err) = &outcome {
        let route_diagnostics = if let Some((hash_a, hash_b)) = destination_hashes.as_ref() {
            [("Rust relay A", relay_a_rpc_port), ("Rust relay B", relay_b_rpc_port)]
                .into_iter()
                .map(|(label, rpc_port)| {
                    let route_a =
                        rpc_snapshot(rpc_port, "path_status", Some(json!({ "destination": hash_a })));
                    let route_b =
                        rpc_snapshot(rpc_port, "path_status", Some(json!({ "destination": hash_b })));
                    let interface_traffic = rpc_call(rpc_port, "daemon_status_ex", None)
                        .map(|status| {
                            status
                                .get("reticulum")
                                .and_then(|reticulum| reticulum.get("transport"))
                                .and_then(|transport| transport.get("interfaces"))
                                .cloned()
                                .unwrap_or(Value::Null)
                                .to_string()
                        })
                        .unwrap_or_else(|error| format!("rpc error: {error}"));
                    format!(
                        "{label} path A: {route_a}\n{label} path B: {route_b}\n{label} interface traffic: {interface_traffic}"
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        } else {
            "Python destination hashes were unavailable".to_string()
        };
        Some(format!(
            "{err}\n\n{route_diagnostics}\n\n{}\n\n{}\n\n{}\n\n{}",
            collect_node_diagnostics("Rust relay A", relay_a_rpc_port, relay_a.as_mut()),
            collect_node_diagnostics("Rust relay B", relay_b_rpc_port, relay_b.as_mut()),
            collect_python_endpoint_diagnostics(
                "python-shared-peer-a",
                python_control_a_port,
                python_peer_a.as_mut(),
            ),
            collect_python_endpoint_diagnostics(
                "python-shared-peer-b",
                python_control_b_port,
                python_peer_b.as_mut(),
            ),
        ))
    } else {
        None
    };

    if let Some(node) = relay_a.as_mut() {
        terminate_child(&mut node.child);
    }
    if let Some(node) = relay_b.as_mut() {
        terminate_child(&mut node.child);
    }
    if let Some(node) = python_peer_a.as_mut() {
        terminate_child(&mut node.child);
    }
    if let Some(node) = python_peer_b.as_mut() {
        terminate_child(&mut node.child);
    }

    if let Some(details) = failure_details {
        panic!("Python/Rust multi-peer daemon-replacement flow failed:\n{details}");
    }
}
