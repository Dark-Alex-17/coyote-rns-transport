// Loopback coverage for what a request handler cannot do to the transport: a handler that
// sleeps or panics delays no other request on the same link and leaves the transport
// running, and a full dispatch queue drops the request with a debug record instead of
// blocking the link. Helpers come from the sibling `tests_parts` files.

use std::sync::Once;

/// Longer than any test here waits for anything, so a handler stuck in it can only be seen
/// through its effect on other requests.
const SLOW_HANDLER_SLEEP: Duration = Duration::from_secs(30);

/// Keeps the link's drop records and the attach/detach lines of the shared-instance client.
/// Tests share one process and `log::set_logger` installs once, so there must be exactly
/// one logger in the binary and this is it; a record is attributed to a test by the link id
/// or the `tp(<name>)` it names, and only those two families are kept so the store stays
/// small for the rest of the run.
struct TestLogger {
    dropped_requests: StdMutex<Vec<String>>,
    shared_instance: StdMutex<Vec<(log::Level, String)>>,
}

static TEST_LOG: TestLogger = TestLogger {
    dropped_requests: StdMutex::new(Vec::new()),
    shared_instance: StdMutex::new(Vec::new()),
};

/// The target `attach_shared_instance` and `detach_shared_instance` log under: their
/// section file is `include!`d, via `links_parts/transport.rs`, into the links module.
const SHARED_INSTANCE_LOG_TARGET: &str = "rns_transport::transport::links";

impl log::Log for TestLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.target().starts_with("rns_transport::destination::link")
            || metadata.target() == SHARED_INSTANCE_LOG_TARGET
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let message = record.args().to_string();
        if record.target() == SHARED_INSTANCE_LOG_TARGET {
            if message.contains("shared instance") {
                self.shared_instance
                    .lock()
                    .expect("shared-instance record store")
                    .push((record.level(), message));
            }
        } else if message.contains("dropped, dispatch") {
            self.dropped_requests.lock().expect("dropped-request record store").push(message);
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
