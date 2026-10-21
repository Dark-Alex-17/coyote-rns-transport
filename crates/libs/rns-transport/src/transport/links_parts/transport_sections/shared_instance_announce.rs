const SHARED_INSTANCE_REGISTRATION_ANNOUNCE_DELAY: Duration = Duration::from_millis(250);

impl Transport {
    /// Announces every registered single destination to the shared instance
    /// as a path response, each with its registered app data, the way a
    /// Python client re-announces when its shared instance reappears
    /// (`RNS/Transport.py:3705-3709`). Returns the number of announces
    /// handed to the transport; a destination whose packet fails to build
    /// is logged and skipped. The caller decides whether the transport is
    /// attached; this method announces regardless.
    #[deny(clippy::await_holding_invalid_type)]
    pub async fn announce_destinations_to_shared_instance(&self) -> usize {
        announce_all_destinations_as_path_response(&self.handler, &self.name).await
    }

    /// Python announces a single destination with `path_response=True` from
    /// a thread that sleeps 0.25 s after the registration when the node is a
    /// shared-instance client (`RNS/Transport.py:2911-2916`). The task ends
    /// with [`Self::stop`], which joins it; a destination deregistered before
    /// the delay elapses is still announced, as in Python.
    fn spawn_shared_instance_registration_announce(
        &self,
        destination: Arc<Mutex<SingleInputDestination>>,
        address_hash: AddressHash,
    ) {
        let handler = self.handler.clone();
        let cancel = self.cancel.clone();
        let name = self.name.clone();
        let announce = async move {
            tokio::select! {
                _ = cancel.cancelled() => {}
                _ = tokio::time::sleep(SHARED_INSTANCE_REGISTRATION_ANNOUNCE_DELAY) => {
                    if let Err(error) =
                        announce_destination_as_path_response(&handler, &destination).await
                    {
                        log::warn!(
                            "tp({}): registration announce to shared instance not sent: packet build failed dst={} error={:?}",
                            name,
                            address_hash.short(),
                            error
                        );
                    }
                }
            }
        };
        spawn_shared_instance_task(&self.shared_instance_tasks, announce);
    }
}

/// Announces `destination` with the path-response context and the app data
/// registered for it, the announce a shared-instance client sends for a
/// destination it has just registered (`RNS/Transport.py:2911-2916`). The
/// packet is built under the destination guard alone and handed to the
/// handler afterwards, as `Transport::try_send_announce` does. `Ok(())`
/// means the packet was built and handed to the transport, not that an
/// interface accepted it.
#[deny(clippy::await_holding_invalid_type)]
pub(crate) async fn announce_destination_as_path_response(
    handler: &Arc<Mutex<TransportHandler>>,
    destination: &Arc<Mutex<SingleInputDestination>>,
) -> Result<(), RnsError> {
    let address_hash = destination.lock().await.desc.address_hash;
    let app_data = handler.lock().await.single_in_destination_app_data.get(&address_hash).cloned();
    let packet = {
        let mut destination = destination.lock().await;
        log::trace!(
            "[tp] announce_tx dst={} app_data_len={} path_response=true",
            address_hash,
            app_data.as_ref().map(Vec::len).unwrap_or(0)
        );
        destination.announce_with_context(OsRng, app_data.as_deref(), true)?
    };
    send_packet(handler, packet).await;
    Ok(())
}

/// Announces every entry of `single_in_destinations` as a path response and
/// returns how many announces were handed to the transport; `name` is the
/// transport's, for the warning a packet that fails to build leaves. The
/// entries are copied out under one short handler lock, so a destination
/// registered while the loop runs is left to its own registration announce.
#[deny(clippy::await_holding_invalid_type)]
pub(crate) async fn announce_all_destinations_as_path_response(
    handler: &Arc<Mutex<TransportHandler>>,
    name: &str,
) -> usize {
    let destinations = {
        let handler = handler.lock().await;
        handler
            .single_in_destinations
            .iter()
            .map(|(address_hash, destination)| (*address_hash, destination.clone()))
            .collect::<Vec<_>>()
    };
    let mut announced = 0;
    for (address_hash, destination) in destinations {
        match announce_destination_as_path_response(handler, &destination).await {
            Ok(()) => announced += 1,
            Err(error) => log::warn!(
                "tp({}): path-response announce not sent: packet build failed dst={} error={:?}",
                name,
                address_hash.short(),
                error
            ),
        }
    }
    announced
}

/// The transmit seam that holds the handler guard across the dispatch, as
/// `Transport::send_packet` and `request_dispatch::send_on_iface` do.
async fn send_packet(handler: &Arc<Mutex<TransportHandler>>, packet: Packet) {
    handler.lock().await.send_packet(packet).await;
}
