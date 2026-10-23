    /// Every adopted NIC gets its own receive loop, and all of them share one
    /// `state` and one `dedupe` mutex. Each loop must take the pair in one total
    /// order (`state`, then `dedupe`); any form that can hold one guard while
    /// waiting on the other lets two loops wait on each other, and the loops
    /// stall under concurrent traffic on a multi-thread runtime.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn peer_data_receive_loops_on_several_sockets_keep_making_progress_under_load() {
        const SOCKETS: usize = 4;
        const DATAGRAMS_PER_SOCKET: usize = 100;
        let listeners = (0..SOCKETS)
            .map(|_| AutoDataListenerBinding {
                ifname: "lo".to_string(),
                link_local_address: "127.0.0.1".to_string(),
                bind_address: "127.0.0.1".to_string(),
                bind_port: 0,
            })
            .collect();
        let plan = plan_with_data_listeners(listeners);
        let sockets = plan
            .bind_data_sockets(|_| panic!("IPv4 data bind is unscoped"))
            .await
            .expect("bind peer data sockets");
        let bind_addrs: Vec<_> = sockets.iter().map(|socket| socket.bind_addr).collect();
        let state = Arc::new(tokio::sync::Mutex::new(plan.discovery_state()));
        let dedupe = Arc::new(tokio::sync::Mutex::new(AutoInboundPacketDeduplicator::from_timing(
            AutoInterfaceTiming::for_platform(AutoInterfacePlatform::Other),
        )));
        let (events_tx, mut events_rx) = tokio::sync::mpsc::channel(1024);
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        let handles = plan.spawn_peer_data_receive_loops(
            sockets,
            Arc::clone(&state),
            Arc::clone(&dedupe),
            None,
            events_tx,
            shutdown_rx,
        );
        let sender =
            Arc::new(tokio::net::UdpSocket::bind("127.0.0.1:0").await.expect("bind sender"));
        let source_address = sender.local_addr().expect("sender addr").ip().to_string();
        // Register the sender so every datagram takes the Accepted path and touches
        // dedupe under the nest.
        state.lock().await.observe_discovery_packet(
            &source_address,
            "lo",
            core::time::Duration::ZERO,
        );

        let senders: Vec<_> = bind_addrs
            .into_iter()
            .enumerate()
            .map(|(socket_index, bind_addr)| {
                let sender = Arc::clone(&sender);
                tokio::spawn(async move {
                    for n in 0..DATAGRAMS_PER_SOCKET {
                        let payload = format!("{socket_index}:{n}");
                        sender
                            .send_to(payload.as_bytes(), bind_addr)
                            .await
                            .expect("send peer data datagram");
                        tokio::task::yield_now().await;
                    }
                })
            })
            .collect();

        let expected = SOCKETS * DATAGRAMS_PER_SOCKET;
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            let mut processed = 0;
            while processed < expected {
                match events_rx.recv().await.expect("events channel stays open") {
                    AutoPeerDataLoopEvent::Processed(_) => processed += 1,
                    other => panic!("unexpected peer-data event: {other:?}"),
                }
            }
        })
        .await
        .expect(
            "every datagram is processed within 2 s: the receive loops did not drain (a deadlock \
             on state/dedupe, or a loopback drop)",
        );

        for sender in senders {
            sender.await.expect("sender task");
        }
        shutdown_tx.send(true).expect("send shutdown");
        for handle in handles {
            tokio::time::timeout(std::time::Duration::from_secs(2), handle)
                .await
                .expect("peer data loop shutdown timeout")
                .expect("peer data loop task");
        }
    }
