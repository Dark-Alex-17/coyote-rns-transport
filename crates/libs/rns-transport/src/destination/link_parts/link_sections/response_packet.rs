// Kept separate from `new.rs` so that module stays below the repository's
// 500-line limit — the same reason `identify_packet_tests.rs` is its own
// file. `include!`d into the same module, so this is a file boundary and
// not a privacy one.

impl Link {
    /// Build a request packet (context = `Request`) carrying `data`.
    ///
    /// The mirror of [`Self::response_packet`], and missing for the same
    /// reason. Real Reticulum makes the same size-based choice on the way
    /// out (`RNS/Link.py`, `request`):
    ///
    /// ```text
    /// packed_request = umsgpack.packb([time, path_hash, data])
    /// if len(packed_request) <= self.mdu:
    ///     RNS.Packet(self, packed_request, RNS.Packet.DATA,
    ///                context = RNS.Packet.REQUEST).send()
    /// else:
    ///     request_resource = RNS.Resource(packed_request, self, ...)
    /// ```
    ///
    /// **The two branches identify the request differently, and a caller
    /// has to know it.** Sent as a resource, the request id is the
    /// requester's own `truncated_hash(packed_request)`. Sent as a packet,
    /// there is no such field on the wire — the responder derives the id
    /// from the packet's hash (`handle_packet`, below, takes the first
    /// [`ADDRESS_HASH_SIZE`] bytes of it), so a requester correlating the
    /// response must use `packet.hash()` from the packet this returns
    /// rather than an id of its own choosing.
    pub fn request_packet(&self, data: &[u8]) -> Result<Packet, RnsError> {
        self.packet_with_context(data, PacketContext::Request)
    }

    /// Build a request packet and record the optional Python-compatible
    /// `max_response_size` under the request id derived from its packet
    /// hash. Without a limit, [`Self::request_packet`] solicits the same
    /// response once the transport sends it; see
    /// the crate-private `Link::note_request_sent`.
    pub fn request_packet_with_max_response_size(
        &mut self,
        data: &[u8],
        max_response_size: Option<usize>,
    ) -> Result<Packet, RnsError> {
        let packet = self.packet_with_context(data, PacketContext::Request)?;
        let request_id = RequestId::from_packet_hash(&packet.hash());
        self.expect_response(request_id.as_bytes(), max_response_size);
        Ok(packet)
    }

    /// Registers `max_response_size` for the response to `request_id`.
    /// Works in either order with `send_request_resource*` and
    /// `request_packet*`: on a record already made it changes the limit
    /// alone, never the clock or the request Resource, and a record made
    /// afterwards keeps the limit.
    pub fn set_response_size_limit(&mut self, request_id: &[u8], max_response_size: usize) {
        let clock = self.pending_response_expiry();
        self.pending_response_limits
            .entry(request_id.to_vec())
            .and_modify(|entry| entry.limit = Some(max_response_size))
            .or_insert(PendingResponseLimit {
                limit: Some(max_response_size),
                clock,
                request_resource: None,
            });
    }

    /// Records that a response to `request_id` is expected, bounded by
    /// `max_response_size` when one is given. Python keeps this on the
    /// `RequestReceipt` (`RNS/Link.py:1046-1049`); a request sent through
    /// `send_request_resource*` has no receipt, so the entry here (the
    /// crate-private `expect_response_to_resource` makes it for that form)
    /// is what tells its response advertisement from an unsolicited one.
    /// The entry lives for the default request timeout unless a receipt
    /// owns the id. A limit already recorded under `request_id` stays when
    /// none is given. A record already receiving a response Resource is
    /// replaced, and that transfer is cancelled on the link's next inbound
    /// packet.
    pub fn expect_response(&mut self, request_id: &[u8], max_response_size: Option<usize>) {
        let limit = max_response_size.or_else(|| self.response_size_limit(request_id));
        let displaced = self.replace_response_record(request_id, limit);
        self.owed_response_cancels.extend(displaced);
    }

