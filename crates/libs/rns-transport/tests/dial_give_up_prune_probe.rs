//! Usage probe for the parts of the `TcpClient` give-up behaviour that
//! `dial_give_up_text_probe.rs` leaves to the reader. The give-up line keeps the error
//! level it always had, so a consumer filtering its logs at error still sees it, and its N
//! is exactly `budget + 1` for a client that only ever reads `Connection refused`. Of the
//! two manager cleanup passes that prune a client that gave up, this file exercises the
//! send pass on its own: once the give-up line is logged, one `Transport::send_packet` is
//! enough for the next read to be `None`, with no `stop` call and no poll. The tick pass
//! is pinned by the text probe's prune poll, which sends nothing and polls until the read
//! settles at `None`.

use std::sync::{Mutex, Once};
use std::time::Duration;

use rns_transport::destination::DestinationName;
use rns_transport::hash::AddressHash;
use rns_transport::identity::PrivateIdentity;
use rns_transport::iface::tcp_client::TcpClient;
use rns_transport::rand_core::OsRng;
use rns_transport::transport::Transport;
use tokio::time::timeout;

#[path = "support/client_transport.rs"]
mod client_transport;
#[path = "support/free_loopback_port.rs"]
mod free_loopback_port;
#[path = "support/wait_until.rs"]
mod wait_until;
use client_transport::client_transport;
use free_loopback_port::free_loopback_port;
use wait_until::wait_until;

const WAIT: Duration = Duration::from_secs(5);
const RECONNECT_WAIT: Duration = Duration::from_millis(50);
const GIVE_UP_PREFIX: &str = "max TCP reconnect attempts reached for <";

struct CaptureLogger {
    records: Mutex<Vec<(log::Level, String)>>,
}

static CAPTURED: CaptureLogger = CaptureLogger { records: Mutex::new(Vec::new()) };

impl log::Log for CaptureLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.target().starts_with("rns_transport::iface")
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        self.records
            .lock()
            .expect("captured record store")
            .push((record.level(), record.args().to_string()));
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

/// The give-up records logged for `addr`, level and message.
fn give_up_records(addr: &str) -> Vec<(log::Level, String)> {
    CAPTURED
        .records
        .lock()
        .expect("captured record store")
        .iter()
        .filter(|(_, message)| message.starts_with(&format!("{GIVE_UP_PREFIX}{addr}>")))
        .cloned()
        .collect()
}

/// Spawns a `TcpClient` with `budget` reconnect tries against `addr` through `transport`
/// and polls until its give-up line is logged; returns the interface address.
#[deny(clippy::await_holding_invalid_type)]
async fn spawn_and_give_up(transport: &Transport, addr: &str, budget: u64) -> AddressHash {
    let client = TcpClient::new(addr.to_string())
        .with_reconnect_wait(RECONNECT_WAIT)
        .with_max_reconnect_tries(Some(budget));
    let iface = transport.iface_manager().lock().await.spawn(client, TcpClient::spawn);
    assert_eq!(transport.interface_connect_count(&iface).await, Some(0));
    wait_until("the client gives up", || !give_up_records(addr).is_empty()).await;
    iface
}

/// A client that only ever reads `Connection refused` logs its give-up line at error, the
/// level the line always had, and N is exactly one above the budget: the budget is spent on
/// the attempt after it.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn the_give_up_line_is_an_error_record_naming_budget_plus_one_attempts() {
    install_capture_logger();
    let transport = client_transport("give-up-level");
    for budget in [1u64, 3] {
        // One endpoint per client, so the wait below is on this client's own line.
        let port = free_loopback_port().await;
        let addr = format!("127.0.0.1:{port}");
        spawn_and_give_up(&transport, &addr, budget).await;
        let records = give_up_records(&addr);
        assert_eq!(records.len(), 1, "exactly one give-up line per client: {records:?}");
        let (level, message) = &records[0];
        assert_eq!(*level, log::Level::Error, "the give-up line's level is unchanged: {message:?}");
        assert_eq!(
            message,
            &format!("{GIVE_UP_PREFIX}{addr}> after {} failed attempt(s)", budget + 1),
            "a budget of {budget} is spent on attempt {}",
            budget + 1
        );
    }
    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
}

/// One send through the transport is a cleanup pass: right after the give-up line, a
/// `send_packet` leaves the record of the client that gave up pruned, so the next
/// `interface_connect_count` reads `None` with no `stop` call. The send is the pass under
/// test; the announce job's 1 s tick is not excluded, only not polled for, so a tick that
/// lands first makes the same read `None` for the other reason.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn one_send_after_the_give_up_line_prunes_the_record() {
    install_capture_logger();
    let port = free_loopback_port().await;
    let addr = format!("127.0.0.1:{port}");
    let transport = client_transport("give-up-send-prunes");
    let destination = transport
        .add_destination(
            PrivateIdentity::new_from_rand(OsRng),
            DestinationName::new("probe", "prune"),
        )
        .await;
    let iface = spawn_and_give_up(&transport, &addr, 1).await;
    let right_after = transport.interface_connect_count(&iface).await;
    assert!(
        right_after.is_none() || right_after == Some(0),
        "right after the give-up line the count is the last Some(0) or already pruned: {right_after:?}"
    );

    let packet = destination.lock().await.announce(OsRng, None).expect("valid announce packet");
    transport.send_packet(packet).await;
    assert_eq!(
        transport.interface_connect_count(&iface).await,
        None,
        "a send is a cleanup pass: the record of a client that gave up is gone once it returns"
    );

    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
    assert_eq!(transport.interface_connect_count(&iface).await, None, "still pruned after stop");
}
