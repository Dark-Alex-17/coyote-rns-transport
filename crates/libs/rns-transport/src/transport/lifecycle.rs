use super::*;
use crate::destination::request::RequestError;

pub(super) const STOP_BUDGET: Duration = Duration::from_secs(5);

impl Transport {
    /// Shuts the transport down: detaches every interface and waits for their
    /// workers, then cancels the transport and waits for the management task.
    /// Interface workers go first because they may be blocked handing a packet
    /// to the management task, which has to stay alive to receive it. The
    /// interface wait, the management wait and the receipt sweep that follows
    /// share one 5 s budget; whatever is still running or still locked when it
    /// runs out is aborted or skipped and logged. On every link it can lock,
    /// provided the handler itself can be re-locked within the budget, the
    /// sweep fails every pending request `LinkClosed`, so a receipt the caller
    /// still holds concludes instead of waiting for a watchdog that no longer
    /// ticks, and concludes every outbound transfer `Failed(LinkClosed)`, so
    /// an outcome the caller still holds resolves. A receipt on a skipped
    /// link concludes `LinkClosed` once the link is dropped; an outcome on one
    /// resolves `Failed(LinkClosed)` once the transport is dropped. Calling it
    /// again is a no-op unless an attach raced the first call (below). A
    /// shared instance attached with [`Self::attach_shared_instance`] is
    /// forgotten once the management task has stopped, and the transport is
    /// no longer marked connected to one, since its interface is detached
    /// with the rest. An attach that starts once the transport is cancelled
    /// is refused. The interfaces are swept before the cancel, so an attach
    /// whose interface is spawned between the two leaves that interface
    /// registered and connected on the stopped transport with the slot empty
    /// and the posture off; [`Self::detach_shared_instance`] then finds
    /// nothing, and calling `stop()` again detaches it.
    #[deny(clippy::await_holding_invalid_type)]
    pub async fn stop(&self) {
        let deadline = Instant::now() + STOP_BUDGET;
        let workers = {
            let mut manager = self.iface_manager.lock().await;
            let handles: Vec<_> = manager
                .interface_hashes()
                .into_iter()
                .flat_map(|address| {
                    manager
                        .take_worker_tree(address)
                        .into_iter()
                        .map(move |handle| (address, handle))
                })
                .collect();
            manager.detach_interfaces();
            handles
        };
        for (address, mut worker) in workers {
            match time::timeout_at(deadline, &mut worker).await {
                Ok(Ok(())) => {}
                Ok(Err(err)) => log::warn!(
                    "[tp] interface worker did not finish cleanly during stop iface={} err={err}",
                    address.short()
                ),
                Err(_) => {
                    worker.abort();
                    log::warn!(
                        "[tp] interface worker still running after stop budget, aborted iface={}",
                        address.short()
                    );
                }
            }
        }
        self.cancel.cancel();
        let manage_task = {
            let mut slot = self.manage_task.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            slot.take()
        };
        if let Some(mut task) = manage_task {
            match time::timeout_at(deadline, &mut task).await {
                Ok(Ok(())) => {}
                Ok(Err(err)) => log::warn!(
                    "[tp] transport management task did not finish cleanly during stop: {err}"
                ),
                Err(_) => {
                    task.abort();
                    log::warn!(
                        "[tp] transport management task still running after stop budget, aborted"
                    );
                }
            }
        }
        match time::timeout_at(deadline, self.handler.lock()).await {
            Ok(mut handler) => {
                let attached = {
                    let mut slot = self
                        .shared_instance
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    slot.take()
                };
                if let Some(iface) = attached {
                    handler.config.set_connected_to_shared_instance(false);
                    self.log_shared_instance_cleared_by_stop(iface);
                }
            }
            Err(_) => {
                let mut slot =
                    self.shared_instance.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                if let Some(iface) = slot.take() {
                    log::warn!(
                        "[tp] handler still locked after stop budget, shared-instance iface={} posture not cleared",
                        iface.short()
                    );
                }
            }
        }
        self.fail_pending_requests_on_stop(deadline).await;
    }

    /// Fails every pending request `LinkClosed` and drops the response
    /// Resources their receipts were receiving; a record no receipt owns
    /// and a cancel still owed stay with the link state. No receiver cancel
    /// can leave: the interfaces are detached by now. Every other resource
    /// transfer on a swept link then ends as it would on a closed link: an
    /// outbound one concludes `Failed(LinkClosed)`, so an outcome the caller
    /// still holds resolves, and an inbound assembly fails `link_closed`; the
    /// response the receipt sweep cancelled keeps its `cancelled` reason
    /// because the sweep runs first. A link the sweep could not lock keeps
    /// its transfers, and their outcomes resolve only when the transport is
    /// dropped and the `ResourceManager` with it. The handler and each
    /// link are locked one after the other, never together, and each lock
    /// wait ends at `deadline`. `Timeout` polls the lock before it checks
    /// the deadline, so an uncontended lock is still taken after the budget
    /// has run out; only a contended one is skipped.
    #[deny(clippy::await_holding_invalid_type)]
    async fn fail_pending_requests_on_stop(&self, deadline: Instant) {
        let links: Vec<Arc<Mutex<Link>>> = match time::timeout_at(deadline, self.handler.lock())
            .await
        {
            Ok(handler) => {
                handler.in_links.values().chain(handler.out_links.values()).cloned().collect()
            }
            Err(_) => {
                log::warn!(
                    "[tp] handler still locked after stop budget, receipt and outbound outcome sweep skipped"
                );
                return;
            }
        };
        let mut swept = Vec::new();
        let mut receiving = Vec::new();
        let mut skipped = 0usize;
        for link in links {
            match time::timeout_at(deadline, link.lock()).await {
                Ok(mut link) => {
                    let link_id = *link.id();
                    swept.push(link_id);
                    receiving.extend(
                        link.fail_all_pending_requests(RequestError::LinkClosed)
                            .into_iter()
                            .map(|response| (link_id, response)),
                    );
                }
                Err(_) => skipped += 1,
            }
        }
        if skipped > 0 {
            log::warn!(
                "[tp] {skipped} link(s) still locked after stop budget, skipped by the receipt sweep; \
                 their pending requests and outbound outcomes conclude when the transport drops"
            );
        }
        let Ok(mut handler) = time::timeout_at(deadline, self.handler.lock()).await else {
            log::warn!(
                "[tp] handler still locked after stop budget, {} response receiver(s) not cancelled \
                 and no outbound outcome concluded",
                receiving.len()
            );
            return;
        };
        // No routed drain follows a stop; publishing the queue is all that is left.
        let mut events = handler.resource_manager.drain_events();
        for (link_id, response) in receiving {
            let cancelled = handler.resource_manager.cancel_incoming_response(
                response.segment,
                response.original,
                &link_id,
            );
            events.extend(cancelled.event);
        }
        for link_id in swept {
            handler.resource_manager.remove_link_state(link_id);
        }
        events.extend(handler.resource_manager.drain_events());
        super::resource_wire::publish_resource_events(&handler, events);
    }
}