    /// [`Self::expect_response`] that replaces the record under `request_id`
    /// whole, limit included. For `Transport::request`, whose receipt
    /// displaces whatever was under the id: a request sent without a limit
    /// under an id a displaced entry recorded one for does not inherit it.
    /// The public builders keep a limit set beforehand instead. Returns the
    /// response Resource the displaced record was receiving, for the caller
    /// to cancel.
    #[deny(clippy::await_holding_invalid_type)]
    #[must_use = "the hashes are for the caller to cancel"]
    pub(crate) fn replace_response_record(
        &mut self,
        request_id: &[u8],
        max_response_size: Option<usize>,
    ) -> Option<InboundResponseHashes> {
        let clock = self.pending_response_expiry();
        self.pending_response_limits
            .insert(
                request_id.to_vec(),
                PendingResponseLimit { limit: max_response_size, clock, request_resource: None },
            )
            .and_then(PendingResponseLimit::into_receiving)
    }

    /// [`Self::expect_response`] for a request leaving as the Resource
    /// `request_resource`: the entry's clock does not run until that
    /// Resource is delivered (`RNS/Link.py:1370-1373`), and its rejection,
    /// failure or cancel drops the entry. A limit already recorded under
    /// `request_id` stays when none is given. Returns the response Resource
    /// the displaced record was receiving, for the caller to cancel.
    #[deny(clippy::await_holding_invalid_type)]
    #[must_use = "the hashes are for the caller to cancel"]
    pub(crate) fn expect_response_to_resource(
        &mut self,
        request_id: &[u8],
        max_response_size: Option<usize>,
        request_resource: Hash,
    ) -> Option<InboundResponseHashes> {
        let limit = max_response_size.or_else(|| self.response_size_limit(request_id));
        self.pending_response_limits
            .insert(
                request_id.to_vec(),
                PendingResponseLimit {
                    limit,
                    clock: RecordClock::Unarmed,
                    request_resource: Some(request_resource),
                },
            )
            .and_then(PendingResponseLimit::into_receiving)
    }

    /// Records that the link sent a request packet under `request_id`. Every
    /// request packet solicits its response, as every `RNS.Link.request`
    /// registers a `RequestReceipt` (`RNS/Link.py:473-510`, `:1365`), so a
    /// packet built with [`Self::request_packet`] is admitted like any other.
    /// A limit recorded before the send stays in force; the send anchors the
    /// clock, and the entry lives for the default request timeout unless a
    /// receipt owns the id. Only an `Expires` clock is re-armed, run out or
    /// not: a re-send under the id of a record receiving its response, or of
    /// a request Resource still in flight, leaves that record as it is.
    #[deny(clippy::await_holding_invalid_type)]
    pub(crate) fn note_request_sent(&mut self, request_id: &[u8]) {
        let clock = self.pending_response_expiry();
        self.pending_response_limits
            .entry(request_id.to_vec())
            .and_modify(|entry| {
                if let RecordClock::Expires(_) = entry.clock {
                    entry.clock = clock;
                }
            })
            .or_insert(PendingResponseLimit { limit: None, clock, request_resource: None });
    }

    /// The clock of a response entry armed now, if no receipt owns it:
    /// the default request timeout (`RNS/Link.py:491`) from now.
    fn pending_response_expiry(&self) -> RecordClock {
        Instant::now()
            .checked_add(RequestOptions::default().timeout_for_rtt(self.rtt()))
            .map_or(RecordClock::Unarmed, RecordClock::Expires)
    }

    /// Whether a response Resource advertising `data_size` bytes for
    /// `request_id` may be received (`RNS/Link.py:1046-1049`, `read_size`
    /// is `d`, `RNS/Resource.py:1285-1287`): `None` when nothing solicited
    /// it, or when the entry's clock ran out ahead of the watchdog,
    /// `Some(false)` when it is over the limit the request went out with.
    /// Checked at every segment advertisement, so nothing is removed here;
    /// the entry goes when the response concludes.
    pub fn response_size_allowed(&self, request_id: &[u8], data_size: usize) -> Option<bool> {
        let owned = owned_by_receipt(&self.pending_requests, request_id);
        match self.pending_response_limits.get(request_id) {
            Some(entry) if owned || !entry.clock.has_run_out(Instant::now()) => {
                Some(entry.limit.is_none_or(|limit| data_size <= limit))
            }
            Some(_) => None,
            None => owned.then_some(true),
        }
    }

