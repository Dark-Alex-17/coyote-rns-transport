//! Decodes the packets in the raw bytes of an HDLC stream; shared by the integration
//! tests that count what a transport's client put on a raw listener's stream.

use rns_transport::buffer::OutputBuffer;
use rns_transport::iface::hdlc::Hdlc;
use rns_transport::packet::Packet;

/// The packets in every complete HDLC frame of `raw`, in order. `Hdlc::find` returns the
/// indices of the next two 0x7E flags, so a closing flag can double as the next frame's
/// opening flag; rescanning from the closing flag handles both doubled-flag and
/// shared-flag streams, and the empty frame between a closing and an opening flag
/// decodes to zero bytes and is skipped.
pub fn packets_in(raw: &[u8]) -> Vec<Packet> {
    let mut packets = Vec::new();
    let mut cursor = 0;
    while let Some((start, end)) = Hdlc::find(&raw[cursor..]) {
        let frame = &raw[cursor + start..=cursor + end];
        let mut decoded = [0u8; 1024];
        let mut output = OutputBuffer::new(&mut decoded);
        let len = Hdlc::decode(frame, &mut output)
            .unwrap_or_else(|err| panic!("a frame on the stream does not decode: {err:?}"));
        if len > 0 {
            packets
                .push(Packet::from_bytes(&decoded[..len]).unwrap_or_else(|err| {
                    panic!("a frame on the stream is not a packet: {err:?}")
                }));
        }
        cursor += end;
    }
    packets
}
