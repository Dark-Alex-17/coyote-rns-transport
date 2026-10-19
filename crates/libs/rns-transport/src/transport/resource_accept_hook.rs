use super::*;
use crate::resource::ResourceAdvertisement;

/// Decides whether an inbound plain resource advertisement, one that is
/// neither a request nor a response, is accepted on the link. Once a hook
/// is installed it is consulted for every such advertisement; `false`
/// refuses the transfer with a receiver cancel (`RNS/Resource.py:155-165`,
/// `Resource.reject`), exactly as the size caps do. Request and response
/// advertisements are governed by `max_request_size` and
/// `max_response_size` instead (`RNS/Link.py:1036-1064`); the hook is the
/// `ACCEPT_APP` callback of `RNS/Link.py:1065-1074` for everything else.
/// The identity is the peer's, when it has identified itself on the link.
/// Without a hook every plain advertisement is accepted.
pub type ResourceAcceptHook =
    Arc<dyn Fn(&ResourceAdvertisement, &LinkId, Option<&Identity>) -> bool + Send + Sync>;

impl Transport {
    /// Installs the handler delivery proofs are reported to. Takes `&self`:
    /// the handler lives behind the transport's own lock, and a `Transport`
    /// is shared as an `Arc` by the time a client is ready to install one.
    pub async fn set_receipt_handler(&self, handler: Box<dyn ReceiptHandler>) {
        self.handler.lock().await.receipt_handler = Some(Arc::from(handler));
    }

    pub fn emit_receipt_for_test(&self, receipt: DeliveryReceipt) {
        let receipt_handler = match self.handler.try_lock() {
            Ok(handler) => handler.receipt_handler.clone(),
            Err(err) => {
                log::warn!("[transport] failed to read receipt handler for test receipt: {err}");
                None
            }
        };

        if let Some(handler) = receipt_handler {
            handler.on_receipt(&receipt);
        }
    }

    /// Installs the [`ResourceAcceptHook`] consulted for every inbound plain
    /// resource advertisement from now on; a later call replaces it. The hook
    /// runs inside the inbound packet path with the link locked, so it must
    /// return promptly and must not call back into the transport.
    pub async fn set_resource_accept_hook(&self, hook: ResourceAcceptHook) {
        self.handler.lock().await.resource_accept_hook = Some(hook);
    }
}
