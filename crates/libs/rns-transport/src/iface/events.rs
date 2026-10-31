//! Fork-only: typed reasons for a stream client's failed dial, and the
//! reserved reasons for an ended stream. Python's clients log such
//! failures as text and retry (`RNS/Interfaces/LocalInterface.py:159-167`);
//! these types let a consumer tell a refused dial from a missing socket.

use crate::hash::AddressHash;

/// Fork-only: why one dial attempt by a stream client failed.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialFailureKind {
    /// Fork-only: the connect did not complete within the client's connect
    /// timeout, or the OS reported `ErrorKind::TimedOut`.
    Timeout,
    /// Fork-only: the endpoint exists but nothing accepts there
    /// (`ErrorKind::ConnectionRefused`).
    Refused,
    /// Fork-only: the endpoint does not exist, for a unix socket a path
    /// nothing has bound (`ErrorKind::NotFound`).
    NotFound,
    /// Fork-only: any other io error, by its kind. Never carries `TimedOut`,
    /// `ConnectionRefused` or `NotFound`; those read as the variants above.
    Io(std::io::ErrorKind),
}

impl DialFailureKind {
    pub(crate) fn from_io_error(err: &std::io::Error) -> Self {
        match err.kind() {
            std::io::ErrorKind::TimedOut => Self::Timeout,
            std::io::ErrorKind::ConnectionRefused => Self::Refused,
            std::io::ErrorKind::NotFound => Self::NotFound,
            kind => Self::Io(kind),
        }
    }
}

/// Fork-only: why an established stream ended. Reserved for the clients'
/// disconnect reporting; no client emits it in this release.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisconnectReason {
    /// Fork-only: the peer closed the stream in an orderly way.
    PeerClosed,
    /// Fork-only: the connection was reset (`ErrorKind::ConnectionReset`).
    Reset,
    /// Fork-only: the stream went quiet past the client's read timeout.
    Timeout,
    /// Fork-only: any other io error, by its kind. Never carries
    /// `ConnectionReset` or `TimedOut`; those read as the variants above.
    Io(std::io::ErrorKind),
}

/// Fork-only: one failed dial attempt, as a stream client reports it on the
/// channel given to its `with_dial_failure_events`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DialFailure {
    iface: AddressHash,
    kind: DialFailureKind,
}

impl DialFailure {
    pub(crate) fn new(iface: AddressHash, kind: DialFailureKind) -> Self {
        Self { iface, kind }
    }

    /// Fork-only: the address of the interface whose dial failed.
    #[must_use]
    pub fn iface(&self) -> AddressHash {
        self.iface
    }

    /// Fork-only: why the dial failed.
    #[must_use]
    pub fn kind(&self) -> DialFailureKind {
        self.kind
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Error, ErrorKind};

    use super::DialFailureKind;

    #[test]
    fn the_three_named_kinds_read_as_their_own_variants() {
        assert_eq!(
            DialFailureKind::from_io_error(&Error::from(ErrorKind::TimedOut)),
            DialFailureKind::Timeout
        );
        assert_eq!(
            DialFailureKind::from_io_error(&Error::from(ErrorKind::ConnectionRefused)),
            DialFailureKind::Refused
        );
        assert_eq!(
            DialFailureKind::from_io_error(&Error::from(ErrorKind::NotFound)),
            DialFailureKind::NotFound
        );
    }

    #[test]
    fn any_other_kind_reads_as_io_with_that_kind() {
        for kind in [ErrorKind::PermissionDenied, ErrorKind::AddrNotAvailable, ErrorKind::Other] {
            assert_eq!(
                DialFailureKind::from_io_error(&Error::from(kind)),
                DialFailureKind::Io(kind)
            );
        }
    }

    #[test]
    fn io_never_carries_one_of_the_three_named_kinds() {
        for kind in [ErrorKind::TimedOut, ErrorKind::ConnectionRefused, ErrorKind::NotFound] {
            assert_ne!(
                DialFailureKind::from_io_error(&Error::from(kind)),
                DialFailureKind::Io(kind)
            );
        }
    }
}
