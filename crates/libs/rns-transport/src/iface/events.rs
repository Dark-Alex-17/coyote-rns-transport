//! Fork-only: typed reasons for a stream client's failed dial, and the
//! reasons for an ended stream. Python's clients log such
//! failures as text and retry (`RNS/Interfaces/LocalInterface.py:159-167`);
//! these types let a consumer tell a refused dial from a missing socket.

use super::tcp_client::HdlcStreamExit;
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

/// Fork-only: why an established stream ended, as `TcpClient` and
/// `LocalUnixClient` report it on the channel given to their
/// `with_disconnect_reason_events`. A stream the client ended itself (a
/// cancel or a stop, or the transport's receive queue closing, which only
/// the plain channel reports) is reported on no channel. `Timeout` reaches a
/// consumer of a `TcpClient` built with a read watchdog
/// (`with_backbone_liveness`), or of either client when a read or write
/// fails with `ErrorKind::TimedOut`; the unix client runs no read watchdog.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisconnectReason {
    /// Fork-only: the peer closed the stream in an orderly way.
    PeerClosed,
    /// Fork-only: the connection was reset (`ErrorKind::ConnectionReset`); a
    /// reset observed first on the write side reads as `Io(BrokenPipe)`.
    Reset,
    /// Fork-only: the stream went quiet past the client's read timeout, or
    /// a read or write failed with `ErrorKind::TimedOut`.
    Timeout,
    /// Fork-only: any other io error, by its kind. Never carries
    /// `ConnectionReset` or `TimedOut`; those read as the variants above.
    Io(std::io::ErrorKind),
}

impl DisconnectReason {
    /// How a stream's exit reads to a consumer; `None` for a stream the
    /// client ended itself (cancel, stop, receive queue closed), which the
    /// clients report on no channel.
    pub(crate) fn from_stream_exit(exit: HdlcStreamExit) -> Option<Self> {
        match exit {
            HdlcStreamExit::PeerClosed => Some(Self::PeerClosed),
            HdlcStreamExit::ReadTimeout => Some(Self::Timeout),
            HdlcStreamExit::ReadError(std::io::ErrorKind::ConnectionReset)
            | HdlcStreamExit::WriteError(std::io::ErrorKind::ConnectionReset) => Some(Self::Reset),
            HdlcStreamExit::ReadError(std::io::ErrorKind::TimedOut)
            | HdlcStreamExit::WriteError(std::io::ErrorKind::TimedOut) => Some(Self::Timeout),
            HdlcStreamExit::ReadError(kind) | HdlcStreamExit::WriteError(kind) => {
                Some(Self::Io(kind))
            }
            HdlcStreamExit::Cancelled => None,
        }
    }
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

    use super::{DialFailureKind, DisconnectReason};
    use crate::iface::tcp_client::HdlcStreamExit;

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

    #[test]
    fn a_peer_close_and_a_read_timeout_read_as_their_own_reasons() {
        assert_eq!(
            DisconnectReason::from_stream_exit(HdlcStreamExit::PeerClosed),
            Some(DisconnectReason::PeerClosed)
        );
        assert_eq!(
            DisconnectReason::from_stream_exit(HdlcStreamExit::ReadTimeout),
            Some(DisconnectReason::Timeout)
        );
    }

    #[test]
    fn a_reset_or_timed_out_io_error_on_either_side_reads_as_reset_or_timeout() {
        for exit in [
            HdlcStreamExit::ReadError(ErrorKind::ConnectionReset),
            HdlcStreamExit::WriteError(ErrorKind::ConnectionReset),
        ] {
            assert_eq!(DisconnectReason::from_stream_exit(exit), Some(DisconnectReason::Reset));
        }
        for exit in [
            HdlcStreamExit::ReadError(ErrorKind::TimedOut),
            HdlcStreamExit::WriteError(ErrorKind::TimedOut),
        ] {
            assert_eq!(DisconnectReason::from_stream_exit(exit), Some(DisconnectReason::Timeout));
        }
    }

    #[test]
    fn any_other_io_error_reads_as_io_with_that_kind() {
        for kind in [ErrorKind::BrokenPipe, ErrorKind::ConnectionAborted, ErrorKind::Other] {
            assert_eq!(
                DisconnectReason::from_stream_exit(HdlcStreamExit::ReadError(kind)),
                Some(DisconnectReason::Io(kind))
            );
            assert_eq!(
                DisconnectReason::from_stream_exit(HdlcStreamExit::WriteError(kind)),
                Some(DisconnectReason::Io(kind))
            );
        }
    }

    #[test]
    fn io_never_carries_reset_or_timed_out() {
        for kind in [ErrorKind::ConnectionReset, ErrorKind::TimedOut] {
            for exit in [HdlcStreamExit::ReadError(kind), HdlcStreamExit::WriteError(kind)] {
                assert_ne!(
                    DisconnectReason::from_stream_exit(exit),
                    Some(DisconnectReason::Io(kind))
                );
            }
        }
    }

    #[test]
    fn a_locally_ended_stream_has_no_reason() {
        assert_eq!(DisconnectReason::from_stream_exit(HdlcStreamExit::Cancelled), None);
    }
}
