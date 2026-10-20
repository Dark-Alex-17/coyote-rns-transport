//! Black-box usage of the `LinkEvent::PacketProven` / `DeliveryReceipt::link_id` surface
//! from outside the crate: the public paths resolve, `LinkEvent::PacketProven { packet_hash }`
//! is matchable the way a downstream consumer writes it (with the wildcard arm
//! `#[non_exhaustive]` forces; `#[deny(unreachable_patterns)]` on the match makes the arm a
//! compile error should the attribute go, so this file pins it), and `DeliveryReceipt` built
//! through its public constructor names no link. The loopback behaviour behind both lives in
//! the crate's own `tests_parts` fixtures.

use rns_transport::destination::link::LinkEvent;
use rns_transport::hash::Hash;
use rns_transport::transport::DeliveryReceipt;

/// The shape every downstream `match` on `LinkEvent` has to take now: a wildcard arm.
#[deny(unreachable_patterns)]
fn event_name(event: &LinkEvent) -> &'static str {
    match event {
        LinkEvent::Activated => "activated",
        LinkEvent::Data(_) => "data",
        LinkEvent::PeerIdentified(_) => "peer_identified",
        LinkEvent::PacketProven { .. } => "packet_proven",
        LinkEvent::Closed => "closed",
        _ => "unknown",
    }
}

#[test]
fn packet_proven_is_constructible_and_matchable_from_outside_the_crate() {
    let packet_hash = Hash::new([0x5a; 32]);
    let event = LinkEvent::PacketProven { packet_hash };
    assert_eq!(event_name(&event), "packet_proven");
    match event {
        LinkEvent::PacketProven { packet_hash: seen } => assert_eq!(seen, packet_hash),
        _ => panic!("the PacketProven arm must match its own value"),
    }
    assert_eq!(event_name(&LinkEvent::Closed), "closed");
}

#[test]
fn delivery_receipt_public_constructor_names_no_link() {
    let receipt = DeliveryReceipt::new([7u8; 32]);
    assert_eq!(receipt.link_id(), None);
    assert_eq!(receipt.packet_hash(), Hash::new([7u8; 32]));
    assert!(receipt.matches_packet_hash(&Hash::new([7u8; 32])));
}
