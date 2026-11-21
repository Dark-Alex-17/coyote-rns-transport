//! Fork-only client of the rnsd shared instance's RPC listener, the
//! `multiprocessing.connection.Listener` a Python shared instance opens at
//! `RNS/Reticulum.py:363-367` and serves from `rpc_loop`
//! (`RNS/Reticulum.py:1274-1334`). Every call is one fresh connection, the
//! mutual HMAC handshake, one msgpack request and one reply, after which the
//! server closes. The calls: [`interface_stats`], the
//! `{"get": "interface_stats"}` request rnstatus sends
//! (`RNS/Reticulum.py:1397-1402`), decoded into [`InterfaceStats`]; and the
//! path calls [`path_table`] (rows of [`PathRow`]),
//! [`next_hop_interface_name`], [`first_hop_timeout`], [`drop_path`] and
//! [`drop_all_via`], with [`drop_path_settled`] waiting out the cull that
//! follows a drop. Typed fields keep the raw msgpack beside them. Python has
//! no typed client and this crate hosts no RPC listener.

use std::fmt;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::Path;
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
#[cfg(unix)]
use tokio::net::UnixStream;

#[cfg(unix)]
use crate::iface::local::LocalUnixEndpoint;

mod decode;
mod framing;
mod handshake;
mod path_table;
mod paths;
#[cfg(test)]
mod tests;

pub use path_table::PathRow;
pub use paths::{
    drop_all_via, drop_path, drop_path_settled, first_hop_timeout, next_hop_interface_name,
    path_table,
};

/// Python's default `instance_control_port` (`RNS/Reticulum.py:296`).
const DEFAULT_CONTROL_PORT: u16 = 37429;

/// Cap on the reply frame; the auth frames are capped at 256 by the
/// handshake itself.
const REPLY_MAX_LEN: usize = 16 * 1024 * 1024;

/// `mp.packb({"get": "interface_stats"})` (`RNS/Reticulum.py:1400`): a
/// msgpack fixmap of one entry, the fixstr `get` to the fixstr
/// `interface_stats`.
const INTERFACE_STATS_REQUEST: &[u8] = b"\x81\xa3get\xafinterface_stats";

/// Where the shared instance's RPC listener is. Python binds
/// `("127.0.0.1", local_control_port)` unless `use_af_unix` is set, in which
/// case it binds the abstract socket `\0rns/{instance_name}/rpc`
/// (`RNS/Reticulum.py:353-358`); `use_af_unix` is set on Linux and Android
/// for every shared instance type but `tcp` (`RNS/Reticulum.py:725-727`).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RpcEndpoint {
    /// The TCP listener at the instance's `instance_control_port`
    /// (`RNS/Reticulum.py:296`, `:495`).
    Tcp(SocketAddr),
    /// A unix-socket listener. Python binds the abstract name
    /// `\0rns/{instance_name}/rpc` (`RNS/Reticulum.py:354`), which resolves
    /// only on Linux and Android; elsewhere `LocalUnixEndpoint::tokio_path`
    /// treats an `Abstract` endpoint as a relative filesystem path, so build
    /// `Filesystem` endpoints there.
    #[cfg(unix)]
    Unix(LocalUnixEndpoint),
}

impl RpcEndpoint {
    /// `127.0.0.1:37429`, Python's default control port
    /// (`RNS/Reticulum.py:296`).
    #[must_use]
    pub fn default_tcp() -> Self {
        Self::Tcp(SocketAddr::from((Ipv4Addr::LOCALHOST, DEFAULT_CONTROL_PORT)))
    }

