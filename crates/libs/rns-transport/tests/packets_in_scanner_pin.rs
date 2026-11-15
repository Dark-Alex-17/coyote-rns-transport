//! Pins the byte-level semantics of the shared `packets_in` scanner that the pending-link
//! probes count link requests with: a doubled-flag stream and a shared-flag stream both
//! yield every packet, and a trailing partial frame yields only the complete ones.
//!
//! Production readers (`src/iface/tcp_client.rs` drains through the closing flag, like
//! the read loop of `RNS/Interfaces/TCPInterface.py`) would drop the second packet of a
//! shared-flag stream; the harness counts it on purpose because the probes pin what the
//! client SENT, and a request a peer would drop still left the client.

use rns_transport::iface::hdlc::Hdlc;
use rns_transport::packet::{Packet, PacketDataBuffer};

#[path = "support/packets_in.rs"]
mod packets_in;
use packets_in::packets_in;

const FLAG: u8 = 0x7e;

fn packet(data: &[u8]) -> Packet {
    Packet { data: PacketDataBuffer::new_from_slice(data), ..Default::default() }
}

/// `packet` as a complete HDLC frame, opening and closing flags included.
fn frame(packet: &Packet) -> Vec<u8> {
    Hdlc::frame(&packet.to_bytes().expect("the packet serialises")).expect("the frame encodes")
}

#[test]
fn a_doubled_flag_stream_yields_both_packets() {
    let (a, b) = (packet(b"first"), packet(b"second"));
    let stream = [frame(&a), frame(&b)].concat();
    assert_eq!(stream.iter().filter(|&&byte| byte == FLAG).count(), 4);

    assert_eq!(packets_in(&stream), vec![a, b]);
}

#[test]
fn a_shared_flag_stream_yields_both_packets() {
    let (a, b) = (packet(b"first"), packet(b"second"));
    let mut stream = frame(&a);
    stream.extend_from_slice(&frame(&b)[1..]);
    assert_eq!(stream.iter().filter(|&&byte| byte == FLAG).count(), 3);

    assert_eq!(packets_in(&stream), vec![a, b]);
}

#[test]
fn a_trailing_partial_frame_yields_only_the_complete_packets() {
    let (a, b) = (packet(b"first"), packet(b"second"));
    let partial = frame(&b);
    let mut stream = frame(&a);
    stream.extend_from_slice(&partial[..partial.len() - 1]);

    assert_eq!(packets_in(&stream), vec![a]);
}
