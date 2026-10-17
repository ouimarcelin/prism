//! Art-Net: DMX universes in UDP packets on port 6454.
//!
//! An ArtDmx packet: the identifier `Art-Net\0`, the operation code 0x5000
//! (little endian), the protocol version 14 (big endian), a sequence
//! number, the physical input, the universe (15 bits: net, sub-net and
//! universe, low byte first) and the channel count (big endian, even, 2 to
//! 512), then the channels.

use crate::{UNIVERSE_SIZE, Universe};

/// UDP port of Art-Net.
pub const PORT: u16 = 6454;

const ID: &[u8; 8] = b"Art-Net\0";
const OP_DMX: u16 = 0x5000;
const VERSION: u16 = 14;
const HEADER: usize = 18;

/// Largest Art-Net universe number (15 bits).
pub const MAX_UNIVERSE: u16 = 0x7FFF;

/// An ArtDmx packet carrying `channels` (512 values) for `universe`.
pub fn dmx_packet(universe: u16, sequence: u8, channels: &Universe) -> Vec<u8> {
    let mut packet = Vec::with_capacity(HEADER + UNIVERSE_SIZE);
    packet.extend_from_slice(ID);
    packet.extend_from_slice(&OP_DMX.to_le_bytes());
    packet.extend_from_slice(&VERSION.to_be_bytes());
    packet.push(sequence);
    packet.push(0);
    let universe = universe & MAX_UNIVERSE;
    packet.extend_from_slice(&universe.to_le_bytes());
    packet.extend_from_slice(&(UNIVERSE_SIZE as u16).to_be_bytes());
    packet.extend_from_slice(channels);
    packet
}

/// The universe and channels of an ArtDmx packet; `None` for any other
/// packet. Channels past the packet's count are zero.
pub fn parse_dmx(packet: &[u8]) -> Option<(u16, Universe)> {
    if packet.len() < HEADER || &packet[..8] != ID {
        return None;
    }
    if u16::from_le_bytes([packet[8], packet[9]]) != OP_DMX {
        return None;
    }
    let universe = u16::from_le_bytes([packet[14], packet[15]]) & MAX_UNIVERSE;
    let length = usize::from(u16::from_be_bytes([packet[16], packet[17]]));
    let data = &packet[HEADER..];
    let length = length.min(data.len()).min(UNIVERSE_SIZE);
    let mut channels = [0; UNIVERSE_SIZE];
    channels[..length].copy_from_slice(&data[..length]);
    Some((universe, channels))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packets_follow_the_layout_and_read_back() {
        let mut channels = [0; UNIVERSE_SIZE];
        channels[0] = 255;
        channels[511] = 7;
        let packet = dmx_packet(0x0123, 9, &channels);
        assert_eq!(packet.len(), 530);
        assert_eq!(&packet[..8], b"Art-Net\0");
        assert_eq!(packet[8..10], [0x00, 0x50]);
        assert_eq!(packet[10..12], [0, 14]);
        assert_eq!(packet[12], 9);
        assert_eq!(packet[14..16], [0x23, 0x01]);
        assert_eq!(packet[16..18], [0x02, 0x00]);
        assert_eq!(parse_dmx(&packet), Some((0x0123, channels)));
        assert_eq!(parse_dmx(b"Art-Net\0"), None);
        let mut poll = packet.clone();
        poll[9] = 0x20;
        assert_eq!(parse_dmx(&poll), None);
    }

    #[test]
    fn short_packets_fill_the_rest_with_zeros() {
        let mut packet = dmx_packet(1, 0, &[200; UNIVERSE_SIZE]);
        packet.truncate(HEADER + 4);
        packet[16..18].copy_from_slice(&4u16.to_be_bytes());
        let (_, channels) = parse_dmx(&packet).unwrap_or((0, [1; UNIVERSE_SIZE]));
        assert_eq!(channels[..5], [200, 200, 200, 200, 0]);
    }
}
