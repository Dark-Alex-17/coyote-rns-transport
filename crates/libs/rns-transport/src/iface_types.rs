pub use driver::{InterfaceDriver, InterfaceDriverFactory};

/// Configured endpoint data for app boundaries that render a compatible
/// TCP-client path-table interface description. It describes only that
/// configured interface, not virtual interfaces hosted by it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TcpClientPathTableMetadata {
    pub name: String,
    pub target_host: String,
    pub target_port: u16,
}

pub type InterfaceTxSender = mpsc::Sender<TxMessage>;
pub type InterfaceTxReceiver = mpsc::Receiver<TxMessage>;

pub type InterfaceRxSender = mpsc::Sender<RxMessage>;
pub type InterfaceRxReceiver = mpsc::Receiver<RxMessage>;

/// Receives the next frame from a management channel a driver may not have.
/// Callers gate the `select!` arm on `is_some()`, so the `None` arm only
/// documents the absent case.
pub(crate) async fn recv_optional_management_frame<T>(
    receiver: Option<&mut mpsc::Receiver<T>>,
) -> Option<T> {
    match receiver {
        Some(receiver) => receiver.recv().await,
        None => None,
    }
}
