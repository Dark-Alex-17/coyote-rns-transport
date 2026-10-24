async fn attach_shared_instance_client(
    transport: &Transport,
    endpoint: SharedInstanceEndpoint,
    iface: &InterfaceConfig,
    label: &str,
    record: &mut InterfaceRecord,
    startup_failures: &mut Vec<InterfaceStartupFailure>,
) -> Option<AddressHash> {
    let (posture, target) = match &endpoint {
        SharedInstanceEndpoint::Tcp(addr) => ("local", format!("endpoint={addr}")),
        #[cfg(unix)]
        SharedInstanceEndpoint::Unix(endpoint) => {
            ("local unix", format!("socket_path={}", endpoint.label()))
        }
        other => ("local", format!("endpoint={other:?}")),
    };
    let mut options = SharedInstanceOptions::default()
        .with_mtu(iface.mtu.unwrap_or(TcpClient::DEFAULT_MTU))
        .with_interface_mode(iface.interface_mode().unwrap_or(InterfaceMode::Full));
    if let Some(bitrate_bps) = iface.force_shared_instance_bitrate {
        options = options.with_forced_bitrate(bitrate_bps);
    }

    let client_iface = match transport.attach_shared_instance(endpoint, options).await {
        Ok(client_iface) => client_iface,
        Err(RnsError::InvalidArgument) => {
            record_startup_failure(
                record,
                startup_failures,
                label.to_string(),
                iface.kind.clone(),
                "a shared instance is already attached to this daemon (one per process); entry ignored"
                    .to_string(),
            );
            return None;
        }
        Err(err) => {
            record_startup_failure(
                record,
                startup_failures,
                label.to_string(),
                iface.kind.clone(),
                format!("shared instance attach failed {target} err={err}"),
            );
            return None;
        }
    };
    {
        let iface_manager = transport.iface_manager();
        let mut manager = iface_manager.lock().await;
        apply_interface_runtime_config(&mut manager, client_iface, iface);
    }
    log::info!("[daemon] {} attached iface={} name={} {}", posture, client_iface, label, target);
    let runtime_iface = client_iface.to_string();
    mark_interface_startup_status(record, "attached", None, Some(runtime_iface.as_str()));
    Some(client_iface)
}

/// A host name resolves to every address of both families; the pick prefers the IPv4
/// loopback, as Python's `LocalClientInterface` connects to `127.0.0.1` only
/// (`RNS/Interfaces/LocalInterface.py:109`), so `localhost` reaches a shared instance
/// listening on `127.0.0.1` even where the resolver lists `::1` first.
/// `TcpStream::connect(&str)` used to try every address in turn; the attach takes one
/// `SocketAddr`, so the choice is made here.
async fn resolve_shared_instance_tcp_endpoint(endpoint: &str) -> Result<SocketAddr, String> {
    if let Ok(addr) = endpoint.parse::<SocketAddr>() {
        return Ok(addr);
    }
    let addrs: Vec<SocketAddr> = tokio::net::lookup_host(endpoint)
        .await
        .map_err(|err| format!("local attach endpoint={endpoint} did not resolve err={err}"))?
        .collect();
    pick_shared_instance_tcp_addr(&addrs)
        .ok_or_else(|| format!("local attach endpoint={endpoint} resolved to no address"))
}

fn pick_shared_instance_tcp_addr(addrs: &[SocketAddr]) -> Option<SocketAddr> {
    addrs
        .iter()
        .find(|addr| addr.is_ipv4() && addr.ip().is_loopback())
        .or_else(|| addrs.iter().find(|addr| addr.is_ipv4()))
        .or_else(|| addrs.first())
        .copied()
}
