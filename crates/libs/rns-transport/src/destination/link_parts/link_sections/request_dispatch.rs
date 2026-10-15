impl Link {
    /// Hands inbound requests to the transport's dispatch worker. Set by the
    /// transport on the links it accepts; an initiator-side link has none.
    pub(crate) fn set_request_dispatch(
        &mut self,
        sender: tokio::sync::mpsc::Sender<DispatchItem>,
    ) {
        self.request_dispatch = Some(sender);
    }

    /// Queues a decrypted request for the dispatch worker without blocking:
    /// a full queue drops it with a debug log. Called under the link lock, so
    /// it must never wait.
    fn dispatch_request(&self, request_id: RequestId, plain_text: &[u8]) {
        let Some(sender) = &self.request_dispatch else {
            return;
        };
        let item = DispatchItem {
            link_id: self.id,
            destination_hash: self.destination.address_hash,
            request_id,
            plain_text: plain_text.to_vec(),
            remote_identity: self.identified_peer_identity,
            form: DispatchForm::Packet,
        };
        match sender.try_send(item) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => {
                log::debug!(
                    "link({}): request {} dropped, dispatch queue full",
                    self.id,
                    request_id
                );
            }
            Err(TrySendError::Closed(_)) => {
                log::debug!(
                    "link({}): request {} dropped, dispatch worker gone",
                    self.id,
                    request_id
                );
            }
        }
    }
}
