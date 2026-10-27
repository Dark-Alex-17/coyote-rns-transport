use tokio::sync::mpsc;
use tokio::task::JoinSet;

/// Python's shared-instance client re-announces `RECONNECT_WAIT + 2` seconds
/// after it reconnects (`RNS/Interfaces/LocalInterface.py:178-181`); this
/// is the 2, added to the attach's `reconnect_wait`.
const SHARED_INSTANCE_REAPPEARED_ANNOUNCE_GRACE: Duration = Duration::from_secs(2);

impl Transport {
    /// Spawns the task that reacts to the attached client's reconnect,
    /// connect and disconnect events and forwards the attach's state
    /// events on `state_events`; see [`SharedInstanceReconnectConsumer::run`].
    fn spawn_shared_instance_reconnect_consumer(
        &self,
        iface: AddressHash,
        reconnect_wait: Duration,
        reconnects: mpsc::Receiver<AddressHash>,
        connects: mpsc::Receiver<AddressHash>,
        disconnects: mpsc::Receiver<AddressHash>,
        state_events: Option<mpsc::Sender<SharedInstanceEvent>>,
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
        spawn_shared_instance_task(
            &self.shared_instance_tasks,
            &self.name,
            consumer.run(reconnects, connects, disconnects, state_events),
        );
    }
}

/// Spawns `future` into the transport's shared-instance task set, the set
/// [`Transport::stop`] joins. The tasks already finished are reaped first so
/// the set holds only live ones; a task that ended by panic is reported
/// here under the transport's `name`, since nothing else joins it. The std
/// guard is released before the caller awaits anything.
fn spawn_shared_instance_task<F>(tasks: &std::sync::Mutex<JoinSet<()>>, name: &str, future: F)
where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    let mut tasks = tasks.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    while let Some(finished) = tasks.try_join_next() {
        if let Err(err) = finished {
            log::warn!("tp({name}): shared instance task did not finish cleanly: {err}");
        }
    }
    tasks.spawn(future);
}

