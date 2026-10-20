impl Link {
    /// Hands inbound requests to the transport's dispatch worker. Set by the
    /// transport on the links it accepts; an initiator-side link has none.
    pub(crate) fn set_request_dispatch(
        &mut self,
        sender: tokio::sync::mpsc::Sender<DispatchItem>,
    ) {
        self.request_dispatch = Some(sender);
    }

    /// Queues a decrypted request for the dispatch worker without blocking.
    /// Only an active link hands a request on (`RNS/Link.py:805`), and a
    /// full queue drops it with a debug log. Called under the link lock, so
    /// it must never wait.
    pub(crate) fn dispatch_request(&self, form: DispatchForm, plain_text: &[u8]) {
        let Some(sender) = &self.request_dispatch else {
            return;
        };
        if self.status != LinkStatus::Active {
            log::debug!("link({}): {} ignored, link is not active", self.id, form);
            return;
        }
        let item = DispatchItem {
            link_id: self.id,
            destination_hash: self.destination.address_hash,
            plain_text: plain_text.to_vec(),
            remote_identity: self.identified_peer_identity,
            form,
        };
        match sender.try_send(item) {
            Ok(()) => {}
            Err(TrySendError::Full(item)) => {
                log::debug!("link({}): {} dropped, dispatch queue full", self.id, item.form);
            }
            Err(TrySendError::Closed(item)) => {
                log::debug!("link({}): {} dropped, dispatch worker gone", self.id, item.form);
            }
        }
    }
}
