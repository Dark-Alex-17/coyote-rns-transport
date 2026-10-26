// Loopback coverage for what a request handler cannot do to the transport: a handler that
// sleeps or panics delays no other request on the same link and leaves the transport
// running, and a full dispatch queue drops the request with a debug record instead of
// blocking the link. Helpers come from the sibling `tests_parts` files.

use std::sync::Once;

/// Longer than any test here waits for anything, so a handler stuck in it can only be seen
/// through its effect on other requests.
const SLOW_HANDLER_SLEEP: Duration = Duration::from_secs(30);

/// Keeps the link's drop records, from the links target every record at `Warn` or above
/// plus the debug lines of the shared-instance client, and from the resource target the
/// dropped-outcome-receiver lines and the rejected-advertisement warnings, from the link
/// target the closed-link packet warnings, from the request-dispatch target the responses
/// dropped for a closed link, from the jobs target the resource-retry diagnostics, and from
/// the packet disk cache the warnings about hashlist orphans the sweep could not remove.
/// Tests share one process and
/// `log::set_logger` installs once, so there must be exactly one logger in the binary and
/// this is it; a record is attributed to a test by the link id, resource hash or
/// `tp(<name>)` it names, and the quieter levels are kept only for the shared-instance,
/// dropped-receiver, closed-link-response and resource-diag lines so the stores stay small.
struct TestLogger {
    dropped_requests: StdMutex<Vec<String>>,
    shared_instance: StdMutex<Vec<(log::Level, String)>>,
    outcome_receiver_dropped: StdMutex<Vec<String>>,
    rejected_advertisement_warns: StdMutex<Vec<(log::Level, String)>>,
    closed_link_packet_warnings: StdMutex<Vec<String>>,
    closed_link_responses: StdMutex<Vec<String>>,
    resource_diag: StdMutex<Vec<String>>,
    hashlist_orphan_warnings: StdMutex<Vec<String>>,
}

static TEST_LOG: TestLogger = TestLogger {
    dropped_requests: StdMutex::new(Vec::new()),
    shared_instance: StdMutex::new(Vec::new()),
    outcome_receiver_dropped: StdMutex::new(Vec::new()),
    rejected_advertisement_warns: StdMutex::new(Vec::new()),
    closed_link_packet_warnings: StdMutex::new(Vec::new()),
    closed_link_responses: StdMutex::new(Vec::new()),
    resource_diag: StdMutex::new(Vec::new()),
    hashlist_orphan_warnings: StdMutex::new(Vec::new()),
};

/// The target `attach_shared_instance` and `detach_shared_instance` log under: their
/// section file is `include!`d, via `links_parts/transport.rs`, into the links module.
const SHARED_INSTANCE_LOG_TARGET: &str = "rns_transport::transport::links";

/// The target `conclude_outbound` logs under: `resource/outcome.rs` is `include!`d into
/// the resource module.
const RESOURCE_LOG_TARGET: &str = "rns_transport::resource";

/// The target the responder side of the request layer logs under.
const REQUEST_DISPATCH_LOG_TARGET: &str = "rns_transport::transport::request_dispatch";

/// The target the resource-retry worker logs its `[resource-diag]` lines under.
const JOBS_LOG_TARGET: &str = "rns_transport::transport::jobs";

/// The target the packet hashlist orphan sweep warns under.
const PACKET_DISK_CACHE_LOG_TARGET: &str = "rns_transport::transport::packet_disk_cache";

