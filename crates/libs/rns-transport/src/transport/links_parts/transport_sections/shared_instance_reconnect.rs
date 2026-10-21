use tokio::sync::mpsc;
use tokio::task::JoinSet;

/// Python's shared-instance client re-announces `RECONNECT_WAIT + 2` seconds
/// after it reconnects (`RNS/Interfaces/LocalInterface.py:177-180`); this
/// is the 2, added to the attach's `reconnect_wait`.
const SHARED_INSTANCE_REAPPEARED_ANNOUNCE_GRACE: Duration = Duration::from_secs(2);

impl Transport {
    /// Spawns the task that reacts to the attached client's reconnect and
    /// disconnect events; see [`SharedInstanceReconnectConsumer::run`].
    fn spawn_shared_instance_reconnect_consumer(
        &self,
        iface: AddressHash,
        reconnect_wait: Duration,
        reconnects: mpsc::Receiver<AddressHash>,
        disconnects: mpsc::Receiver<AddressHash>,
    ) {
        let consumer = SharedInstanceReconnectConsumer {
            handler: self.handler.clone(),
            iface_manager: self.iface_manager.clone(),
            cancel: self.cancel.clone(),
            name: self.name.clone(),
            tasks: self.shared_instance_tasks.clone(),
            iface,
            reconnect_wait,
        };
        spawn_shared_instance_task(&self.shared_instance_tasks, consumer.run(reconnects, disconnects));
    }
}

/// Spawns `future` into the transport's shared-instance task set, the set
/// [`Transport::stop`] joins. The tasks already finished are reaped first so
/// the set holds only live ones; a task that ended by panic is reported
/// here, since nothing else joins it. The std guard is released before the
/// caller awaits anything.
fn spawn_shared_instance_task<F>(tasks: &std::sync::Mutex<JoinSet<()>>, future: F)
where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    let mut tasks = tasks.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    while let Some(finished) = tasks.try_join_next() {
        if let Err(err) = finished {
            log::warn!("[tp] shared-instance task did not finish cleanly: {err}");
        }
    }
    tasks.spawn(future);
}

/// Resynthesizes the tunnel on `iface` and returns whether the interface
/// was still registered. The identity is copied out under the handler
/// guard and the interface hash read under the iface_manager guard, one
/// after the other, before the send takes the handler again.
#[deny(clippy::await_holding_invalid_type)]
pub(crate) async fn synthesize_tunnel_on_iface(
    handler: &Arc<Mutex<TransportHandler>>,
    iface: AddressHash,
) -> bool {
    let (identity, iface_manager) = {
        let handler = handler.lock().await;
        (handler.config.identity.clone(), handler.iface_manager.clone())
    };
    let Some(interface_hash) = iface_manager.lock().await.full_hash(&iface) else {
        return false;
    };
    let packet = super::tunnels::synthesize_tunnel_packet(&identity, interface_hash);
    super::request_dispatch::send_on_iface(handler, iface, packet).await;
    true
}

/// What one attach's consumer task carries: the task outlives the attach
/// call, so it holds clones of the transport's parts rather than a borrow.
#[derive(Clone)]
pub(super) struct SharedInstanceReconnectConsumer {
    pub(super) handler: Arc<Mutex<TransportHandler>>,
    pub(super) iface_manager: Arc<Mutex<InterfaceManager>>,
    pub(super) cancel: CancellationToken,
    pub(super) name: String,
    pub(super) tasks: Arc<std::sync::Mutex<JoinSet<()>>>,
    pub(super) iface: AddressHash,
    pub(super) reconnect_wait: Duration,
}

impl SharedInstanceReconnectConsumer {
    /// Serves the client's events until the transport is cancelled or the
    /// client worker exits. Both senders belong to that one worker and drop
    /// with it, after a detach, `stop_interface` or `stop`, so the first
    /// closed channel ends the task. A disconnect queued together with the
    /// reconnect that follows it is served first, so the flush of one drop
    /// cycle always precedes that cycle's tunnel re-synthesis.
    #[deny(clippy::await_holding_invalid_type)]
    pub(super) async fn run(
        self,
        mut reconnects: mpsc::Receiver<AddressHash>,
        mut disconnects: mpsc::Receiver<AddressHash>,
    ) {
        loop {
            // Biased so a disconnect is never overtaken by the reconnect
            // queued right behind it.
            tokio::select! {
                biased;
                _ = self.cancel.cancelled() => break,
                disconnected = disconnects.recv() => {
                    if disconnected.is_none() {
                        break;
                    }
                    self.on_disconnected().await;
                }
                reconnected = reconnects.recv() => {
                    if reconnected.is_none() {
                        break;
                    }
                    self.on_reconnected().await;
                }
            }
        }
    }

    /// Python's `is_connected_to_shared_instance` guard
    /// (`RNS/Transport.py:3706`) together with the fork's equivalent of its
    /// `not self.detached` (`RNS/Interfaces/LocalInterface.py:259`): the
    /// interface is still the one flagged as the shared instance. The two
    /// guards are taken one after the other, never together.
    #[deny(clippy::await_holding_invalid_type)]
    async fn attached(&self) -> bool {
        let connected = self.handler.lock().await.config.connected_to_shared_instance;
        connected && self.iface_manager.lock().await.is_shared_instance(&self.iface)
    }