    /// The `max_response_size` the request `request_id` went out with.
    #[deny(clippy::await_holding_invalid_type)]
    pub(crate) fn response_size_limit(&self, request_id: &[u8]) -> Option<usize> {
        self.pending_response_limits.get(request_id).and_then(|entry| entry.limit)
    }

    /// Removes the record under `request_id` for the response packet of
    /// `response_size` bytes and tells whether the packet is admitted: a
    /// missing or unbounded record admits it. A record refusing the packet
    /// while receiving a response Resource has that transfer cancelled by
    /// the transport once the packet is handled.
    pub fn take_response_limit_if_allowed(
        &mut self,
        request_id: &[u8],
        response_size: usize,
    ) -> bool {
        let (allowed, refused_while_receiving) =
            self.take_response_limit(request_id, response_size);
        self.owed_response_cancels.extend(refused_while_receiving);
        allowed
    }

    /// [`Self::take_response_limit_if_allowed`] that hands the response
    /// Resource a refusing record was receiving to the caller to cancel. An
    /// admitted packet leaves that Resource to run, as it does for a
    /// receipt.
    #[deny(clippy::await_holding_invalid_type)]
    #[must_use = "the hashes are for the caller to cancel"]
    pub(crate) fn take_response_limit(
        &mut self,
        request_id: &[u8],
        response_size: usize,
    ) -> (bool, Option<InboundResponseHashes>) {
        let Some(entry) = self.pending_response_limits.remove(request_id) else {
            return (true, None);
        };
        let allowed = entry.limit.is_none_or(|limit| response_size <= limit);
        (allowed, if allowed { None } else { entry.into_receiving() })
    }

    pub fn clear_response_size_limit(&mut self, request_id: &[u8]) {
        self.pending_response_limits.remove(request_id);
    }

    /// Removes the record under `request_id` and returns the response
    /// Resource it was receiving, for the caller to cancel: a later segment
    /// refused by the limit would otherwise leave the earlier segments'
    /// partial assembly behind in the ResourceManager.
    #[deny(clippy::await_holding_invalid_type)]
    #[must_use = "the hashes are for the caller to cancel"]
    pub(crate) fn take_response_record(
        &mut self,
        request_id: &[u8],
    ) -> Option<InboundResponseHashes> {
        self.pending_response_limits.remove(request_id)?.into_receiving()
    }

    /// Build a response packet (context = `Response`) carrying `data`.
    ///
    /// Real Reticulum answers a request with a single packet whenever the
    /// packed response fits the link MDU, and only falls back to a resource
    /// transfer when it does not (`RNS/Link.py`, `handle_request`):
    ///
    /// ```text
    /// packed_response = umsgpack.packb([request_id, response])
    /// if len(packed_response) <= self.mdu:
    ///     RNS.Packet(self, packed_response, RNS.Packet.DATA,
    ///                context = RNS.Packet.RESPONSE).send()
    /// else:
    ///     response_resource = RNS.Resource(packed_response, self, ...)
    /// ```
    ///
    /// `data` is that already-packed `[request_id, response]` envelope —
    /// the same bytes either branch carries, so a responder only chooses
    /// the mechanism, never the payload.
    ///
    /// The receive half has always been here: `handle_packet` decrypts
    /// `PacketContext::Response` and posts it as a `LinkEvent::Data`. Only
    /// the constructor was missing, so a responder built on this crate had
    /// to send every reply as a resource — several round trips and an
    /// advertisement for a payload that fits in one packet, which on a slow
    /// link costs far more than the bytes do.
    pub fn response_packet(&self, data: &[u8]) -> Result<Packet, RnsError> {
        self.packet_with_context(data, PacketContext::Response)
    }
}

fn response_envelope_metadata(data: &[u8]) -> Option<(Option<[u8; ADDRESS_HASH_SIZE]>, usize)> {
    let mut cursor = std::io::Cursor::new(data);
    let value = rmpv::decode::read_value(&mut cursor).ok()?;
    if cursor.position() != data.len() as u64 {
        return None;
    }
    let values = value.as_array()?;
    let request_id = values.first()?.as_slice()?;
    let request_id = request_id.try_into().ok();
    let response = values.get(1)?;
    let mut packed_response = Vec::new();
    rmpv::encode::write_value(&mut packed_response, response).ok()?;
    Some((request_id, packed_response.len()))
}
