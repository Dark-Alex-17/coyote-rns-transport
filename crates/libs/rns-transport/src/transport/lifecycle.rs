use super::*;

const STOP_BUDGET: Duration = Duration::from_secs(5);

impl Transport {
    /// Shuts the transport down: detaches every interface and waits for their
    /// workers, then cancels the transport and waits for the management task.
    /// Interface workers go first because they may be blocked handing a packet
    /// to the management task, which has to stay alive to receive it. The whole
    /// call shares a 5 s budget; whatever is still running when it runs out is
    /// aborted and logged. Calling it again is a no-op.
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
    }
}
