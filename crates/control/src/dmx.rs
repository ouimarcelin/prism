//! DMX received from a lighting console, over Art-Net or sACN: each channel
//! that moves becomes a [`ControlMessage`], mapped like a MIDI fader.

use std::collections::HashMap;
use std::net::{Ipv4Addr, UdpSocket};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::thread::JoinHandle;
use std::time::Duration;

use prism_core::{ControlSource, DmxProtocol};
use prism_dmx::{UNIVERSE_SIZE, Universe, artnet, sacn};

use crate::ControlMessage;

/// sACN universes listened to (their multicast groups joined): the first
/// ones, where consoles start.
const SACN_UNIVERSES: std::ops::RangeInclusive<u16> = 1..=16;

/// The messages for the channels of `channels` that differ from
/// `previous`; none for the first packet of a universe, which only sets
/// where its channels stand.
pub fn dmx_changes(
    universe: u16,
    previous: Option<&Universe>,
    channels: &Universe,
) -> Vec<ControlMessage> {
    let Some(previous) = previous else {
        return Vec::new();
    };
    (0..UNIVERSE_SIZE)
        .filter(|&index| previous[index] != channels[index])
        .map(|index| ControlMessage {
            source: ControlSource::Dmx {
                universe,
                channel: index as u16 + 1,
            },
            value: f32::from(channels[index]) / 255.0,
            pressed: channels[index] >= 128,
        })
        .collect()
}

/// Listens for DMX on its thread.
pub struct DmxInput {
    protocol: DmxProtocol,
    running: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl DmxInput {
    /// Starts listening for `protocol` on its port; messages go to
    /// `sender`, then `wake` is called.
    pub fn start(
        protocol: DmxProtocol,
        sender: Sender<ControlMessage>,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) -> std::io::Result<Self> {
        let port = match protocol {
            DmxProtocol::ArtNet => artnet::PORT,
            DmxProtocol::Sacn => sacn::PORT,
        };
        let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, port))?;
        if protocol == DmxProtocol::Sacn {
            for universe in SACN_UNIVERSES {
                socket
                    .join_multicast_v4(&sacn::multicast_group(universe), &Ipv4Addr::UNSPECIFIED)?;
            }
        }
        socket.set_read_timeout(Some(Duration::from_millis(200)))?;
        let running = Arc::new(AtomicBool::new(true));
        let still = Arc::clone(&running);
        let thread = std::thread::Builder::new()
            .name("prism-dmx-in".into())
            .spawn(move || {
                let mut buffer = [0u8; 1024];
                let mut universes: HashMap<u16, Universe> = HashMap::new();
                while still.load(Ordering::Relaxed) {
                    let Ok(size) = socket.recv(&mut buffer) else {
                        continue;
                    };
                    let packet = &buffer[..size];
                    let parsed = match protocol {
                        DmxProtocol::ArtNet => artnet::parse_dmx(packet),
                        DmxProtocol::Sacn => sacn::parse_data(packet),
                    };
                    let Some((universe, channels)) = parsed else {
                        continue;
                    };
                    let messages = dmx_changes(universe, universes.get(&universe), &channels);
                    universes.insert(universe, channels);
                    if messages.is_empty() {
                        continue;
                    }
                    for message in messages {
                        let _ = sender.send(message);
                    }
                    wake();
                }
            })?;
        Ok(Self {
            protocol,
            running,
            thread: Some(thread),
        })
    }

    /// Protocol listened to.
    pub fn protocol(&self) -> DmxProtocol {
        self.protocol
    }
}

impl Drop for DmxInput {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moved_channels_become_messages() {
        let mut previous = [0u8; UNIVERSE_SIZE];
        let mut channels = previous;
        channels[0] = 255;
        channels[9] = 64;
        assert!(dmx_changes(2, None, &channels).is_empty());
        let messages = dmx_changes(2, Some(&previous), &channels);
        assert_eq!(messages.len(), 2);
        assert_eq!(
            messages[0].source,
            ControlSource::Dmx {
                universe: 2,
                channel: 1
            }
        );
        assert!(messages[0].pressed && messages[0].value == 1.0);
        assert_eq!(
            messages[1].source,
            ControlSource::Dmx {
                universe: 2,
                channel: 10
            }
        );
        assert!(!messages[1].pressed);
        previous = channels;
        assert!(dmx_changes(2, Some(&previous), &channels).is_empty());
    }
}
