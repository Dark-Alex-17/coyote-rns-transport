use super::*;
use crate::resource::ResourceAdvertisement;

/// Decides whether an inbound plain resource advertisement, one that is
/// neither a request nor a response, is accepted on the link. Once a hook
/// is installed it is consulted once per such advertisement, so once per
/// segment of a split resource, each segment advertising its own hash;
/// `false` refuses the transfer with a receiver cancel
/// (`RNS/Resource.py:155-165`, `Resource.reject`), exactly as the size caps
/// do. A hook that panics is logged at error and the advertisement is
/// dropped unanswered, neither accepted nor rejected, as
/// `RNS/Link.py:1073-1074` does with a callback that raises; the transport
/// and its workers carry on. Request and response advertisements are
/// governed by `max_request_size` and `max_response_size` instead
/// (`RNS/Link.py:1036-1064`); the hook is the `ACCEPT_APP` callback of
/// `RNS/Link.py:1066-1074` for everything else. The identity is the peer's,
/// when it has identified itself on the link. Without a hook every plain
/// advertisement is accepted, which is Python's `ACCEPT_ALL`
/// (`RNS/Link.py:1075-1076`); Python's default `ACCEPT_NONE`
/// (`RNS/Link.py:239`, `:1065`), which ignores the advertisement, has no
/// equivalent here, and a hook that always returns `false` is the closest.
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
    /// return promptly and must not call back into the transport. This is
    /// the `ACCEPT_APP` callback `RNS/Link.py:1066-1074` consults.
    pub async fn set_resource_accept_hook(&self, hook: ResourceAcceptHook) {
        self.handler.lock().await.resource_accept_hook = Some(hook);
    }
}