impl log::Log for TestLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.target().starts_with("rns_transport::destination::link")
            || metadata.target() == SHARED_INSTANCE_LOG_TARGET
            || metadata.target() == RESOURCE_LOG_TARGET
            || metadata.target() == REQUEST_DISPATCH_LOG_TARGET
            || metadata.target() == JOBS_LOG_TARGET
            || metadata.target() == PACKET_DISK_CACHE_LOG_TARGET
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let message = record.args().to_string();
        if record.target() == SHARED_INSTANCE_LOG_TARGET {
            if record.level() <= log::Level::Warn || message.contains("shared instance") {
                self.shared_instance
                    .lock()
                    .expect("shared-instance record store")
                    .push((record.level(), message));
            }
        } else if record.target() == RESOURCE_LOG_TARGET {
            if message.contains("outcome receiver dropped") {
                self.outcome_receiver_dropped
                    .lock()
                    .expect("dropped-outcome-receiver record store")
                    .push(message);
            } else if message.contains("rejecting resource advertisement over inbound limits") {
                self.rejected_advertisement_warns
                    .lock()
                    .expect("rejected-advertisement record store")
                    .push((record.level(), message));
            }
        } else if record.target() == REQUEST_DISPATCH_LOG_TARGET {
            if message.contains("closed before transmit") || message.contains("is gone") {
                self.closed_link_responses
                    .lock()
                    .expect("closed-link response record store")
                    .push(message);
            }
        } else if record.target() == JOBS_LOG_TARGET {
            if message.contains("[resource-diag]") {
                self.resource_diag.lock().expect("resource-diag record store").push(message);
            }
        } else if record.target() == PACKET_DISK_CACHE_LOG_TARGET {
            if record.level() == log::Level::Warn && message.contains("packet hashlist orphan") {
                self.hashlist_orphan_warnings
                    .lock()
                    .expect("hashlist orphan warning store")
                    .push(message);
            }
        } else if message.contains("dropped, dispatch") {
            self.dropped_requests.lock().expect("dropped-request record store").push(message);
        } else if message.contains("can't create data packet") {
            self.closed_link_packet_warnings
                .lock()
                .expect("closed-link packet warning store")
                .push(message);
        }
    }

    fn flush(&self) {}
}

/// Installs the binary's one logger at `Debug`; every test that reads records calls this
/// first, and the `Once` makes the order between them irrelevant.
fn install_test_logger() {
    static INSTALL: Once = Once::new();
    INSTALL.call_once(|| {
        log::set_logger(&TEST_LOG).expect("no other logger is installed in this test binary");
        log::set_max_level(log::LevelFilter::Debug);
    });
}

fn dropped_request_records_for(link_id: AddressHash) -> Vec<String> {
    let needle = link_id.to_string();
    TEST_LOG
        .dropped_requests
        .lock()
        .expect("dropped-request record store")
        .iter()
        .filter(|record| record.contains(&needle))
        .cloned()
        .collect()
}

/// The shared-instance records a transport named `transport_name` emitted so far, oldest
/// first, with the level each was logged at.
fn shared_instance_records_for(transport_name: &str) -> Vec<(log::Level, String)> {
    let needle = format!("tp({transport_name})");
    TEST_LOG
        .shared_instance
        .lock()
        .expect("shared-instance record store")
        .iter()
        .filter(|(_, record)| record.contains(&needle))
        .cloned()
        .collect()
}

/// Every dropped-outcome-receiver record emitted so far, oldest first.
fn outcome_receiver_dropped_records() -> Vec<String> {
    TEST_LOG.outcome_receiver_dropped.lock().expect("dropped-outcome-receiver record store").clone()
}

/// The rejected-advertisement records the resource manager emitted for `link_id` so far,
/// oldest first, with the level each was logged at.
fn rejected_advertisement_records_for(link_id: AddressHash) -> Vec<(log::Level, String)> {
    let needle = link_id.to_string();
    TEST_LOG
        .rejected_advertisement_warns
        .lock()
        .expect("rejected-advertisement record store")
        .iter()
        .filter(|(_, record)| record.contains(&needle))
        .cloned()
        .collect()
}

/// How many `can't create data packet for closed link` warnings the link target has
/// emitted so far; the message names no link, so a test reads the count before and after
/// the call it probes.
fn closed_link_packet_warning_count() -> usize {
    TEST_LOG.closed_link_packet_warnings.lock().expect("closed-link packet warning store").len()
}

/// The responses the request layer dropped because `link_id` closed before transmit or
/// had already been swept from the link tables when the handler answered.
fn closed_link_response_records_for(link_id: AddressHash) -> Vec<String> {
    let needle = link_id.to_string();
    TEST_LOG
        .closed_link_responses
        .lock()
        .expect("closed-link response record store")
        .iter()
        .filter(|record| record.contains(&needle))
        .cloned()
        .collect()
}

