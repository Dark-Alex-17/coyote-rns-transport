use super::*;
use crate::destination::request::RequestError;

const STOP_BUDGET: Duration = Duration::from_secs(5);

impl Transport {
    /// Shuts the transport down: detaches every interface and waits for their
    /// workers, then cancels the transport and waits for the management task.
    /// Interface workers go first because they may be blocked handing a packet
    /// to the management task, which has to stay alive to receive it. The
    /// interface and management waits share a 5 s budget; whatever is still
    /// running when it runs out is aborted and logged. The receipt sweep that
    /// follows takes the handler and each link lock once and has no budget of
    /// its own: every pending request on every link fails `LinkClosed`, so a
    /// receipt the caller still holds concludes instead of waiting for a
    /// watchdog that no longer ticks. Calling it again is a no-op.
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
        self.fail_pending_requests_on_stop().await;
    }

    /// Fails every pending request `LinkClosed` and drops the response
    /// Resources they were receiving. No receiver cancel can leave: the
    /// interfaces are detached by now. The handler and each link are locked
    /// one after the other, never together.
    #[deny(clippy::await_holding_invalid_type)]
    async fn fail_pending_requests_on_stop(&self) {
        let links: Vec<Arc<Mutex<Link>>> = {
            let handler = self.handler.lock().await;
            handler.in_links.values().chain(handler.out_links.values()).cloned().collect()
        };
        let mut receiving = Vec::new();
        for link in links {
            receiving.extend(link.lock().await.fail_all_pending_requests(RequestError::LinkClosed));
        }
        let mut handler = self.handler.lock().await;
        for segment_hash in receiving {
            handler.resource_manager.cancel_incoming(segment_hash);
        }
        let events = handler.resource_manager.drain_events();
        super::resource_wire::publish_resource_events(&handler, events);
    }
}
