pub struct Transport {
    name: String,
    link_in_event_tx: broadcast::Sender<LinkEventData>,
    link_out_event_tx: broadcast::Sender<LinkEventData>,
    received_data_tx: broadcast::Sender<ReceivedData>,
    iface_messages_tx: broadcast::Sender<RxMessage>,
    resource_events_tx: broadcast::Sender<ResourceEvent>,
    handler: Arc<Mutex<TransportHandler>>,
    iface_manager: Arc<Mutex<InterfaceManager>>,
    cancel: CancellationToken,
    manage_task: std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>,
    shared_instance: std::sync::Mutex<Option<AddressHash>>,
    /// Fork-only: the tasks the shared-instance client spawns, namely each
    /// attach's reconnect consumer, the delayed registration announces and
    /// the delayed re-announces after a reconnect. Python runs these as
    /// threads it never joins (`RNS/Transport.py:2911-2916`,
    /// `RNS/Interfaces/LocalInterface.py:178-181`); here [`Self::stop`] joins
    /// them within its budget, since a task that outlived the stop would
    /// announce on a stopped transport. Shared with the consumers, which
    /// spawn their re-announces through it.
    shared_instance_tasks: Arc<std::sync::Mutex<tokio::task::JoinSet<()>>>,
}
