// The per-path interface read, `include!`d into `links_parts/transport.rs`
// beside `path_table.rs`, which keeps it next to the table export it agrees
// with and `close_channel.rs` under the module size cap.

/// The interface one path was learned over, read from the local path table
/// by [`Transport::path_interface`]. Fork-only record: Python's
/// `Transport.next_hop_interface(destination_hash)` returns the interface
/// object itself (`RNS/Transport.py:3156-3164`), where this carries the
/// interface's address, the display name the interface manager holds for
/// it, the local learn time and the hop count, so a caller can check the
/// interface a path rests on against its own interface state.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathIface {
    /// The interface the announce arrived on, the `IDX_PT_RVCD_IF` slot of
    /// the table row (`RNS/Transport.py:4127`) that `next_hop_interface`
    /// returns (`RNS/Transport.py:3162`); the same address
    /// [`Transport::path_status`] reports as `interface`.
    pub iface: AddressHash,
    /// The interface manager's display name for `iface`, the source
    /// `TransportPathTableEntry::interface_name` and the JSON status read;
    /// `None` when the interface has no display name or is no longer
    /// registered with the manager (the entry outlives a detached interface
    /// until it is culled or expired). Fork-only: Python's interface object
    /// carries its own `name`.
    pub name: Option<String>,
    /// When this transport recorded the path, on the accepted announce or on
    /// a tunnel or disk restore, the in-process analogue of a rnsd
    /// `PathRow.timestamp` (`transport::shared_instance_rpc::PathRow`):
    /// Python stores `now` in the row's `IDX_PT_TIMESTAMP` slot
    /// (`RNS/Transport.py:4122`, set at `:2458-2459`). The table entry keeps
    /// an `Instant`, so this is `SystemTime::now()` less the entry's age and
    /// carries the imprecision of sampling the two clocks, typically well
    /// under a millisecond.
    pub learned_at: std::time::SystemTime,
    /// The announce's hop count, the `IDX_PT_HOPS` slot (`RNS/Transport.py:4124`);
    /// the same figure [`Transport::hops_to`] answers.
    pub hops: u8,
}

impl Transport {
    /// The interface the path to `destination` was learned over, with its
    /// display name, local learn time and hop count, or `None` when the path
    /// table has no entry, the case [`Transport::path_status`] reports as
    /// `path_found == false`. An expired or unresponsive entry is whatever the
    /// table still holds; nothing is filtered. [`Transport::path_table`]
    /// exports the same facts for every destination whose interface is still
    /// registered with the manager; a path whose interface has since been
    /// detached is omitted there and still returned here, with `name` as
    /// `None`.
    ///
    /// On a failed link, look the path's interface up here and check it
    /// against the interface state you hold: in-process the state stream is
    /// [`Transport::interface_events`], where a `Disconnected` followed by
    /// `Connecting` or `Connected` means the interface came back since the
    /// path was learned. When the interface is down, call
    /// [`Transport::expire_path`] and then [`Transport::request_path`].
    ///
    /// Fork-only accessor over Python's `Transport.next_hop_interface`
    /// (`RNS/Transport.py:3156-3164`), which returns the interface object
    /// and not a record. Only the path-table branch (`RNS/Transport.py:3162`)
    /// is mirrored: Python answers the shared-instance interface for a
    /// destination in `destinations_map` (`:3163`), where a destination of
    /// this transport's own has no path table entry and answers `None` here.
    pub async fn path_interface(&self, destination: &AddressHash) -> Option<PathIface> {
        let (iface, hops, timestamp) = {
            let handler = self.handler.lock().await;
            let entry = handler.path_table.get(destination)?;
            (entry.iface, entry.hops, entry.timestamp)
        };
        let name = self.iface_manager.lock().await.display_name(&iface).map(ToOwned::to_owned);
        let learned_at = std::time::SystemTime::now()
            .checked_sub(timestamp.elapsed())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        Some(PathIface { iface, name, learned_at, hops })
    }
}
