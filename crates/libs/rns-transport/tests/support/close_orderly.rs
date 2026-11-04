//! Closes a served stream so the peer reads a FIN rather than a reset; shared by the
//! integration tests that drop a stream client from a raw listener.

use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _};
use tokio::time::timeout;

const WAIT: Duration = Duration::from_secs(5);

/// Closes `stream` in an orderly way: the write side is shut down and the stream read to
/// the peer's EOF before it is dropped. An attached transport writes its tunnel packet
/// on each connect, and a TCP socket dropped with unread data in its receive buffer sends
/// a reset rather than a FIN.
#[deny(clippy::await_holding_invalid_type)]
pub async fn close_orderly(mut stream: impl AsyncRead + AsyncWrite + Unpin) {
    stream.shutdown().await.expect("shut the write side down");
    timeout(WAIT, async {
        let mut sink = [0u8; 1024];
        while stream.read(&mut sink).await.expect("drain the peer's writes") > 0 {}
    })
    .await
    .expect("timed out waiting for the peer to close its side");
}
