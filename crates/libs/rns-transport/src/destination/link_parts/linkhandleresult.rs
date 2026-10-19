#[allow(clippy::large_enum_variant)]
pub enum LinkHandleResult {
    None,
    Activated,
    Proof(Packet),
    KeepAlive,
}

#[derive(Debug, PartialEq, Eq)]
#[allow(clippy::large_enum_variant)]
pub enum LinkWatchdogAction {
    None,
    SendKeepAlive,
    SendTeardown(Packet),
}

#[derive(Clone)]
#[non_exhaustive]
pub enum LinkEvent {
    Activated,
    Data(Box<LinkPayload>),
    PeerIdentified(Box<Identity>),
    /// Posted once, when the peer's proof names a plain Data packet
    /// (`PacketContext::None`) this side sent on the link and not yet proved.
    /// The peer proves plain Data packets (`RNS/Link.py:950-962`) and Channel
    /// frames (`RNS/Link.py:1147-1151`), but only the plain Data packets this
    /// side sent are recorded, so a Channel frame's proof, which the channel
    /// machinery consumes, a proof for a hash this side never sent and a
    /// replayed proof post nothing. Requests, responses and Resource packets
    /// are never proved. A link remembers at most 1024 unproved sent packets
    /// (Python's `Transport.MAX_RECEIPTS`, `RNS/Transport.py:159`), oldest
    /// dropped first, so a proof for a dropped record posts nothing.
    PacketProven {
        packet_hash: Hash,
    },
    Closed,
}

/// Why an RNS link entered its closed state. Numeric values match
/// `RNS.Link.TIMEOUT`, `INITIATOR_CLOSED`, and `DESTINATION_CLOSED`.
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
#[repr(u8)]
pub enum LinkCloseReason {
    Timeout = 0x01,
    InitiatorClosed = 0x02,
    DestinationClosed = 0x03,
}

impl LinkCloseReason {
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

#[derive(Clone)]
pub struct LinkEventData {
    pub id: LinkId,
    pub address_hash: AddressHash,
    pub event: LinkEvent,
    /// Present only for `LinkEvent::Closed` and matches the pinned Python
    /// `Link.teardown_reason` value.
    pub close_reason: Option<LinkCloseReason>,
}

/// When a record no receipt owns goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RecordClock {
    /// The clock runs; the watchdog prunes the record at this instant.
    Expires(Instant),
    /// A response Resource is inbound (the segment in flight and the whole
    /// Resource, as `InboundResponseHashes`); that Resource's own outcome
    /// clears the record, as `RNS/Link.py:1389` never times out `RECEIVING`.
    Receiving(InboundResponseHashes),
    /// No clock: the request Resource is still in flight (`OutboundComplete`
    /// arms it, `RNS/Link.py:1370-1373`), or the timeout does not fit an `Instant`.
    Unarmed,
}

impl RecordClock {
    /// Whether the clock ran out by `now`; a stopped or unarmed clock never does.
    fn has_run_out(self, now: Instant) -> bool {
        matches!(self, Self::Expires(at) if at <= now)
    }
}

/// What a request left behind for its response: the `max_response_size`
/// it went out with, and when the record goes if no receipt owns it.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PendingResponseLimit {
    limit: Option<usize>,
    clock: RecordClock,
    /// The request Resource this record waits behind, for a request sent
    /// through `send_request_resource*`; its outcome arms or drops the record.
    request_resource: Option<Hash>,
}

impl PendingResponseLimit {
    /// The response Resource a record taken out of the table was receiving.
    fn into_receiving(self) -> Option<InboundResponseHashes> {
        match self.clock {
            RecordClock::Receiving(inbound) => Some(inbound),
            _ => None,
        }
    }
}

