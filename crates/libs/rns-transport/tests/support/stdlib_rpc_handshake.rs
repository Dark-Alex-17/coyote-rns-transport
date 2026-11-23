//! The server side of CPython's `multiprocessing.connection` mutual handshake
//! (`deliver_challenge`, then `answer_challenge`) in the Python 3.12+ `{sha256}` form;
//! shared by the integration tests that stand up a loopback server for the rnsd RPC
//! client. The challenge and the answer are public beside the whole handshake so a test
//! can serve it honestly up to one step and go wrong there. The including test binary
//! declares `stdlib_rpc_frame` alongside this module; declaring it here as well would
//! load that file twice.

use hmac::{Hmac, Mac};
use sha2::Sha256;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};

use super::stdlib_rpc_frame::{frame, recv_frame, CHALLENGE, FAILURE, WELCOME};

const DIGEST_PREFIX: &[u8] = b"{sha256}";

/// The stdlib's `deliver_challenge` message in the modern form: `{sha256}` + 40 bytes.
pub fn server_challenge() -> Vec<u8> {
    let mut message = DIGEST_PREFIX.to_vec();
    message.extend_from_slice(&[0x42u8; 40]);
    message
}

/// The stdlib's `_create_response` for a `{sha256}`-prefixed message: the MAC covers the
/// whole message, header included, and the reply carries the same prefix.
pub fn answer(authkey: &[u8], message: &[u8]) -> Vec<u8> {
    assert!(
        message.starts_with(DIGEST_PREFIX),
        "the client challenges with {{sha256}}: {message:?}"
    );
    let mut mac = Hmac::<Sha256>::new_from_slice(authkey).expect("hmac key");
    mac.update(message);
    let mut out = DIGEST_PREFIX.to_vec();
    out.extend_from_slice(&mac.finalize().into_bytes());
    out
}

/// Serve the stdlib's mutual handshake honestly with `authkey` and return the request
/// frame the client sends next; with a key the client does not hold, do what
/// `deliver_challenge` does: send `#FAILURE#`, close, return `None`.
#[deny(clippy::await_holding_invalid_type)]
pub async fn serve_handshake<S: AsyncRead + AsyncWrite + Unpin>(
    stream: &mut S,
    authkey: &[u8],
) -> Option<Vec<u8>> {
    let message = server_challenge();
    let mut challenge = CHALLENGE.to_vec();
    challenge.extend_from_slice(&message);
    stream.write_all(&frame(&challenge)).await.expect("write challenge");
    let response = recv_frame(stream).await.expect("the client's answer");
    if response != answer(authkey, &message) {
        stream.write_all(&frame(FAILURE)).await.expect("write failure");
        stream.flush().await.expect("flush");
        let _ = stream.shutdown().await;
        return None;
    }
    stream.write_all(&frame(WELCOME)).await.expect("write welcome");
    let theirs = recv_frame(stream).await.expect("the client's challenge");
    assert!(theirs.starts_with(CHALLENGE), "{theirs:?}");
    let ours = answer(authkey, &theirs[CHALLENGE.len()..]);
    stream.write_all(&frame(&ours)).await.expect("write our answer");
    let welcome = recv_frame(stream).await.expect("the client's verdict");
    assert_eq!(welcome, WELCOME, "the client rejected a correct answer");
    Some(recv_frame(stream).await.expect("the request"))
}
