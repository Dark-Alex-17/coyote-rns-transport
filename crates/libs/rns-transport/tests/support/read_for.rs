//! Reads everything a stream carries during a window; shared by the integration tests
//! that watch a raw listener's stream for what a transport's client puts on it.

use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::time::timeout;

/// Everything `stream` carries during `window`. An end of stream is a failure: the
/// client has no reason to close a stream it just connected.
#[deny(clippy::await_holding_invalid_type)]
pub async fn read_for(stream: &mut (impl AsyncRead + Unpin), window: Duration) -> Vec<u8> {
    let mut raw = Vec::new();
    let mut buf = [0u8; 1024];
    let deadline = tokio::time::Instant::now() + window;
    loop {
        let left = deadline.saturating_duration_since(tokio::time::Instant::now());
        match timeout(left, stream.read(&mut buf)).await {
            Err(_) => return raw,
            Ok(Ok(0)) => panic!("the client closed the stream"),
            Ok(Ok(read)) => raw.extend_from_slice(&buf[..read]),
            Ok(Err(err)) => panic!("the read failed: {err}"),
        }
    }
}
