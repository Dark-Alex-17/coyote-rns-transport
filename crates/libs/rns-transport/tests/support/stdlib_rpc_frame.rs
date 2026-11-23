//! The handshake tokens and the `send_bytes`/`recv_bytes` framing of CPython's
//! `multiprocessing.connection`; shared by the integration tests that stand up a loopback
//! server for the rnsd RPC client. The tokens and the two framing functions live together
//! because a mock never writes or reads one without the other.

use tokio::io::{AsyncRead, AsyncReadExt};

pub const CHALLENGE: &[u8] = b"#CHALLENGE#";
pub const WELCOME: &[u8] = b"#WELCOME#";
pub const FAILURE: &[u8] = b"#FAILURE#";
/// The digest-name prefix the modern-form challenge and answer messages carry.
pub const DIGEST_PREFIX: &[u8] = b"{sha256}";

/// One stdlib `send_bytes` frame: signed big-endian 32-bit length, then the payload.
pub fn frame(payload: &[u8]) -> Vec<u8> {
    let mut out = i32::try_from(payload.len()).expect("frame fits").to_be_bytes().to_vec();
    out.extend_from_slice(payload);
    out
}

/// `Connection._recv_bytes` for the frames this exchange carries.
#[deny(clippy::await_holding_invalid_type)]
pub async fn recv_frame<S: AsyncRead + Unpin>(stream: &mut S) -> std::io::Result<Vec<u8>> {
    let mut header = [0u8; 4];
    stream.read_exact(&mut header).await?;
    let len = usize::try_from(i32::from_be_bytes(header)).expect("non-negative length");
    let mut payload = vec![0u8; len];
    stream.read_exact(&mut payload).await?;
    Ok(payload)
}
