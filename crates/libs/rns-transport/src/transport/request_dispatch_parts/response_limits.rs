// The responder's response cap of `request_dispatch.rs`, `include!`d into
// that module so it stays below the repository's 500-line limit; a file
// boundary, not a privacy one.

/// What a response may be on its way out: the registration's `auto_compress`
/// flag and the destination's `max_response_size`, read in `dispatch_one`
/// and carried to the send as one value.
#[derive(Clone, Copy)]
struct ResponseLimits {
    auto_compress: bool,
    max_response_size: Option<usize>,
}

impl ResponseLimits {
    /// Fork-only: the refusal for a response of `len` bytes, `None` when it
    /// may go out. At the cap goes out. Logs the one warn line a refused
    /// response gets; the caller sends nothing.
    fn refusal(
        &self,
        link_id: AddressHash,
        request_id: RequestId,
        len: usize,
    ) -> Option<ResponseDispatch> {
        let max = self.max_response_size.filter(|max| len > *max)?;
        // The test logger in tests_parts/two_transport_loopback_requests_isolation.rs and
        // assert_one_refusal_warn in tests_parts/two_transport_loopback_requests_response_cap.rs
        // match this text by substring; reword all three together.
        log::warn!(
            "tp: response {} on link {} refused, {}B exceeds the destination response limit {}B",
            request_id,
            link_id,
            len,
            max
        );
        Some(ResponseDispatch::TooLarge { len, max })
    }
}
