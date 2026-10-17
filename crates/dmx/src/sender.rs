//! Sending universes over UDP.

use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};

use crate::{Universe, artnet, sacn};

/// How universes travel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Protocol {
    /// Art-Net, to one address or the broadcast address of the network.
    ArtNet,
    /// sACN, to the multicast group of each universe, or to one address.
    Sacn,
}

/// Sends universes; keeps one sequence number per universe.
pub struct DmxSender {
    socket: UdpSocket,
    protocol: Protocol,
    /// Where packets go; `None`: broadcast (Art-Net) or multicast (sACN).
    target: Option<SocketAddr>,
    sequences: std::collections::HashMap<u16, u8>,
    cid: [u8; 16],
    name: String,
}

impl DmxSender {
    /// A sender of `protocol` packets to `target`, or to everyone on the
    /// network with `None`.
    pub fn new(protocol: Protocol, target: Option<IpAddr>) -> io::Result<Self> {
        let port = match protocol {
            Protocol::ArtNet => artnet::PORT,
            Protocol::Sacn => sacn::PORT,
        };
        Self::to(
            protocol,
            target.map(|address| SocketAddr::new(address, port)),
        )
    }

    /// A sender to an address and port of its own (a receiver listening
    /// elsewhere than on the protocol's port).
    pub fn to(protocol: Protocol, target: Option<SocketAddr>) -> io::Result<Self> {
        let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))?;
        socket.set_broadcast(true)?;
        socket.set_nonblocking(true)?;
        // A source identifier from the process and the time: unique enough
        // for receivers to tell sources apart.
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |time| time.as_nanos())
            ^ u128::from(std::process::id());
        Ok(Self {
            socket,
            protocol,
            target,
            sequences: std::collections::HashMap::new(),
            cid: seed.to_le_bytes(),
            name: "Prism".into(),
        })
    }

    /// Where packets go now.
    fn destination(&self, universe: u16) -> SocketAddr {
        match (self.protocol, self.target) {
            (_, Some(address)) => address,
            (Protocol::ArtNet, None) => SocketAddr::new(Ipv4Addr::BROADCAST.into(), artnet::PORT),
            (Protocol::Sacn, None) => {
                SocketAddr::new(sacn::multicast_group(universe).into(), sacn::PORT)
            }
        }
    }

    /// Sends `channels` as `universe`. A full network buffer drops the
    /// packet rather than wait: the next frame sends again.
    pub fn send(&mut self, universe: u16, channels: &Universe) -> io::Result<()> {
        let sequence = self.sequences.entry(universe).or_insert(0);
        // sACN keeps 0 for receivers that ignore sequences; Art-Net too.
        *sequence = sequence.wrapping_add(1).max(1);
        let packet = match self.protocol {
            Protocol::ArtNet => artnet::dmx_packet(universe, *sequence, channels),
            Protocol::Sacn => sacn::data_packet(
                &self.cid,
                &self.name,
                universe,
                *sequence,
                sacn::DEFAULT_PRIORITY,
                channels,
            ),
        };
        match self.socket.send_to(&packet, self.destination(universe)) {
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => Ok(()),
            result => result.map(|_| ()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn universes_reach_a_receiver_on_this_computer() {
        let receiver = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        receiver
            .set_read_timeout(Some(std::time::Duration::from_secs(2)))
            .unwrap();
        let port = receiver.local_addr().unwrap().port();
        let mut channels = [0; crate::UNIVERSE_SIZE];
        channels[2] = 99;
        for protocol in [Protocol::ArtNet, Protocol::Sacn] {
            let mut sender = DmxSender::new(protocol, Some(Ipv4Addr::LOCALHOST.into())).unwrap();
            // The standard ports may be taken: send to the test's port.
            let packet_target = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), port);
            let packet = match protocol {
                Protocol::ArtNet => artnet::dmx_packet(5, 1, &channels),
                Protocol::Sacn => sacn::data_packet(&sender.cid, "Prism", 5, 1, 100, &channels),
            };
            sender.socket.send_to(&packet, packet_target).unwrap();
            sender.send(5, &channels).unwrap();
            let mut buffer = [0; 1024];
            let (length, _) = receiver.recv_from(&mut buffer).unwrap();
            let parsed = match protocol {
                Protocol::ArtNet => artnet::parse_dmx(&buffer[..length]),
                Protocol::Sacn => sacn::parse_data(&buffer[..length]),
            };
            assert_eq!(parsed, Some((5, channels)));
            assert_eq!(sender.sequences.get(&5), Some(&1));
        }
    }
}