/// The `[resource-diag]` records the retry worker emitted for `link_id` so far, oldest
/// first.
fn resource_diag_records_for(link_id: AddressHash) -> Vec<String> {
    let needle = link_id.to_string();
    TEST_LOG
        .resource_diag
        .lock()
        .expect("resource-diag record store")
        .iter()
        .filter(|record| record.contains(&needle))
        .cloned()
        .collect()
}

/// The warnings the packet hashlist sweep emitted so far for the orphan at `orphan`, oldest
/// first; the path is unique per test storage, so it attributes a record to its test.
fn hashlist_orphan_warnings_for(orphan: &std::path::Path) -> Vec<String> {
    let needle = orphan.display().to_string();
    TEST_LOG
        .hashlist_orphan_warnings
        .lock()
        .expect("hashlist orphan warning store")
        .iter()
        .filter(|record| record.contains(&needle))
        .cloned()
        .collect()
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_slow_or_panicking_handler_does_not_stall_the_link() {
    let mut pair = loopback_request_pair("request-isolation").await;
    register_handler(&pair, "/slow", RequestAllow::All, |_: InboundRequest| async {
        tokio::time::sleep(SLOW_HANDLER_SLEEP).await;
        None
    })
    .await;
    register_handler(&pair, "/panic", RequestAllow::All, |_: InboundRequest| async {
        panic!("handler panics on purpose")
    })
    .await;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;

    send_request(&pair, "/slow", rmpv::Value::Nil).await;
    let id = send_request(&pair, "/echo", rmpv::Value::from("after slow")).await;
    assert_eq!(await_response(&mut pair, id).await, rmpv::Value::from("after slow"));

    send_request(&pair, "/panic", rmpv::Value::Nil).await;
    let id = send_request(&pair, "/echo", rmpv::Value::from("after panic")).await;
    assert_eq!(await_response(&mut pair, id).await, rmpv::Value::from("after panic"));
    // The worker joins the panicked task between requests; a worker that exited on that
    // join would leave this one unanswered.
    let id = send_request(&pair, "/echo", rmpv::Value::from("after the join")).await;
    assert_eq!(await_response(&mut pair, id).await, rmpv::Value::from("after the join"));

    exchange_data_both_ways(&mut pair.handles).await;

    timeout(STOP_TIMEOUT, pair.handles.listener.transport.stop())
        .await
        .expect("listener stops while a handler is still sleeping");
    timeout(STOP_TIMEOUT, pair.handles.connector.transport.stop()).await.expect("connector stops");
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_full_dispatch_queue_drops_without_blocking_the_link() {
    install_test_logger();
    let mut pair = loopback_request_pair("request-queue-full").await;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;
    let link_id = pair.handles.link_id;

    // A one-slot queue that nobody reads: the first request fills it, the second must be
    // dropped. The receiver stays alive so the drop is a full queue, not a closed one.
    let (queue_tx, _queue_rx) = tokio::sync::mpsc::channel(1);
    pair.handles
        .listener
        .transport
        .find_in_link(&link_id)
        .await
        .expect("responder tracks the inbound link")
        .lock()
        .await
        .set_request_dispatch(queue_tx);

    let first = send_request(&pair, "/echo", rmpv::Value::from(1)).await;
    let second = send_request(&pair, "/echo", rmpv::Value::from(2)).await;
    for id in [first, second] {
        let observed = await_event(&mut pair.handles.in_events, |event| {
            event.id == link_id
                && matches!(&event.event, LinkEvent::Data(payload)
                    if payload.request_id() == Some(id.to_bytes()))
        })
        .await;
        assert!(
            matches!(&observed.event, LinkEvent::Data(payload)
                if payload.context() == PacketContext::Request),
            "the Data event still posts for a request the queue could not take"
        );
    }

    let dropped = dropped_request_records_for(link_id);
    assert_eq!(dropped.len(), 1, "exactly the second request is dropped: {dropped:?}");
    assert!(
        dropped[0].contains(&format!("request {} dropped, dispatch queue full", second)),
        "the drop record names the request: {dropped:?}"
    );

    exchange_data_both_ways(&mut pair.handles).await;
}
