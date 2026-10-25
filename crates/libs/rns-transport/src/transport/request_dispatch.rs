use super::*;
use crate::destination::request::{
    DispatchForm, InboundRequest, RequestAllow, RequestFrame, RequestHandler, RequestId,
    RequestResponse, ResponseFrame,
};
use crate::resource::OutboundOutcome;
use tokio::sync::mpsc;
use tokio::task::JoinSet;

/// The `request-dispatch` worker: `RNS.Link.handle_request`
/// (`RNS/Link.py:804-855`) on the responder side. Links queue decrypted
/// requests here; each one is matched to its destination's handler table and
/// run on its own task, as Python runs `handle_request` on its own thread
/// (`RNS/Link.py:1004-1005`), so one slow or panicking handler delays no
/// other request. Dropping the `JoinSet` on cancel aborts whatever is still
/// running, which is what keeps a handler that never returns from holding up
/// shutdown.
#[deny(clippy::await_holding_invalid_type)]
pub(super) async fn run(
    handler: Arc<Mutex<TransportHandler>>,
    cancel: CancellationToken,
    mut rx: mpsc::Receiver<DispatchItem>,
) {
    let mut tasks: JoinSet<()> = JoinSet::new();
    loop {
        tokio::select! {
            _ = cancel.cancelled() => break,
            item = rx.recv() => match item {
                Some(item) => dispatch_one(&handler, &mut tasks, item).await,
                None => break,
            },
            // `join_next` on an empty set resolves `None` at once, so without
            // the guard this arm would spin the loop.
            Some(finished) = tasks.join_next(), if !tasks.is_empty() => {
                if let Err(err) = finished {
                    if err.is_panic() {
                        log::warn!("tp: request handler panicked: {err}");
                    }
                }
            }
        }
    }
}

/// `RNS/Link.py:999-1001`: a packet-form request larger than the
/// destination's `max_request_size` is ignored before any handler runs. The
/// limit and the handler row are read under one destination guard so a
/// limit changed between the two reads cannot let the request through.
/// A resource-form request was already measured against the limit at its
/// advertisement (`RNS/Link.py:1036-1046`); the check here is its second gate.
#[deny(clippy::await_holding_invalid_type)]
async fn dispatch_one(
    handler: &Arc<Mutex<TransportHandler>>,
    tasks: &mut JoinSet<()>,
    item: DispatchItem,
) {
    // `RNS/Link.py:885-889`: the resource form's id is `truncated_hash` over
    // the assembled bytes, computed here rather than under the link lock.
    let request_id = match item.form {
        DispatchForm::Packet { request_id } => request_id,
        DispatchForm::Resource => RequestId::of_packed_request(&item.plain_text),
    };
    let frame = match RequestFrame::decode(&item.plain_text) {
        Ok(frame) => frame,
        Err(err) => {
            log::debug!("tp: request {} on link {} is malformed: {err}", request_id, item.link_id);
            return;
        }
    };
    let destination =
        handler.lock().await.single_in_destinations.get(&item.destination_hash).cloned();
    let Some(destination) = destination else {
        log::debug!(
            "tp: request {} on link {} names an unknown destination {}",
            request_id,
            item.link_id,
            item.destination_hash
        );
        return;
    };
    let path_hash = frame.path_hash();
    let (limit, registered) = {
        let guard = destination.lock().await;
        (guard.max_request_size(), guard.request_handler(&path_hash))
    };
    if let Some(limit) = limit.filter(|limit| item.plain_text.len() > *limit) {
        log::debug!(
            "tp: request {} on link {} ignored, {}B exceeds the destination limit {}B",
            request_id,
            item.link_id,
            item.plain_text.len(),
            limit
        );
        return;
    }
    let Some(registered) = registered else {
        log::debug!(
            "tp: request {} on link {} has no handler for path hash {}",
            request_id,
            item.link_id,
            path_hash
        );
        return;
    };
    if !is_allowed(&registered.allow, item.remote_identity.as_ref()) {
        let peer = item
            .remote_identity
            .as_ref()
            .map_or_else(|| "<unidentified>".to_owned(), |identity| identity.address_hash.short());
        log::debug!(
            "tp: request {} from {} not allowed for: {}",
            request_id,
            peer,
            registered.path
        );
        return;
    }
    let requested_at = frame.requested_at();
    let request = InboundRequest::new(
        registered.path.clone(),
        path_hash,
        frame.into_data(),
        request_id,
        item.link_id,
        item.remote_identity,
        requested_at,
    );
    tasks.spawn(handle_and_respond(
        handler.clone(),
        registered.handler.clone(),
        registered.auto_compress,
        item.link_id,
        request_id,
        request,
    ));
}

