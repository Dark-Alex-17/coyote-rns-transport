//! Fork-only: typed reasons for a stream client's failed dial, and the
//! reasons for an ended stream, plus the transport-wide event each client
//! publishes when its connection state changes. Python's clients log such
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

/// Fork-only: one change in a stream client's connection state, read from
/// `Transport::interface_events`; `iface` is the interface the change
/// belongs to. Python has no such stream: its clients log state changes
/// (`RNS/Interfaces/LocalInterface.py:159-167`, `:255-266`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InterfaceEvent {
    iface: AddressHash,
    kind: InterfaceEventKind,
}

impl InterfaceEvent {
    pub(crate) fn new(iface: AddressHash, kind: InterfaceEventKind) -> Self {
        Self { iface, kind }
    }

    /// Fork-only: the interface whose state changed.
    #[must_use]
    pub fn iface(&self) -> AddressHash {
        self.iface
    }

    /// Fork-only: what changed.
    #[must_use]
    pub fn kind(&self) -> InterfaceEventKind {
        self.kind
    }
}

/// Fork-only: what changed on a stream client interface. Published by a
/// dialing `TcpClient` or `LocalUnixClient` only. A client built from a
/// pre-supplied stream (`TcpClient::new_from_stream`,
/// `LocalUnixClient::new_from_stream`; how `TcpServer` and
/// `LocalUnixServer` run each accepted peer) publishes nothing for that
/// stream, and every other interface kind publishes nothing yet.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterfaceEventKind {
    /// Fork-only: a dial attempt is starting, once per attempt, redials
    /// included.
    Connecting,
    /// Fork-only: a stream to the endpoint was established, the first included.
    Connected,
    /// Fork-only: an established stream ended with `reason`, the same reason
    /// the client's `with_disconnect_reason_events` channel carries; a stream
    /// the client ended itself (cancel, stop, the transport's receive queue
    /// closing) publishes nothing.
    Disconnected { reason: DisconnectReason },
    /// Fork-only: one dial attempt failed with `reason`, the same kind the
    /// client's `with_dial_failure_events` channel carries.
    DialFailed { reason: DialFailureKind },
}

/// Handle a client clones out of its `InterfaceChannel` before `split()`:
/// the interface address plus the transport's broadcast sender, or `None`
/// for a channel minted by `InterfaceChannel::new` (tests, bare managers),
/// or for a client built from a pre-supplied stream, where publishing is a
/// no-op.
#[derive(Clone)]
pub(crate) struct InterfaceEventPublisher {
    iface: AddressHash,
    events: Option<tokio::sync::broadcast::Sender<InterfaceEvent>>,
}

impl InterfaceEventPublisher {
    pub(crate) fn new(
        iface: AddressHash,
        events: Option<tokio::sync::broadcast::Sender<InterfaceEvent>>,
    ) -> Self {
        Self { iface, events }
    }

    pub(crate) fn detached(iface: AddressHash) -> Self {
        Self::new(iface, None)
    }

    /// Publishes `kind` tagged with the interface; a detached publisher
    /// publishes nothing.
    pub(crate) fn publish(&self, kind: InterfaceEventKind) {
        let Some(events) = &self.events else { return };
        Self::send(events, self.iface, kind);
    }

    /// Publishes `kind` tagged with `iface` on `events`. `broadcast::Sender::send`
    /// is synchronous and never blocks; it fails only when no receiver is
    /// subscribed, the normal case for a transport whose owner never called
    /// `interface_events()`, so that error only earns a trace line.
    pub(crate) fn send(
        events: &tokio::sync::broadcast::Sender<InterfaceEvent>,
        iface: AddressHash,
        kind: InterfaceEventKind,
    ) {
        if events.send(InterfaceEvent::new(iface, kind)).is_err() {
            log::trace!(
                "interface event has no active subscribers iface={} kind={:?}",
                iface,
                kind
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Error, ErrorKind};

    use super::{
        DialFailureKind, DisconnectReason, InterfaceEvent, InterfaceEventKind,
        InterfaceEventPublisher,
    };
    use crate::hash::AddressHash;
    use crate::iface::tcp_client::HdlcStreamExit;
    use rand_core::OsRng;
    use tokio::sync::broadcast;
    use tokio::sync::broadcast::error::TryRecvError;

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

    #[test]
    fn interface_event_getters_return_the_tagged_fields() {
        let iface = AddressHash::new_from_rand(OsRng);
        let kind = InterfaceEventKind::Disconnected { reason: DisconnectReason::PeerClosed };
        let event = InterfaceEvent::new(iface, kind);
        assert_eq!(event.iface(), iface);
        assert_eq!(event.kind(), kind);
    }

    #[test]
    fn a_publisher_without_a_stream_publishes_nothing() {
        let publisher = InterfaceEventPublisher::new(AddressHash::new_from_rand(OsRng), None);
        publisher.publish(InterfaceEventKind::Connecting);
        publisher.publish(InterfaceEventKind::DialFailed { reason: DialFailureKind::Refused });
    }

    #[test]
    fn a_detached_publisher_publishes_nothing() {
        let detached = InterfaceEventPublisher::detached(AddressHash::new_from_rand(OsRng));
        detached.publish(InterfaceEventKind::Connected);
        detached.publish(InterfaceEventKind::Disconnected { reason: DisconnectReason::PeerClosed });
    }

    #[test]
    fn a_publisher_sends_to_a_subscriber() {
        let iface = AddressHash::new_from_rand(OsRng);
        let (tx, mut rx) = broadcast::channel(4);
        let publisher = InterfaceEventPublisher::new(iface, Some(tx));
        publisher.publish(InterfaceEventKind::Connected);
        assert_eq!(rx.try_recv(), Ok(InterfaceEvent::new(iface, InterfaceEventKind::Connected)));
        assert_eq!(rx.try_recv(), Err(TryRecvError::Empty));
    }
}
