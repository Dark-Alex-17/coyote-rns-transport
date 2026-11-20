//! The `multiprocessing.connection` frame: a big-endian signed 32-bit length
//! and the payload; a length of -1 announces a big-endian unsigned 64-bit
//! length in the next eight bytes (`connection.py:427-458`).

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use super::RpcError;

/// `Connection._send_bytes` for a payload the 32-bit header can carry
/// (`connection.py:427-448`): header and payload go out in one write, as
/// Python does for small frames. Our frames are the auth messages and a
/// one-key request, so the wide header is never needed.
pub(super) async fn send_frame<S>(stream: &mut S, payload: &[u8]) -> Result<(), RpcError>
where
    S: AsyncWrite + Unpin,
{
    let len = i32::try_from(payload.len()).map_err(|_| {
        RpcError::Protocol(format!("frame of {} bytes exceeds the 32-bit header", payload.len()))
    })?;
    let mut frame = Vec::with_capacity(4 + payload.len());
    frame.extend_from_slice(&len.to_be_bytes());
    frame.extend_from_slice(payload);
    stream.write_all(&frame).await.map_err(RpcError::Io)?;
    stream.flush().await.map_err(RpcError::Io)
}

/// `Connection._recv_bytes(maxsize)` (`connection.py:450-458`): the length is
/// checked against `max_len` before anything is allocated, so a hostile
/// header cannot ask for memory; Python returns `None` there and
/// `recv_bytes` raises `OSError("bad message length")`. A negative length
/// other than -1 is also refused. A short read is [`RpcError::Io`]
/// (`UnexpectedEof`).
pub(super) async fn recv_frame<S>(stream: &mut S, max_len: usize) -> Result<Vec<u8>, RpcError>
where
    S: AsyncRead + Unpin,
{
    let mut header = [0u8; 4];
    read_exact(stream, &mut header).await?;
    let len = match i32::from_be_bytes(header) {
        -1 => {
            let mut wide = [0u8; 8];
            read_exact(stream, &mut wide).await?;
            u64::from_be_bytes(wide)
        }
        size => u64::try_from(size)
            .map_err(|_| RpcError::Protocol(format!("negative frame length {size}")))?,
    };
    let len = usize::try_from(len)
        .ok()
        .filter(|len| *len <= max_len)
        .ok_or_else(|| RpcError::Protocol(format!("frame length {len} exceeds cap {max_len}")))?;
    let mut payload = vec![0u8; len];
    read_exact(stream, &mut payload).await?;
    Ok(payload)
}

async fn read_exact<S>(stream: &mut S, buf: &mut [u8]) -> Result<(), RpcError>
where
    S: AsyncRead + Unpin,
{
    stream.read_exact(buf).await.map(|_| ()).map_err(RpcError::Io)
}