/// `RNS/Link.py:818-824`: a list is matched against the identified peer,
/// and a peer that never identified matches no list.
fn is_allowed(allow: &RequestAllow, remote_identity: Option<&Identity>) -> bool {
    match allow {
        RequestAllow::None => false,
        RequestAllow::All => true,
        RequestAllow::List(allowed) => {
            remote_identity.is_some_and(|identity| allowed.contains(&identity.address_hash))
        }
    }
}

#[deny(clippy::await_holding_invalid_type)]
async fn handle_and_respond(
    handler: Arc<Mutex<TransportHandler>>,
    request_handler: Arc<dyn RequestHandler>,
    auto_compress: bool,
    link_id: AddressHash,
    request_id: RequestId,
    request: InboundRequest,
) {
    let Some(response) = request_handler.handle(request).await else {
        return;
    };
    let body = match response {
        RequestResponse::Value(value) => value,
        RequestResponse::Bytes(bytes) => rmpv::Value::Binary(bytes),
        RequestResponse::Resource { data, metadata } => {
            let metadata = match metadata.map(pack_value).transpose() {
                Ok(metadata) => metadata,
                Err(err) => {
                    log::warn!(
                        "tp: response {} on link {} dropped, metadata does not pack: {err}",
                        request_id,
                        link_id
                    );
                    return;
                }
            };
            send_response_as_resource(&handler, link_id, request_id, data, metadata, auto_compress)
                .await;
            return;
        }
    };
    let encoded = ResponseFrame::new(request_id, body).encode();
    send_packet_response(&handler, link_id, request_id, encoded, auto_compress).await;
}

fn pack_value(value: rmpv::Value) -> Result<Vec<u8>, rmpv::encode::Error> {
    let mut packed = Vec::new();
    rmpv::encode::write_value(&mut packed, &value)?;
    Ok(packed)
}

/// `RNS/Link.py:849-852`: one `RESPONSE` packet when the envelope fits the
/// link MDU, a response resource carrying the same bytes when it does not.
///
/// The packet leaves on the link's ingress interface, which the transport
/// sets on every link it accepts. Routing through the path table instead
/// would mean holding the handler guard across the interface write, so a
/// link without an ingress interface drops the response.
#[deny(clippy::await_holding_invalid_type)]
async fn send_packet_response(
    handler: &Arc<Mutex<TransportHandler>>,
    link_id: AddressHash,
    request_id: RequestId,
    encoded: Vec<u8>,
    auto_compress: bool,
) {
    let Some(link) = find_any_link(handler, &link_id).await else {
        log::debug!("tp: response {} dropped, link {} is gone", request_id, link_id);
        return;
    };
    let (packet, iface) = {
        let link = link.lock().await;
        if !link.status().can_exchange_data() {
            log::debug!(
                "tp: response {} dropped, link {} closed before transmit",
                request_id,
                link_id
            );
            return;
        }
        let fits = encoded.len() <= link.link_mdu();
        let packet = fits.then(|| link.response_packet(&encoded)).transpose();
        (packet, link.ingress_iface())
    };
    let packet = match packet {
        Ok(packet) => packet,
        Err(err) => {
            log::warn!(
                "tp: response {} on link {} dropped, packet build failed: {err:?}",
                request_id,
                link_id
            );
            return;
        }
    };
    let Some(packet) = packet else {
        send_response_as_resource(handler, link_id, request_id, encoded, None, auto_compress).await;
        return;
    };
    let Some(iface) = iface else {
        log::debug!(
            "tp: response {} dropped, link {} has no ingress interface",
            request_id,
            link_id
        );
        return;
    };
    let trace = send_on_iface(handler, iface, packet).await;
    if trace.sent_ifaces == 0 {
        log::debug!("tp: response {} on link {} reached no interface", request_id, link_id);
    }
}

/// Both response forms that leave as a resource (`RNS/Link.py:846,852`).
/// `data` is the whole resource payload: the packed `[request_id, body]`
/// envelope of an oversize `Value`/`Bytes` response, or the raw bytes of a
/// `Resource` response. `metadata` is the packed metadata of the latter and
/// `None` for the former. Either way the advertisement carries
/// `q == request_id` with the response flag `p` set, which the requester
/// reads before any part arrives (`RNS/Link.py:1044-1046`).
#[deny(clippy::await_holding_invalid_type)]
async fn send_response_as_resource(
    handler: &Arc<Mutex<TransportHandler>>,
    link_id: AddressHash,
    request_id: RequestId,
    data: Vec<u8>,
    metadata: Option<Vec<u8>>,
    auto_compress: bool,
) {
    let sent = send_response_resource(
        handler,
        &link_id,
        request_id.to_bytes().to_vec(),
        data,
        metadata,
        auto_compress,
    )
    .await
    .map(|(hash, _)| hash);
    if let Err(err) = sent {
        log::debug!("tp: response resource {} on link {} not sent: {err:?}", request_id, link_id);
    }
}

