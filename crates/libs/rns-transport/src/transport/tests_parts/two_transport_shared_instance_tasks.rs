// Pins that `stop` joins the tasks the shared-instance client spawns: the registration
// announce still waiting out its quarter second when the stop runs sees the cancel and
// never reaches the shared instance, and the reconnect consumer of an attach ends with the
// stop; the task set is empty either way. A task that ended by panic is reported once at
// warn under the transport's name and the stop still completes, and so is a task aborted
// through its own handle, the other join error a stop can observe. Also pins the test
// logger's shared-instance store: a warning on the links target is kept whether or not it
// names the shared instance. Helpers come from the sibling `tests_parts` files included
// into this module.

/// Three times the registration announce delay, so an announce the stop failed to cancel
/// would have reached the shared instance by the time the receiver is drained.
const SHARED_INSTANCE_TASKS_ANNOUNCE_SILENCE: Duration =
    super::links::SHARED_INSTANCE_REGISTRATION_ANNOUNCE_DELAY.saturating_mul(3);

/// How many shared-instance tasks `transport` tracks right now.
fn shared_instance_task_count(transport: &Transport) -> usize {
    transport.shared_instance_tasks.lock().expect("shared-instance task set").len()
}

/// Asserts that no announce for `address_hash` is waiting on `announces`.
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_tasks_assert_not_announced(
    announces: &mut broadcast::Receiver<AnnounceEvent>,
    address_hash: &AddressHash,
) {
    loop {
        match announces.try_recv() {
            Ok(event) => {
                let announced = event.destination.lock().await.desc.address_hash;
                assert_ne!(announced, *address_hash, "the destination was announced");
            }
            Err(TryRecvError::Lagged(_)) => continue,
            Err(TryRecvError::Empty | TryRecvError::Closed) => return,
        }
    }
}

/// Fork-only: a destination registered while attached schedules its announce a quarter
/// second out; a stop inside that delay joins the task and leaves the set empty, and the
/// announce never reaches the shared instance.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_stop_joins_the_pending_registration_announce() {
    install_test_logger();
    let name = "shared-instance-tasks-announce";
    let server = shared_instance_server().await;
    let mut server_announces = server.transport.recv_announces().await;
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let transport = Transport::new(TransportConfig::new(name, &identity, false));
    let iface = transport
        .attach_shared_instance(server.endpoint(), SharedInstanceOptions::default())
        .await
        .expect("attach to the shared instance");
    server.await_accepted_connections(1).await;
    shared_instance_assert_attached(&transport, iface).await;
    assert_eq!(shared_instance_task_count(&transport), 1, "the attach's consumer is tracked");

    let destination = transport
        .add_destination(
            PrivateIdentity::new_from_rand(OsRng),
            DestinationName::new("shared-instance-tasks", "announce"),
        )
        .await;
    let address_hash = destination.lock().await.desc.address_hash;
    assert_eq!(shared_instance_task_count(&transport), 2, "the registration announce is tracked");

    timeout(STOP_TIMEOUT, transport.stop()).await.expect("client stop completes");
    assert_eq!(shared_instance_task_count(&transport), 0, "stop joined every task");
    shared_instance_assert_released(&transport, iface).await;

    tokio::time::sleep(SHARED_INSTANCE_TASKS_ANNOUNCE_SILENCE).await;
    shared_instance_tasks_assert_not_announced(&mut server_announces, &address_hash).await;
    shared_instance_assert_never_warned(name);
    timeout(STOP_TIMEOUT, server.transport.stop()).await.expect("shared instance stop completes");
}

/// The never-warn oracle rests on the logger keeping every record at `Warn` or above from
/// the links target, needle or not; a warning without the words `shared instance` is
/// stored at `Warn`, while a debug record without them is not stored at all.
#[test]
fn shared_instance_test_logger_keeps_a_needle_less_warning() {
    install_test_logger();
    let name = "shared-instance-tasks-logger";
    log::warn!(
        target: "rns_transport::transport::links",
        "tp({name}): synthetic warning without the needle"
    );
    log::debug!(
        target: "rns_transport::transport::links",
        "tp({name}): synthetic debug record without the needle"
    );
    let records = shared_instance_records_for(name);
    assert_eq!(
        records,
        vec![(log::Level::Warn, format!("tp({name}): synthetic warning without the needle"))],
        "the warning is kept and the debug record is not"
    );
}