    /// The tunnel goes out at once, as reticulumd does for its own clients;
    /// the re-announce waits like Python's reconnect job
    /// (`RNS/Interfaces/LocalInterface.py:177-180`).
    #[deny(clippy::await_holding_invalid_type)]
    async fn on_reconnected(&self) {
        if synthesize_tunnel_on_iface(&self.handler, self.iface).await {
            log::debug!(
                "tp({}): shared instance iface={} reconnected, tunnel re-synthesized",
                self.name,
                self.iface
            );
        } else {
            log::debug!(
                "tp({}): shared instance iface={} reconnected but no longer registered, tunnel not re-synthesized",
                self.name,
                self.iface
            );
        }
        let consumer = self.clone();
        let reannounce = async move {
            tokio::select! {
                _ = consumer.cancel.cancelled() => {}
                _ = tokio::time::sleep(
                    consumer.reconnect_wait + SHARED_INSTANCE_REAPPEARED_ANNOUNCE_GRACE
                ) => consumer.announce_reappeared().await,
            }
        };
        spawn_shared_instance_task(&self.tasks, reannounce);
    }

    /// `shared_connection_reappeared` (`RNS/Transport.py:3705-3709`): every
    /// registered single destination is announced as a path response, if
    /// the transport is still attached when the delay has elapsed.
    #[deny(clippy::await_holding_invalid_type)]
    async fn announce_reappeared(&self) {
        if !self.attached().await {
            log::debug!(
                "tp({}): shared instance iface={} detached before the reappearance delay elapsed, no re-announce",
                self.name,
                self.iface
            );
            return;
        }
        let announced = announce_all_destinations_as_path_response(&self.handler).await;
        log::debug!(
            "tp({}): shared instance iface={} reappeared, {} destination(s) re-announced as path responses",
            self.name,
            self.iface,
            announced
        );
    }

    /// `shared_connection_disappeared` (`RNS/Transport.py:3690-3702`), run
    /// only while attached as Python's `receive` does
    /// (`RNS/Interfaces/LocalInterface.py:255-261`): every active and
    /// pending link is closed with the reason of its own side, as
    /// `Link.teardown` sets it (`RNS/Link.py:670-671`), and the tables are
    /// emptied. Python's tables map to the fork's as follows:
    /// `active_links` and `pending_links` are `in_links` and `out_links`
    /// (both hold links in every status); `announce_table`, `path_table`
    /// and `tunnels` are the fields of the same name; `reverse_table` is
    /// kept on the `link_table` entries, so clearing that table covers it;
    /// `held_announces` are the held announces of `announce_limits`. The
    /// resource state of each closed link goes with it, as the link
    /// maintenance job does. Lock discipline follows
    /// `fail_pending_requests_on_stop`: the handler and each link are
    /// locked one after the other, never together, so the link set is
    /// snapshotted first and only those links are removed afterwards.
    #[deny(clippy::await_holding_invalid_type)]
    async fn on_disconnected(&self) {
        if !self.attached().await {
            log::debug!(
                "tp({}): shared instance iface={} stream ended after detach, tables kept",
                self.name,
                self.iface
            );
            return;
        }
        let (in_links, out_links) = {
            let handler = self.handler.lock().await;
            (snapshot_links(&handler.in_links), snapshot_links(&handler.out_links))
        };
        let mut closed = Vec::with_capacity(in_links.len() + out_links.len());
        for (_, link) in in_links.iter().chain(&out_links) {
            let mut link = link.lock().await;
            link.close();
            closed.push(*link.id());
        }
        let (paths, announces, tunnels, held) = {
            let mut handler = self.handler.lock().await;
            for (link_id, _) in &in_links {
                handler.in_links.remove(link_id);
            }
            for (destination, _) in &out_links {
                handler.out_links.remove(destination);
            }
            for link_id in &closed {
                handler.resource_manager.remove_link_state(*link_id);
            }
            let events = handler.resource_manager.drain_events();
            super::resource_wire::publish_resource_events(&handler, events);
            let paths = handler.path_table.clear();
            let announces = handler.announce_table.clear();
            handler.link_table.clear();
            let tunnels = handler.tunnel_table.clear();
            let held = handler.announce_limits.clear_held_announces();
            (paths, announces, tunnels, held)
        };
        log::debug!(
            "tp({}): shared instance iface={} disconnected, {} link(s) torn down, {} path(s), {} announce(s), {} tunnel(s), {} held announce(s) cleared",
            self.name,
            self.iface,
            closed.len(),
            paths,
            announces,
            tunnels,
            held
        );
    }
}

fn snapshot_links(
    links: &HashMap<AddressHash, Arc<Mutex<Link>>>,
) -> Vec<(AddressHash, Arc<Mutex<Link>>)> {
    links.iter().map(|(key, link)| (*key, link.clone())).collect()
}
