//! Usage probes for the one-request rule written from the contract, not the code: a
//! pending outbound link sends its `LinkRequest` once and the establishment timeout is
//! the only bound on a link that never comes up (`RNS/Link.py:313-320`, `:722-729`).
//! The shipped probes count the frame on a raw listener's stream and read the `Closed`
//! event; these cover the consumer patterns around them that no committed test pins:
//!
//! - cold start with NO interface at all: `Transport::link` with broadcast on still hands
//!   back a pending link, and a 2 s establishment timeout closes it with `Timeout` on
//!   the next 1 s maintenance tick, so the `Closed` event lands between 2 s and 4 s after
//!   `link()`; the close is published once and the link stays `Closed` afterwards;
//! - the initiator gives up first: `Link::close()` on a pending link publishes `Closed`
//!   with `InitiatorClosed`, nothing more addressed to the link reaches the stream, and
//!   the establishment timeout that elapses afterwards does not publish a second close;
//! - the removed maintenance repeat leaves no trace in the log: across the 6 s mark at
//!   which it used to fire, no record of any level says "repeat link request" and the
//!   link is still `Pending` with nothing overridden.

use std::sync::{Mutex, Once};
use std::time::{Duration, Instant};

use rns_transport::destination::link::{
    LinkCloseReason, LinkEvent, LinkEventData, LinkId, LinkStatus,
};
use tokio::sync::broadcast;
use tokio::time::timeout;

#[path = "support/accept_within.rs"]
mod accept_within;
#[path = "support/broadcasting.rs"]
mod broadcasting;
#[path = "support/closed_event_for.rs"]
mod closed_event_for;
#[path = "support/initiator_on_a_silent_listener.rs"]
mod initiator_on_a_silent_listener;
#[path = "support/link_requests_to.rs"]
mod link_requests_to;
#[path = "support/packets_in.rs"]
mod packets_in;
#[path = "support/read_for.rs"]
mod read_for;
#[path = "support/spawn_plain_client.rs"]
mod spawn_plain_client;
#[path = "support/unknown_destination.rs"]
mod unknown_destination;
#[path = "support/wait_until.rs"]
mod wait_until;
use broadcasting::broadcasting;
use closed_event_for::closed_event_for;
use initiator_on_a_silent_listener::initiator_on_a_silent_listener;
use link_requests_to::link_requests_to;
use packets_in::packets_in;
use read_for::read_for;
use unknown_destination::unknown_destination;

/// The shortened establishment timeout given to a link the test wants closed.
const ESTABLISHMENT: Duration = Duration::from_secs(2);
/// The link maintenance job ticks once a second: a 2 s timeout closes by 3 s on a quiet
/// host; a second of slack covers a loaded one.
const CLOSE_BOUND: Duration = Duration::from_secs(4);
/// Two more maintenance ticks and a little: long enough for a second close to show.
const QUIET: Duration = Duration::from_millis(2500);
/// The interval at which the removed maintenance repeat used to fire (6 s), plus the
/// tick that would have carried it, plus a second to spare.
const PAST_OLD_REPEAT: Duration = Duration::from_secs(8);
const WAIT: Duration = Duration::from_secs(5);

/// How many `Closed` events for `id` the receiver carries during `window`.
#[deny(clippy::await_holding_invalid_type)]
async fn closed_events_during(
    events: &mut broadcast::Receiver<LinkEventData>,
    id: LinkId,
    window: Duration,
) -> usize {
    let mut count = 0;
    let deadline = tokio::time::Instant::now() + window;
    loop {
        let left = deadline.saturating_duration_since(tokio::time::Instant::now());
        match timeout(left, events.recv()).await {
            Err(_) => return count,
            Ok(Ok(event)) => {
                if event.id == id && matches!(event.event, LinkEvent::Closed) {
                    count += 1;
                }
            }
            Ok(Err(broadcast::error::RecvError::Lagged(_))) => {}
            Ok(Err(broadcast::error::RecvError::Closed)) => return count,
        }
    }
}

/// Cold start with no interface: `link()` hands back a pending link, a 2 s establishment
/// timeout closes it with `Timeout` between 2 s and 4 s after `link()`, the close is
/// published exactly once, and the link stays `Closed` with that reason afterwards.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn a_pending_link_with_no_interface_is_closed_once_at_its_timeout() {
    let transport = broadcasting("pending-link-no-interface");
    let mut events = transport.out_link_events();

    let started = Instant::now();
    let link = transport.link(unknown_destination()).await.expect("pending link with broadcast on");
    link.lock().await.set_establishment_timeout(ESTABLISHMENT);
    let id = *link.lock().await.id();
    assert_eq!(link.lock().await.status(), LinkStatus::Pending);
    assert_eq!(link.lock().await.close_reason(), None, "a pending link has no close reason");

    let closed = closed_event_for(&mut events, id, WAIT).await;
    let elapsed = started.elapsed();
    assert_eq!(closed.close_reason, Some(LinkCloseReason::Timeout));
    assert!(
        elapsed >= ESTABLISHMENT,
        "the link is not closed before its establishment timeout, closed after {elapsed:?}"
    );
    assert!(
        elapsed < CLOSE_BOUND,
        "the close lands on the maintenance tick after the timeout, closed after {elapsed:?}"
    );
    assert_eq!(link.lock().await.status(), LinkStatus::Closed);
    assert_eq!(link.lock().await.close_reason(), Some(LinkCloseReason::Timeout));

    // The close is published once; later ticks leave the closed link alone.
    assert_eq!(
        closed_events_during(&mut events, id, QUIET).await,
        0,
        "no second Closed event for a link already closed by its timeout"
    );
    assert_eq!(link.lock().await.status(), LinkStatus::Closed);
    assert_eq!(link.lock().await.close_reason(), Some(LinkCloseReason::Timeout));

    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
}

