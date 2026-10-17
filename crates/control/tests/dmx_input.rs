//! DMX from a console, over Art-Net on this computer: a moved channel
//! becomes a control message.

#![allow(clippy::unwrap_used)]

use std::net::UdpSocket;
use std::sync::Arc;
use std::sync::mpsc;
use std::time::Duration;

use prism_control::DmxInput;
use prism_core::{ControlSource, DmxProtocol};
use prism_dmx::{UNIVERSE_SIZE, artnet};

#[test]
fn a_moved_art_net_channel_becomes_a_message() {
    let (sender, messages) = mpsc::channel();
    let Ok(_input) = DmxInput::start(DmxProtocol::ArtNet, sender, Arc::new(|| {})) else {
        eprintln!("Art-Net port taken: skipped");
        return;
    };
    let console = UdpSocket::bind("127.0.0.1:0").unwrap();
    let mut channels = [0u8; UNIVERSE_SIZE];
    console
        .send_to(
            &artnet::dmx_packet(4, 1, &channels),
            ("127.0.0.1", artnet::PORT),
        )
        .unwrap();
    std::thread::sleep(Duration::from_millis(100));
    channels[6] = 200;
    console
        .send_to(
            &artnet::dmx_packet(4, 2, &channels),
            ("127.0.0.1", artnet::PORT),
        )
        .unwrap();
    let message = messages.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(
        message.source,
        ControlSource::Dmx {
            universe: 4,
            channel: 7
        }
    );
    assert!(message.pressed);
}
