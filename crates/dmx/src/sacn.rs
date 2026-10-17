//! sACN (ANSI E1.31, streaming ACN): DMX universes in UDP packets on port
//! 5568, usually sent to the multicast group of each universe,
//! 239.255.(universe high byte).(universe low byte).
//!
//! A data packet has three layers, each starting with flags (0x7) and its
//! length on 12 bits: the root layer (preamble, the ACN identifier, the
//! sender's CID), the framing layer (source name, priority, sequence,
//! universe), and the DMP layer (start code 0 then the 512 channels).

use std::net::Ipv4Addr;

use crate::{UNIVERSE_SIZE, Universe};

/// UDP port of sACN.
pub const PORT: u16 = 5568;

/// Default priority of a source.
pub const DEFAULT_PRIORITY: u8 = 100;

const IDENTIFIER: &[u8; 12] = b"ASC-E1.17\0\0\0";
const ROOT_VECTOR_DATA: u32 = 0x0000_0004;
const FRAMING_VECTOR_DATA: u32 = 0x0000_0002;
const DMP_VECTOR_SET: u8 = 0x02;
/// Length of a full data packet: 126 bytes of headers, 512 channels.
const PACKET_LENGTH: usize = 638;

fn flags_and_length(length: usize) -> [u8; 2] {
    (0x7000 | (length as u16 & 0x0FFF)).to_be_bytes()
}

/// The multicast group of `universe`.
pub fn multicast_group(universe: u16) -> Ipv4Addr {
    let [high, low] = universe.to_be_bytes();
    Ipv4Addr::new(239, 255, high, low)
}

/// A data packet carrying `channels` for `universe` (1 to 63 999), from the
/// source `cid` named `name`.
pub fn data_packet(
    cid: &[u8; 16],
    name: &str,
    universe: u16,
    sequence: u8,
    priority: u8,
    channels: &Universe,
) -> Vec<u8> {
    let mut packet = Vec::with_capacity(PACKET_LENGTH);
    // Root layer.
    packet.extend_from_slice(&0x0010u16.to_be_bytes());
    packet.extend_from_slice(&0x0000u16.to_be_bytes());
    packet.extend_from_slice(IDENTIFIER);
    packet.extend_from_slice(&flags_and_length(PACKET_LENGTH - 16));
    packet.extend_from_slice(&ROOT_VECTOR_DATA.to_be_bytes());
    packet.extend_from_slice(cid);
    // Framing layer.
    packet.extend_from_slice(&flags_and_length(PACKET_LENGTH - 38));
    packet.extend_from_slice(&FRAMING_VECTOR_DATA.to_be_bytes());
    let mut source = [0u8; 64];
    // A UTF-8 name, cut on a character boundary, null-terminated.
    let mut end = name.len().min(63);
    while !name.is_char_boundary(end) {
        end -= 1;
    }
    source[..end].copy_from_slice(&name.as_bytes()[..end]);
    packet.extend_from_slice(&source);
    packet.push(priority.min(200));
    packet.extend_from_slice(&0u16.to_be_bytes());
    packet.push(sequence);
    packet.push(0);
    packet.extend_from_slice(&universe.to_be_bytes());
    // DMP layer.
    packet.extend_from_slice(&flags_and_length(PACKET_LENGTH - 115));
    packet.push(DMP_VECTOR_SET);
    packet.push(0xA1);
    packet.extend_from_slice(&0x0000u16.to_be_bytes());
    packet.extend_from_slice(&0x0001u16.to_be_bytes());
    packet.extend_from_slice(&(UNIVERSE_SIZE as u16 + 1).to_be_bytes());
    packet.push(0);
    packet.extend_from_slice(channels);
    packet
}

/// The universe and channels of a data packet with start code 0; `None`
/// for any other packet.
pub fn parse_data(packet: &[u8]) -> Option<(u16, Universe)> {
    if packet.len() < 126 || &packet[4..16] != IDENTIFIER {
        return None;
    }
    let vector = |at: usize| {
        u32::from_be_bytes([packet[at], packet[at + 1], packet[at + 2], packet[at + 3]])
    };
    if vector(18) != ROOT_VECTOR_DATA || vector(40) != FRAMING_VECTOR_DATA {
        return None;
    }
    if packet[117] != DMP_VECTOR_SET || packet[125] != 0 {
        return None;
    }
    let universe = u16::from_be_bytes([packet[113], packet[114]]);
    let count = usize::from(u16::from_be_bytes([packet[123], packet[124]])).saturating_sub(1);
    let data = &packet[126..];
    let count = count.min(data.len()).min(UNIVERSE_SIZE);
    let mut channels = [0; UNIVERSE_SIZE];
    channels[..count].copy_from_slice(&data[..count]);
    Some((universe, channels))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packets_follow_the_layout_and_read_back() {
        let cid = [7u8; 16];
        let mut channels = [0; UNIVERSE_SIZE];
        channels[0] = 1;
        channels[511] = 2;
        let packet = data_packet(&cid, "Prism", 258, 3, DEFAULT_PRIORITY, &channels);
        assert_eq!(packet.len(), PACKET_LENGTH);
        assert_eq!(packet[0..2], [0x00, 0x10]);
        assert_eq!(&packet[4..16], b"ASC-E1.17\0\0\0");
        // Root layer: 0x7000 | 622.
        assert_eq!(packet[16..18], [0x72, 0x6E]);
        assert_eq!(packet[22..38], cid);
        // Framing layer: 0x7000 | 600, source name, priority, universe.
        assert_eq!(packet[38..40], [0x72, 0x58]);
        assert_eq!(&packet[44..49], b"Prism");
        assert_eq!(packet[108], DEFAULT_PRIORITY);
        assert_eq!(packet[111], 3);
        assert_eq!(packet[113..115], [0x01, 0x02]);
        // DMP layer: 0x7000 | 523, 513 properties, start code 0.
        assert_eq!(packet[115..117], [0x72, 0x0B]);
        assert_eq!(packet[123..125], [0x02, 0x01]);
        assert_eq!(packet[125], 0);
        assert_eq!(parse_data(&packet), Some((258, channels)));
        let mut other = packet.clone();
        other[125] = 0xDD;
        assert_eq!(parse_data(&other), None);
    }

    #[test]
    fn universes_have_their_multicast_group() {
        assert_eq!(multicast_group(1), Ipv4Addr::new(239, 255, 0, 1));
        assert_eq!(multicast_group(258), Ipv4Addr::new(239, 255, 1, 2));
    }
}