/// The initiator gives up first: after the single request is on the stream, `Link::close()`
/// publishes `Closed` with `InitiatorClosed`, no further `LinkRequest` to the link's
/// destination reaches the stream, and the establishment timeout elapsing afterwards
/// publishes no second close.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn closing_a_pending_link_by_hand_sends_nothing_more_and_closes_once() {
    let (transport, mut stream) = initiator_on_a_silent_listener("pending-link-hand-close").await;
    let mut events = transport.out_link_events();

    let destination = unknown_destination();
    let link = transport.link(destination).await.expect("pending link with broadcast on");
    link.lock().await.set_establishment_timeout(ESTABLISHMENT);
    let id = *link.lock().await.id();

    // The single request is on the wire well before the timeout.
    let raw = read_for(&mut stream, Duration::from_secs(1)).await;
    assert_eq!(
        link_requests_to(&packets_in(&raw), destination.address_hash),
        1,
        "the pending link's one request is on the stream within a second"
    );

    link.lock().await.close();
    let closed = closed_event_for(&mut events, id, WAIT).await;
    assert_eq!(
        closed.close_reason,
        Some(LinkCloseReason::InitiatorClosed),
        "a link the initiator closes reports InitiatorClosed, not Timeout"
    );
    assert_eq!(link.lock().await.status(), LinkStatus::Closed);
    assert_eq!(link.lock().await.close_reason(), Some(LinkCloseReason::InitiatorClosed));

    // Past the establishment timeout that would have closed it: nothing more for the link
    // on the stream and no second close.
    let tail = read_for(&mut stream, 2 * ESTABLISHMENT).await;
    assert_eq!(
        link_requests_to(&packets_in(&tail), destination.address_hash),
        0,
        "no link request follows the initiator's close"
    );
    assert_eq!(
        closed_events_during(&mut events, id, Duration::from_millis(100)).await,
        0,
        "the establishment timeout does not close a link the initiator already closed"
    );
    assert_eq!(link.lock().await.close_reason(), Some(LinkCloseReason::InitiatorClosed));

    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
}

struct CaptureLogger {
    records: Mutex<Vec<(log::Level, String, String)>>,
}

static CAPTURED: CaptureLogger = CaptureLogger { records: Mutex::new(Vec::new()) };

impl log::Log for CaptureLogger {
    fn enabled(&self, _metadata: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        self.records.lock().expect("captured record store").push((
            record.level(),
            record.target().to_string(),
            record.args().to_string(),
        ));
    }

    fn flush(&self) {}
}

fn install_capture_logger() {
    static INSTALL: Once = Once::new();
    INSTALL.call_once(|| {
        log::set_logger(&CAPTURED).expect("no other logger in this test binary");
        log::set_max_level(log::LevelFilter::Trace);
    });
}

/// Every captured record of any level whose message contains `needle`, case-insensitive.
fn records_containing(needle: &str) -> Vec<(log::Level, String, String)> {
    let needle = needle.to_ascii_lowercase();
    CAPTURED
        .records
        .lock()
        .expect("captured record store")
        .iter()
        .filter(|(_, _, message)| message.to_ascii_lowercase().contains(&needle))
        .cloned()
        .collect()
}

/// With nothing overridden and no interface, a pending link crosses the 6 s mark at which
/// the removed repeat used to fire still `Pending`, with no "repeat link request" record
/// at any level and no `Closed` event.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn no_repeat_link_request_is_logged_across_the_old_repeat_interval() {
    install_capture_logger();
    let transport = broadcasting("pending-link-no-repeat-log");
    let mut events = transport.out_link_events();

    let link = transport.link(unknown_destination()).await.expect("pending link with broadcast on");
    let id = *link.lock().await.id();

    assert_eq!(
        closed_events_during(&mut events, id, PAST_OLD_REPEAT).await,
        0,
        "a link inside its transport-sized establishment timeout is not closed"
    );
    assert_eq!(link.lock().await.status(), LinkStatus::Pending);
    assert_eq!(link.lock().await.close_reason(), None);

    let repeats = records_containing("repeat link request");
    assert!(repeats.is_empty(), "the removed repeat left log records: {repeats:?}");
    let warns: Vec<_> = CAPTURED
        .records
        .lock()
        .expect("captured record store")
        .iter()
        .filter(|(level, _, message)| {
            *level <= log::Level::Warn && message.to_ascii_lowercase().contains("link request")
        })
        .cloned()
        .collect();
    assert!(warns.is_empty(), "a pending link raises no warning about its request: {warns:?}");

    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
}