pub struct Link {
    id: LinkId,
    is_initiator: bool,
    close_reason: Option<LinkCloseReason>,
    destination: DestinationDesc,
    ingress_iface: Option<AddressHash>,
    priv_identity: PrivateIdentity,
    peer_identity: Identity,
    identified_peer_identity: Option<Identity>,
    derived_key: DerivedKey,
    session_cipher: Option<CachedFernet>,
    signalling: Option<[u8; LINK_MTU_SIZE]>,
    status: LinkStatus,
    establishment_started_at: Instant,
    establishment_timeout: Duration,
    request_time: Instant,
    rtt: Duration,
    activated_at: Option<Instant>,
    last_inbound: Option<Instant>,
    last_outbound: Option<Instant>,
    last_data: Option<Instant>,
    last_keepalive: Option<Instant>,
    last_proof: Option<Instant>,
    stale_since: Option<Instant>,
    keepalive: Duration,
    stale_time: Duration,
    next_channel_sequence: u16,
    next_channel_rx_sequence: u16,
    channel_open: bool,
    next_channel_handler_id: u64,
    channel_handlers: HashMap<u16, Vec<RegisteredChannelHandler>>,
    channel_pending: HashMap<Hash, PendingChannelPacket>,
    channel_states: HashMap<u16, ChannelMessageState>,
    channel_rx_ring: HashMap<u16, ChannelEnvelope>,
    /// The hashes of the plain Data packets (`PacketContext::None`) this side
    /// sent on the link whose proof has not arrived, oldest first. Python
    /// keeps one `PacketReceipt` per sent packet in `Transport.receipts` and
    /// matches a proof against the outstanding receipts of status `SENT`,
    /// removing the one it proves (`RNS/Transport.py:2748-2761`). Here the
    /// list is capped at `UNPROVEN_DATA_HASHES_CAP` entries, the oldest
    /// dropped first, Python's `Transport.MAX_RECEIPTS` (`RNS/Transport.py:159`,
    /// `:745-748`) applied per link; Python also fails a link receipt after
    /// `max(rtt * 6, 5 ms)` (`RNS/Packet.py:428`), which this list does not.
    unproven_data_hashes: VecDeque<Hash>,
    /// The responses this side is waiting for, by request id. An entry's
    /// `limit` is the `max_response_size` the request went out with, `None`
    /// for a request sent without one. An id absent here and from
    /// `pending_requests` solicited nothing. An entry with a receipt goes
    /// with the receipt. One no receipt owns keeps its own clock: it runs
    /// from the send for a request packet and from `OutboundComplete` for a
    /// request Resource (`RNS/Link.py:1370-1373`), stops while a response
    /// Resource is inbound, and that Resource's own outcome clears the
    /// entry; otherwise the watchdog prunes it at the default request
    /// timeout. Python's marker is the `RequestReceipt`: one `DELIVERED`
    /// past its timeout leaves `pending_requests` (`:1397-1405`), one
    /// `RECEIVING` is never timed out (`:1389`), and one that never reached
    /// `DELIVERED` stays, so a packet-form marker there can outlive this entry.
    pending_response_limits: HashMap<Vec<u8>, PendingResponseLimit>,
    pending_requests: HashMap<RequestId, PendingRequest>,
    /// Response Resources still inbound that a receipt or a record failed or
    /// displaced under the link lock left owed a cancel: an over-limit
    /// packet response, a request re-sent under a receiving id,
    /// `expect_response` over a receiving record. The ResourceManager is
    /// out of reach there, so the transport drains these on the link's
    /// next inbound packet, Resource packets included, and cancels each
    /// (`RNS/Link.py:1435`).
    owed_response_cancels: Vec<InboundResponseHashes>,
    request_dispatch: Option<tokio::sync::mpsc::Sender<DispatchItem>>,
    channel_window: u8,
    channel_window_max: u8,
    channel_window_min: u8,
    channel_window_flexibility: u8,
    channel_fast_rate_rounds: u8,
    channel_medium_rate_rounds: u8,
    event_tx: tokio::sync::broadcast::Sender<LinkEventData>,
}
