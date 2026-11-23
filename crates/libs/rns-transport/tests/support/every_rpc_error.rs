//! Every `RpcError` variant, hand-maintained (the enum is `#[non_exhaustive]`,
//! so this list cannot detect growth; the in-crate match beside
//! `is_transient_follows_the_io_kind_retry_rule` does), with every io kind the README names
//! behind `Io` and `Connect`; shared by the integration tests that hold a consumer's retry
//! predicate to `RpcError::is_transient()`.

use std::io::{Error, ErrorKind};

use rns_transport::transport::shared_instance_rpc::RpcError;

/// The four retryable kinds on each of `Io` and `Connect`, a refused `Connect`, an `Io` of
/// kind `Other`, and the six variants with no io kind: nine of them transient.
pub fn every_rpc_error() -> Vec<RpcError> {
    let mut errors = Vec::new();
    for kind in [
        ErrorKind::ConnectionReset,
        ErrorKind::ConnectionAborted,
        ErrorKind::BrokenPipe,
        ErrorKind::UnexpectedEof,
    ] {
        errors.push(RpcError::Io(Error::from(kind)));
        errors.push(RpcError::Connect(Error::from(kind)));
    }
    errors.extend([
        RpcError::Connect(Error::from(ErrorKind::ConnectionRefused)),
        RpcError::Io(Error::from(ErrorKind::Other)),
        RpcError::Timeout,
        RpcError::Authentication,
        RpcError::Protocol("short".into()),
        RpcError::Decode("shape".into()),
        RpcError::UnsupportedDigest("sha1".into()),
        RpcError::InvalidKeyMaterial,
    ]);
    errors
}
