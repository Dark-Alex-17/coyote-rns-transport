//! Counts the link requests to one destination among decoded packets; shared by the
//! integration tests that pin how many times a pending link's request reaches the wire.

use rns_transport::hash::AddressHash;
use rns_transport::packet::{Packet, PacketType};

/// How many `LinkRequest` packets in `packets` are addressed to `destination`.
pub fn link_requests_to(packets: &[Packet], destination: AddressHash) -> usize {
    packets
        .iter()
        .filter(|p| p.header.packet_type == PacketType::LinkRequest && p.destination == destination)
        .count()
}