/// Delivers `event` on the attach's state stream without blocking: a full
/// channel drops it at debug and a closed one, whose receiver is gone, is
/// silent. `None` is an attach without a stream.
fn send_shared_instance_state_event(
    name: &str,
    iface: AddressHash,
    events: Option<&mpsc::Sender<SharedInstanceEvent>>,
    event: SharedInstanceEvent,
) {
    let Some(events) = events else {
        return;
    };
    match events.try_send(event) {
        Ok(()) | Err(mpsc::error::TrySendError::Closed(_)) => {}
        Err(mpsc::error::TrySendError::Full(event)) => {
            log::debug!(
                "tp({}): shared instance iface={} state event {:?} dropped, channel full",
                name,
                iface,
                event
            );
        }
    }
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
    /// client worker exits. All three senders belong to that one worker and drop
    /// with it, after a detach, `stop_interface` or `stop`, so the first
    /// closed channel ends the task. The channels are `mpsc::channel(8)`
    /// and the worker emits each cycle's disconnect before its connect and
    /// reconnect from one task, each with `try_send`; this loop is biased so
    /// every queued disconnect is served before any connect, and every
    /// connect before any reconnect. So a disconnect
    /// can never be dropped while its reconnect lands: at each disconnect
    /// send the disconnect queue is no longer than the reconnect queue, so
    /// the reconnect channel fills first and the realistic loss under a
    /// storm is a reconnect, which the next reconnect that does land heals
    /// (the tunnel re-synthesis and the re-announce are idempotent). The
    /// connect channel, also `mpsc::channel(8)` and fed before the
    /// reconnect of the same cycle, is never longer than the reconnect
    /// queue once the first connect has been served, so a full connect
    /// channel means the reconnect channel is full too: the cycle's
    /// `Connected` goes with its reconnect, and the next cycle's connect
    /// heals the stream the same way. A per-cycle sequence number that
    /// would let the consumer detect a lost reconnect outright would
    /// change the public `Sender<AddressHash>` payload and is deferred.
    ///
    /// `state_events` is the attach's stream: a served disconnect sends
    /// `Disconnected` then `Connecting` once the flush has run, a served
    /// connect sends `Connected`, each with `try_send`. The sender is a
    /// parameter rather than a field so it drops when this task returns:
    /// the re-announce task clones `self` and outlives a detach by
    /// `reconnect_wait` plus the grace, and must not hold the stream open.
    /// The attach sends the first `Connecting` itself, before this task is
    /// spawned.
    #[deny(clippy::await_holding_invalid_type)]
    pub(super) async fn run(
        self,
        mut reconnects: mpsc::Receiver<AddressHash>,
        mut connects: mpsc::Receiver<AddressHash>,
        mut disconnects: mpsc::Receiver<AddressHash>,
        state_events: Option<mpsc::Sender<SharedInstanceEvent>>,
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
                    self.send_state_event(state_events.as_ref(), SharedInstanceEvent::Disconnected);
                    self.send_state_event(state_events.as_ref(), SharedInstanceEvent::Connecting);
                }
                connected = connects.recv() => {
                    if connected.is_none() {
                        break;
                    }
                    self.send_state_event(state_events.as_ref(), SharedInstanceEvent::Connected);
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

    fn send_state_event(
        &self,
        events: Option<&mpsc::Sender<SharedInstanceEvent>>,
        event: SharedInstanceEvent,
    ) {
        send_shared_instance_state_event(&self.name, self.iface, events, event);
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
    /// (`RNS/Interfaces/LocalInterface.py:178-181`). Unlike the re-announce,
    /// the tunnel re-synthesis is not posture-gated: it goes out for any
    /// interface that is still registered, as reticulumd's own reconnect
    /// does.
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
            // Biased so a cancel that lands with the delay is served first, as
            // the registration announce does; announce_reappeared re-checks the
            // posture anyway.
            tokio::select! {
                biased;
                _ = consumer.cancel.cancelled() => {}
                _ = tokio::time::sleep(
                    consumer.reconnect_wait + SHARED_INSTANCE_REAPPEARED_ANNOUNCE_GRACE
                ) => consumer.announce_reappeared().await,
            }
        };
        spawn_shared_instance_task(&self.tasks, &self.name, reannounce);
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
        let announced = announce_all_destinations_as_path_response(&self.handler, &self.name).await;
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
    /// pending link is torn down as `Link.teardown` does
    /// (`RNS/Transport.py:3692`, `:3695`, `RNS/Link.py:657-668`): a
    /// LINKCLOSE packet goes to the peer of each link past `Pending` on the
    /// interface the link rides, after every guard is released, and each
    /// link is closed with the reason of its own side (`:670-671`); the
    /// tables are emptied. Python's tables map to the fork's as follows:
    /// `active_links` and `pending_links` are `in_links` and `out_links`
    /// (both hold links in every status); `announce_table`, `path_table`
    /// and `tunnels` are the fields of the same name; `link_table` is the
    /// field of that name alone, and `reverse_table` is the packet cache's
    /// received-packet entries, the source notes and proof-destination
    /// index entries that `clear_reverse_routes` empties, while the
    /// duplicate-filter hashes stay, as Python's `packet_hashlist` does,
    /// and the entries of packets this node sent stay too: they are the
    /// receipt guard, Python's `receipts` list (`RNS/Transport.py:174`),
    /// not part of `reverse_table`;
    /// `held_announces` are the held announces of `announce_limits`. The
    /// resource state of each closed link goes with it, as the link
    /// maintenance job does. Lock discipline follows
    /// `fail_pending_requests_on_stop`: the handler and each link are
    /// locked one after the other, never together, and the packet cache is
    /// locked after the handler guard has been released; the teardown
    /// packets are sent last, after the packet cache guard. The links are
    /// taken out of both maps under the first handler guard and closed
    /// afterwards, so a link opened on the `Closed` event is not evicted by
    /// the clears that follow. A link riding the dropped interface itself
    /// sends no close packet: its peer is gone with the connection, and a
    /// packet queued on the client's channel would only be flushed to the
    /// restarted instance as a close for a link it never knew; such links
    /// are counted as skipped in the drop line. A close packet on another
    /// interface that does not go out is recorded at debug. Every link that
    /// reaches `Handshake`, `Active` or `Stale` has its ingress interface
    /// set by its producer, the accept at `transport/path.rs`
    /// (`set_ingress_iface`) or the proof at
    /// `destination/link_parts/link_sections/handle_proof_packet.rs`
    /// (`get_or_insert`), so a teardown packet with no interface to ride is
    /// a defensive no-op.
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
            let mut handler = self.handler.lock().await;
            (drain_links(&mut handler.in_links), drain_links(&mut handler.out_links))
        };
        let mut closed = Vec::with_capacity(in_links.len() + out_links.len());
        let mut teardowns: Vec<(AddressHash, Packet)> = Vec::new();
        for link in in_links.iter().chain(&out_links) {
            let mut link = link.lock().await;
            let iface = link.ingress_iface();
            let packet = link.teardown();
            closed.push(*link.id());
            if let (Some(iface), Some(packet)) = (iface, packet) {
                teardowns.push((iface, packet));
            }
        }
        let (paths, announces, links, tunnels, held, packet_cache) = {
            let mut handler = self.handler.lock().await;
            for link_id in &closed {
                handler.resource_manager.remove_link_state(*link_id);
            }
            let events = handler.resource_manager.drain_events();
            super::resource_wire::publish_resource_events(&handler, events);
            let paths = handler.path_table.clear();
            let announces = handler.announce_table.clear();
            let links = handler.link_table.clear();
            let tunnels = handler.tunnel_table.clear();
            let held = handler.announce_limits.clear_held_announces();
            (paths, announces, links, tunnels, held, handler.packet_cache.clone())
        };
        let reverse_routes = packet_cache.lock().await.clear_reverse_routes();
        let mut close_packets_sent = 0usize;
        let mut close_packets_skipped = 0usize;
        for (iface, packet) in teardowns {
            if iface == self.iface {
                close_packets_skipped += 1;
                continue;
            }
            let link_id = packet.destination;
            let trace = super::request_dispatch::send_on_iface(&self.handler, iface, packet).await;
            if trace.sent_ifaces > 0 {
                close_packets_sent += 1;
            } else {
                log::debug!(
                    "tp({}): shared instance iface={} disconnected, close packet for link={} not sent on iface={}",
                    self.name,
                    self.iface,
                    link_id,
                    iface
                );
            }
        }
        log::debug!(
            "tp({}): shared instance iface={} disconnected, {} link(s) torn down ({} close packet(s) sent, {} skipped on the dropped interface), {} path(s), {} announce(s), {} link table entr(ies), {} tunnel(s), {} held announce(s), {} reverse route(s) cleared",
            self.name,
            self.iface,
            closed.len(),
            close_packets_sent,
            close_packets_skipped,
            paths,
            announces,
            links,
            tunnels,
            held,
            reverse_routes
        );
    }
}

fn drain_links(links: &mut HashMap<AddressHash, Arc<Mutex<Link>>>) -> Vec<Arc<Mutex<Link>>> {
    links.drain().map(|(_, link)| link).collect()
}
