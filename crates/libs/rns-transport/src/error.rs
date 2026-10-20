#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RnsError {
    #[error("allocation failed")]
    OutOfMemory,
    #[error("invalid argument")]
    InvalidArgument,
    #[error("signature verification failed")]
    IncorrectSignature,
    #[error("hash verification failed")]
    IncorrectHash,
    #[error("cryptographic operation failed")]
    CryptoError,
    #[error("invalid packet")]
    PacketError,
    #[error("connection failed")]
    ConnectionError,
    #[error("resource reader failed: {0}")]
    ResourceReader(#[source] std::io::Error),
    /// A link request to a destination the path table does not know, on a
    /// transport whose `broadcast` is off, is not sent. `RNS/Transport.py:1383`
    /// routes an outbound packet through the path table and `:1443` otherwise
    /// broadcasts it on every outgoing interface; this crate only broadcasts
    /// when `broadcast` is on and drops the packet as `DroppedNoRoute` when it
    /// is not.
    #[error("no path to destination")]
    NoPath,
}
