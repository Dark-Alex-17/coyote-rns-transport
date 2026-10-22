impl Transport {
    pub async fn path_table(
        &self,
        max_hops: Option<u64>,
    ) -> std::io::Result<Vec<crate::transport::TransportPathTableEntry>> {
        let now = std::time::Instant::now();
        let now_unix_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|err| std::io::Error::other(format!("system clock precedes Unix epoch: {err}")))?
            .as_secs_f64();
        let ifaces = self.exported_ifaces().await;
        let handler = self.handler.lock().await;
        let entries = handler.path_table.export_python_entries(now, now_unix_secs, |iface| {
            ifaces.get(iface).map(|info| (info.mode, info.full_hash))
        });
        Ok(entries
            .into_iter()
            .filter(|entry| max_hops.is_none_or(|limit| u64::from(entry.hops) <= limit))
            .map(|entry| crate::transport::TransportPathTableEntry {
                destination: entry.destination,
                timestamp_secs: entry.timestamp_secs,
                next_hop: entry.received_from,
                hops: entry.hops,
                expires_secs: entry.expires_secs,
                interface_name: ifaces.get(&entry.iface).and_then(|info| info.display_name.clone()),
                interface: entry.iface,
                interface_hash: entry.interface_hash,
            })
            .collect())
    }
}