    /// The endpoint a Python shared instance named `name` opens by default:
    /// the abstract unix socket `rns/{name}/rpc` on Linux and Android
    /// (`RNS/Reticulum.py:353-355`, `:725-727`), [`Self::default_tcp`]
    /// elsewhere. A shared instance configured with
    /// `shared_instance_type = tcp` on Linux listens on TCP instead; reach it
    /// with [`RpcEndpoint::Tcp`] at its `instance_control_port`.
    #[must_use]
    pub fn for_instance(name: &str) -> Self {
        instance_rpc_endpoint(name)
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn instance_rpc_endpoint(name: &str) -> RpcEndpoint {
    RpcEndpoint::Unix(LocalUnixEndpoint::abstract_name(format!("rns/{name}/rpc")))
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
fn instance_rpc_endpoint(_name: &str) -> RpcEndpoint {
    RpcEndpoint::default_tcp()
}

/// The `authkey` of the listener. Python uses the configured `rpc_key`
/// (`RNS/Reticulum.py:499-505`) or, by default, the SHA-256 of the transport
/// identity's private key (`RNS/Reticulum.py:360-361`). `Debug` prints
/// `RpcKey(redacted)`; the bytes never reach a log.
#[derive(Clone)]
pub struct RpcKey(Vec<u8>);

impl RpcKey {
    /// A configured `rpc_key` (`RNS/Reticulum.py:499-505`): the hex digits
    /// `bytes.fromhex` accepts, without its whitespace tolerance. An empty
    /// string, an odd length or a non-hex character is
    /// [`RpcError::InvalidKeyMaterial`]; Python keeps an empty key and runs
    /// the listener with an empty `authkey` (`RNS/Reticulum.py:360-364`).
    pub fn from_hex(hex: &str) -> Result<Self, RpcError> {
        let bytes = hex::decode(hex).map_err(|_| RpcError::InvalidKeyMaterial)?;
        if bytes.is_empty() {
            return Err(RpcError::InvalidKeyMaterial);
        }
        Ok(Self(bytes))
    }

    /// Python's default key: the SHA-256 of the 64-byte private key stored in
    /// `<configdir>/storage/transport_identity` (`RNS/Reticulum.py:360-361`,
    /// `Identity.full_hash`). A file of any other length is
    /// [`RpcError::InvalidKeyMaterial`]; so is an unreadable file, with the
    /// io error logged at warn level; [`RpcError::Io`] is reserved for
    /// failures after the dial and [`RpcError::Connect`] for the dial itself,
    /// and a local file that never reached the network fits neither.
    pub fn from_transport_identity_file(path: &Path) -> Result<Self, RpcError> {
        let private_key = std::fs::read(path).map_err(|err| {
            log::warn!("transport identity unreadable path={} err={}", path.display(), err);
            RpcError::InvalidKeyMaterial
        })?;
        if private_key.len() != 64 {
            return Err(RpcError::InvalidKeyMaterial);
        }
        Ok(Self(<sha2::Sha256 as sha2::Digest>::digest(&private_key).to_vec()))
    }

    fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Debug for RpcKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RpcKey(redacted)")
    }
}

/// What a call can fail with.
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum RpcError {
    /// The TCP or unix connect itself failed; nothing was exchanged. A
    /// `ConnectionReset` kind here is the overflowed dial (see
    /// [`RpcError::Io`]); read it through [`RpcError::io_kind`].
    #[error("connect failed: {0}")]
    Connect(#[source] std::io::Error),
    /// The server rejected our answer to its challenge with `#FAILURE#`
    /// (anything but `#WELCOME#`), or its answer to our challenge did not
    /// verify, in which case we attempt to send `#FAILURE#` ourselves (a
    /// notice a server that has already hung up never reads).
    #[error("shared instance rpc authentication failed")]
    Authentication,
    /// The bytes were wrong: a bad length, an oversized frame, or an
    /// unexpected handshake token (or, in theory only, a request that would
    /// not encode; writing msgpack into a `Vec` does not fail).
    #[error("shared instance rpc protocol error: {0}")]
    Protocol(String),
    /// Every io failure after the dial succeeded (a failed write or flush, a
    /// short read, EOF during the handshake or the reply), except the failed
    /// write of the `#FAILURE#` notice to a server that already hung up,
    /// which stays [`RpcError::Authentication`].
    ///
    /// The kinds worth retrying are `ConnectionReset`, `ConnectionAborted`,
    /// `BrokenPipe` and `UnexpectedEof` (and [`RpcError::Timeout`]): rnsd
    /// serves RPC dials one at a time on `rpc_loop`
    /// (`RNS/Reticulum.py:1274-1334`) behind a `Listener`
    /// (`RNS/Reticulum.py:364`) left at the stdlib default `backlog=1`
    /// (`multiprocessing/connection.py`, `Listener.__init__`), so a dial that
    /// lands while another is served may be queued, dropped or reset, and a
    /// server that departs mid-call renders as one of those kinds (a peer
    /// that accepts and hangs up at once yields `UnexpectedEof` within
    /// microseconds). This library never retries; a caller that does should
    /// bound its attempts and back off briefly between them. Any other kind
    /// is not transient; surface it rather than retry. Some kernels (macOS)
    /// reset the overflowed dial before `connect()` returns, so the same kind
    /// can arrive on [`RpcError::Connect`]; [`RpcError::io_kind`] reads both,
    /// which is why a retrying caller matches the kind, not the variant.
    #[error("shared instance rpc io error after connect: {0}")]
    Io(#[source] std::io::Error),
    /// The reply was not the msgpack shape the call expects: the map
    /// `get_interface_stats` builds, the row array of `get_path_table`, or
    /// the scalar the next-hop, timeout and drop calls answer with.
    #[error("shared instance rpc reply did not decode: {0}")]
    Decode(String),
    /// The server's challenge named a digest outside Python's
    /// `_ALLOWED_DIGESTS`.
    #[error("unsupported challenge digest: {0}")]
    UnsupportedDigest(String),
    /// Key material that cannot be an `authkey`: bad hex, an empty key, or a
    /// transport identity file of the wrong length or unreadable.
    #[error("invalid rpc key material")]
    InvalidKeyMaterial,
    /// The whole call, connect included, outran the caller's budget.
    #[error("shared instance rpc call timed out")]
    Timeout,
}

impl RpcError {
    /// The io error kind behind an [`RpcError::Io`] or [`RpcError::Connect`],
    /// `None` for every other variant. The kinds worth a retry are
    /// `ConnectionReset`, `ConnectionAborted`, `BrokenPipe` and
    /// `UnexpectedEof` (the [`RpcError::Io`] docs say why);
    /// `Connect(ConnectionRefused)` is the listener not being there and is
    /// not one of them, so a caller that waits for a listener to bind keeps
    /// its own `Connect` arm, while a `Connect` whose kind is
    /// `ConnectionReset` is the overflowed dial and worth the same retry (on
    /// unix; Windows renders a full listen backlog as `ConnectionRefused`
    /// too). [`RpcError::Timeout`] is also worth a retry but has no kind;
    /// test for it by variant.
    #[must_use]
    pub fn io_kind(&self) -> Option<std::io::ErrorKind> {
        match self {
            Self::Io(err) | Self::Connect(err) => Some(err.kind()),
            _ => None,
        }
    }

    /// The retry rule the [`RpcError::io_kind`] and [`RpcError::Io`] docs
    /// describe, as one predicate: [`RpcError::Timeout`], or an
    /// [`RpcError::Io`] or [`RpcError::Connect`] whose kind is
    /// `ConnectionReset`, `ConnectionAborted`, `BrokenPipe` or
    /// `UnexpectedEof`. `Connect(ConnectionRefused)` and every other variant
    /// are not transient. This library never retries; a caller that does
    /// bounds its attempts and backs off briefly between them.
    #[must_use]
    pub fn is_transient(&self) -> bool {
        use std::io::ErrorKind::{BrokenPipe, ConnectionAborted, ConnectionReset, UnexpectedEof};
        match self.io_kind() {
            Some(ConnectionReset | ConnectionAborted | BrokenPipe | UnexpectedEof) => true,
            Some(_) => false,
            None => matches!(self, Self::Timeout),
        }
    }
}

/// The decoded `interface_stats` reply (`RNS/Reticulum.py:1405-1628`).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct InterfaceStats {
    /// One entry per interface the shared instance runs, in the server's
    /// order.
    pub interfaces: Vec<InterfaceStat>,
    /// The whole top-level map as received, with the totals (`rxb`, `txb`,
    /// `rxs`, `txs`) and whatever else the server's version adds.
    pub raw: rmpv::Value,
}

/// One interface of the reply (`RNS/Reticulum.py:1527-1566`).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct InterfaceStat {
    /// `str(interface)`, the `Type[name]` form.
    pub name: String,
    /// The configured interface name.
    pub short_name: String,
    /// The Python class name, such as `AutoInterface` or
    /// `TCPServerInterface` (the `type` key).
    pub kind: String,
    /// `interface.online` (the `status` key).
    pub online: bool,
    /// `len(interface.peers)` for an interface with a `peers` attribute
    /// (`RNS/Reticulum.py:1501-1503`); `None` when the key is absent or nil.
    pub peers: Option<u64>,
    /// `interface.clients` (`RNS/Reticulum.py:1409-1410`); `None` when nil.
    pub clients: Option<u64>,
    /// Bytes received.
    pub rxb: u64,
    /// Bytes sent.
    pub txb: u64,
    /// This entry's whole map as received, `hash`, `mtu`, `mode` and the
    /// rest included.
    pub raw: rmpv::Value,
}

/// Reads the shared instance's interface statistics over one fresh
/// connection: connect, the mutual HMAC handshake, the request and the one
/// reply, after which the server closes (`RNS/Reticulum.py:1274-1334`). The
/// whole call runs under `timeout`; outrunning it is [`RpcError::Timeout`].
pub async fn interface_stats(
    endpoint: &RpcEndpoint,
    key: &RpcKey,
    timeout: Duration,
) -> Result<InterfaceStats, RpcError> {
    tokio::time::timeout(timeout, async {
        decode::interface_stats(call(endpoint, key, INTERFACE_STATS_REQUEST).await?)
    })
    .await
    .map_err(|_| RpcError::Timeout)?
}

/// One connection to `endpoint` carrying `request`; the reply as decoded
/// msgpack, left to the caller to shape.
async fn call(
    endpoint: &RpcEndpoint,
    key: &RpcKey,
    request: &[u8],
) -> Result<rmpv::Value, RpcError> {
    let result = match endpoint {
        RpcEndpoint::Tcp(addr) => {
            let stream = TcpStream::connect(addr).await.map_err(RpcError::Connect)?;
            exchange(stream, key, request).await
        }
        #[cfg(unix)]
        RpcEndpoint::Unix(unix) => {
            let stream = UnixStream::connect(unix.tokio_path()).await.map_err(RpcError::Connect)?;
            exchange(stream, key, request).await
        }
    };
    if let Err(RpcError::Authentication) = &result {
        let label = match endpoint {
            RpcEndpoint::Tcp(addr) => addr.to_string(),
            #[cfg(unix)]
            RpcEndpoint::Unix(unix) => unix.label(),
        };
        log::warn!("shared instance rpc authentication failed endpoint={}", label);
    }
    result
}

async fn exchange<S>(mut stream: S, key: &RpcKey, request: &[u8]) -> Result<rmpv::Value, RpcError>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    let form = handshake::answer_challenge(&mut stream, key.as_bytes()).await?;
    handshake::deliver_challenge(&mut stream, key.as_bytes(), form).await?;
    framing::send_frame(&mut stream, request).await?;
    let reply = framing::recv_frame(&mut stream, REPLY_MAX_LEN).await?;
    rmpv::decode::read_value(&mut reply.as_slice()).map_err(|err| RpcError::Decode(err.to_string()))
}
