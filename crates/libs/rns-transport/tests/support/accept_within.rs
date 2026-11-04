//! Accepts one connection on a listener within a fixed deadline; shared by the integration
//! tests that serve a stream client from a raw listener.

use std::time::Duration;

use tokio::net::{TcpListener, TcpStream};
use tokio::time::timeout;

const WAIT: Duration = Duration::from_secs(5);

/// The next stream `listener` accepts within a 5 s deadline, or a panic naming `what`.
#[deny(clippy::await_holding_invalid_type)]
pub async fn accept_within(listener: &TcpListener, what: &str) -> TcpStream {
    timeout(WAIT, listener.accept())
        .await
        .unwrap_or_else(|_| panic!("{what} timed out"))
        .unwrap_or_else(|err| panic!("{what} failed: {err}"))
        .0
}