/// Holds the handler guard across the dispatch like `Transport::send_packet`
/// and `Transport::send_broadcast`.
#[allow(clippy::await_holding_invalid_type)] // TASK-026: handler guard spans the job by design, see TransportHandler lock discipline
pub(super) async fn send_on_iface(
    handler: &Arc<Mutex<TransportHandler>>,
    iface: AddressHash,
    packet: Packet,
) -> TxDispatchTrace {
    handler.lock().await.send(TxMessage { tx_type: TxMessageType::Direct(iface), packet }).await
}

#[deny(clippy::await_holding_invalid_type)]
pub(super) async fn find_any_link(
    handler: &Arc<Mutex<TransportHandler>>,
    link_id: &AddressHash,
) -> Option<Arc<Mutex<Link>>> {
    let out_links = {
        let handler = handler.lock().await;
        if let Some(link) = handler.in_links.get(link_id) {
            return Some(link.clone());
        }
        handler.out_links.values().cloned().collect::<Vec<_>>()
    };
    for link in out_links {
        if *link.lock().await.id() == *link_id {
            return Some(link);
        }
    }
    None
}

#[deny(clippy::await_holding_invalid_type)]
pub(super) async fn resource_mtu_for_iface(
    handler: &Arc<Mutex<TransportHandler>>,
    iface: Option<AddressHash>,
) -> usize {
    let Some(iface) = iface else {
        return crate::resource::DEFAULT_RESOURCE_INTERFACE_MTU;
    };
    let iface_manager = handler.lock().await.iface_manager.clone();
    let mtu = iface_manager.lock().await.mtu(&iface);
    mtu.unwrap_or(crate::resource::DEFAULT_RESOURCE_INTERFACE_MTU)
}

/// Sends `data` as a response resource on `link_id` (`RNS/Link.py:846,852`):
/// `request_id` becomes the advertisement's `q` and `is_response` its `p`
/// flag (`RNS/Resource.py:1305-1318`). `metadata`, when present, sets the
/// `x` flag and travels at the head of the resource data behind a 3-byte
/// length (`RNS/Resource.py:261-269,338`).
///
/// `auto_compress` is the registration's flag; some handlers, such as
/// NomadNet media, return raw binary and turn it off.
///
/// The [`OutboundOutcome`] resolves with the transfer's verdict; there is
/// none when the advertisement never left (`Err(ConnectionError)`).
#[deny(clippy::await_holding_invalid_type)]
pub(super) async fn send_response_resource(
    handler: &Arc<Mutex<TransportHandler>>,
    link_id: &AddressHash,
    request_id: Vec<u8>,
    data: Vec<u8>,
    metadata: Option<Vec<u8>>,
    auto_compress: bool,
) -> Result<(Hash, OutboundOutcome), RnsError> {
    let Some(link) = find_any_link(handler, link_id).await else {
        log::debug!(
            "tp: response {} dropped, link {} is gone",
            super::diag::bytes_to_hex(&request_id),
            link_id
        );
        return Err(RnsError::InvalidArgument);
    };
    let iface = {
        let link_guard = link.lock().await;
        if !link_guard.status().can_exchange_data() {
            log::debug!(
                "tp: response {} dropped, link {} closed before transmit",
                super::diag::bytes_to_hex(&request_id),
                link_id
            );
            return Err(RnsError::ConnectionError);
        }
        link_guard.ingress_iface()
    };
    let interface_mtu = resource_mtu_for_iface(handler, iface).await;
    // See `send_resource_observed`: prepared off the handler lock.
    let prepared = {
        let link_guard = link.lock().await;
        // The MTU lookup above released the link; a close that landed in
        // between has zeroed the derived key, and encrypting with it would
        // put an undecryptable advertisement on the wire.
        if !link_guard.status().can_exchange_data() {
            log::debug!(
                "tp: response {} dropped, link {} closed before transmit (after MTU lookup)",
                super::diag::bytes_to_hex(&request_id),
                link_id
            );
            return Err(RnsError::ConnectionError);
        }
        // See `resource_wire.rs`: the negotiated link MTU, not the local
        // interface alone, is what a fragment has to fit through.
        let interface_mtu = interface_mtu.min(link_guard.link_mtu());
        ResourceManager::prepare_send_with_compression(
            &link_guard,
            data,
            metadata,
            Some(request_id),
            true,
            interface_mtu,
            auto_compress,
        )?
    };
    let (resource_hash, packet, outcome) =
        handler.lock().await.resource_manager.track_prepared_with_outcome(prepared);
    let sent = match iface {
        Some(iface) => send_on_iface(handler, iface, packet).await.sent_ifaces > 0,
        None => false,
    };
    let mut guard = handler.lock().await;
    guard.resource_manager.confirm_outbound_dispatch(resource_hash, sent);
    let events = guard.resource_manager.drain_events();
    super::resource_wire::publish_resource_events(&guard, events);
    if sent {
        Ok((resource_hash, outcome))
    } else {
        Err(RnsError::ConnectionError)
    }
}
