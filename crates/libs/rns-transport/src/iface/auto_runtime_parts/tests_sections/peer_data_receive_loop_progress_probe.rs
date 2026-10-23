    /// Busy threads, one per logical CPU, that keep the OS scheduler
    /// preempting the runtime's workers for as long as the guard lives.
    /// Preemption inside the window between a loop's two lock acquisitions is
    /// what turns the cross-thread race below from rare into frequent; the
    /// guard stops and joins the threads on drop, so a panicking test cannot
    /// leak them.
    struct CpuPressure {
        stop: Arc<std::sync::atomic::AtomicBool>,
        threads: Vec<std::thread::JoinHandle<()>>,
    }

    impl CpuPressure {
        fn start() -> Self {
            let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
            let cpus = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
            let threads = (0..cpus)
                .map(|_| {
                    let stop = Arc::clone(&stop);
                    std::thread::spawn(move || {
                        let mut spin = 0_u64;
                        while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                            spin = std::hint::black_box(spin.wrapping_add(1));
                        }
                    })
                })
                .collect();
            Self { stop, threads }
        }
    }

    impl Drop for CpuPressure {
        fn drop(&mut self) {
            self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
            for thread in self.threads.drain(..) {
                let _ = thread.join();
            }
        }
    }

    /// Repeated-trial twin of
    /// `peer_data_receive_loops_on_several_sockets_keep_making_progress_under_load`.
    /// The only way two receive loops can wait on each other is a cross-thread
    /// race inside one task poll: one loop has just taken `state` and has not
    /// yet polled `dedupe` while a sibling on another worker finds `state`
    /// held and takes `dedupe`. tokio's semaphore hands permits to queued
    /// waiters in order, so the window is a few instructions wide and one
    /// burst on an idle machine hits it only a fraction of the time; a worker
    /// preempted inside it widens the window to a scheduler slice. Each round
    /// here binds eight fresh sockets, queues a backlog on every one before
    /// its loop starts, then feeds them live with no yield between sends,
    /// under CPU pressure from one busy thread per core; the rounds are
    /// independent trials, so an acquisition shape that can hold one guard
    /// while pending on the other (a `join!` over the two lock futures) stalls
    /// one of them almost surely, while the sequential `state`-then-`dedupe`
    /// order finishes every round well inside its two-second budget.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn peer_data_receive_loops_keep_making_progress_across_repeated_bursts() {
        const ROUNDS: usize = 30;
        let pressure = CpuPressure::start();
        let mut stalled = Vec::new();
        for round in 0..ROUNDS {
            if !drain_burst(round).await {
                stalled.push(round);
            }
        }
        drop(pressure);
        assert!(
            stalled.is_empty(),
            "every burst drains within 2 s and its loops shut down; the receive loops must not \
             deadlock on state/dedupe (stalled rounds: {stalled:?})"
        );
    }

    /// Eight loops on one `state`/`dedupe` pair, every socket pre-loaded with
    /// a backlog before its loop starts, then fed live. Returns whether every
    /// datagram was processed within two seconds and every loop then shut
    /// down (a wedged loop never observes the shutdown signal).
    async fn drain_burst(round: usize) -> bool {
        const SOCKETS: usize = 8;
        const BACKLOG_PER_SOCKET: usize = 96;
        const LIVE_PER_SOCKET: usize = 150;
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

        // One sender per target, each a source the state already knows, and a
        // backlog queued in every socket before any loop exists.
        let mut senders = Vec::with_capacity(SOCKETS);
        for bind_addr in &bind_addrs {
            let sender = tokio::net::UdpSocket::bind("127.0.0.1:0").await.expect("bind sender");
            let source_address = sender.local_addr().expect("sender addr").ip().to_string();
            state.lock().await.observe_discovery_packet(
                &source_address,
                "lo",
                core::time::Duration::ZERO,
            );
            for n in 0..BACKLOG_PER_SOCKET {
                let payload = format!("backlog {round} {bind_addr}:{n}");
                sender.send_to(payload.as_bytes(), bind_addr).await.expect("queue backlog datagram");
            }
            senders.push(sender);
        }

        let (events_tx, mut events_rx) = tokio::sync::mpsc::channel(4096);
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        let handles = plan.spawn_peer_data_receive_loops(
            sockets,
            Arc::clone(&state),
            Arc::clone(&dedupe),
            None,
            events_tx,
            shutdown_rx,
        );
        assert_eq!(handles.len(), SOCKETS, "one receive loop per bound data socket");

        let live: Vec<_> = senders
            .into_iter()
            .zip(bind_addrs)
            .map(|(sender, bind_addr)| {
                tokio::spawn(async move {
                    for n in 0..LIVE_PER_SOCKET {
                        let payload = format!("live {round} {bind_addr}:{n}");
                        sender
                            .send_to(payload.as_bytes(), bind_addr)
                            .await
                            .expect("send live datagram");
                    }
                })
            })
            .collect();

        let expected = SOCKETS * (BACKLOG_PER_SOCKET + LIVE_PER_SOCKET);
        let drained = tokio::time::timeout(std::time::Duration::from_secs(2), async {
            let mut processed = 0;
            while processed < expected {
                match events_rx.recv().await.expect("events channel stays open") {
                    AutoPeerDataLoopEvent::Processed(_) => processed += 1,
                    other => panic!("unexpected peer-data event: {other:?}"),
                }
            }
        })
        .await
        .is_ok();

        for sender in live {
            sender.abort();
        }
        shutdown_tx.send(true).expect("send shutdown");
        let mut shut_down = true;
        for handle in handles {
            if tokio::time::timeout(std::time::Duration::from_millis(500), handle).await.is_err() {
                shut_down = false;
            }
        }
        if drained && shut_down {
            // Both mutexes are free once every loop has returned.
            assert!(state.try_lock().is_ok(), "no loop still holds the state guard");
            assert!(dedupe.try_lock().is_ok(), "no loop still holds the dedupe guard");
        }
        drained && shut_down
    }