/// Fork-only: the consumer spawned by an attach ends with the stop that cancels the
/// transport, so the set is empty afterwards with nothing left to abort.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_stop_joins_the_reconnect_consumer() {
    install_test_logger();
    let name = "shared-instance-tasks-consumer";
    let server = shared_instance_server().await;
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let transport = Transport::new(TransportConfig::new(name, &identity, false));
    let iface = transport
        .attach_shared_instance(server.endpoint(), SharedInstanceOptions::default())
        .await
        .expect("attach to the shared instance");
    server.await_accepted_connections(1).await;
    shared_instance_assert_attached(&transport, iface).await;
    assert_eq!(shared_instance_task_count(&transport), 1, "the attach's consumer is tracked");

    timeout(STOP_TIMEOUT, transport.stop()).await.expect("client stop completes");
    assert_eq!(shared_instance_task_count(&transport), 0, "stop joined the consumer");
    shared_instance_assert_released(&transport, iface).await;
    shared_instance_assert_never_warned(name);
    timeout(STOP_TIMEOUT, server.transport.stop()).await.expect("shared instance stop completes");
}

/// Best-effort tasks never block the shutdown: a shared-instance task that ended by panic
/// is reported once at warn under the transport's name when `stop` joins it, and the stop
/// still completes inside its budget. Spawned straight into the set, with no attach, so no
/// later spawn reaps the finished task first.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_stop_warns_once_for_a_task_that_ended_by_panic_and_still_completes() {
    install_test_logger();
    let name = "shared-instance-tasks-panic";
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let transport = Transport::new(TransportConfig::new(name, &identity, false));
    transport
        .shared_instance_tasks
        .lock()
        .expect("shared-instance task set")
        .spawn(async { panic!("synthetic shared-instance task panic") });
    tokio::time::sleep(LOOPBACK_POLL).await;
    assert_eq!(shared_instance_task_count(&transport), 1, "the panicked task is still tracked");

    timeout(STOP_TIMEOUT, transport.stop())
        .await
        .expect("stop completes with a panicked task in the set");
    assert_eq!(shared_instance_task_count(&transport), 0, "stop drained the set");

    let warned: Vec<_> = shared_instance_records_for(name)
        .into_iter()
        .filter(|(level, record)| {
            *level == log::Level::Warn
                && record.contains("shared instance task did not finish cleanly during stop")
        })
        .collect();
    assert_eq!(warned.len(), 1, "one warning per panicked task, under tp({name}): {warned:?}");
    assert!(
        warned[0].1.starts_with(&format!("tp({name}): ")),
        "the warning opens with the transport's name: {}",
        warned[0].1
    );
}

/// A shared-instance task aborted by a holder of its handle joins as a cancelled
/// `JoinError`; `stop` reports it once at warn, naming the cancellation, and drains the
/// set. The in-crate spawner drops the handle, so this is the path a panic does not cover.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_stop_warns_once_for_a_task_aborted_through_its_handle() {
    install_test_logger();
    let name = "shared-instance-tasks-abort";
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let transport = Transport::new(TransportConfig::new(name, &identity, false));
    let handle = transport
        .shared_instance_tasks
        .lock()
        .expect("shared-instance task set")
        .spawn(std::future::pending::<()>());
    handle.abort();
    tokio::time::sleep(LOOPBACK_POLL).await;
    assert_eq!(shared_instance_task_count(&transport), 1, "the aborted task is still tracked");

    timeout(STOP_TIMEOUT, transport.stop())
        .await
        .expect("stop completes with an aborted task in the set");
    assert_eq!(shared_instance_task_count(&transport), 0, "stop drained the set");

    let warned: Vec<_> = shared_instance_records_for(name)
        .into_iter()
        .filter(|(level, record)| {
            *level == log::Level::Warn
                && record.contains("shared instance task did not finish cleanly during stop")
        })
        .collect();
    assert_eq!(warned.len(), 1, "one warning per aborted task, under tp({name}): {warned:?}");
    assert!(
        warned[0].1.contains("cancelled"),
        "the warning carries tokio's cancellation reason: {}",
        warned[0].1
    );
}
