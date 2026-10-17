//! OSC over UDP: messages in, to drive the composition like a MIDI
//! controller, and values out, so a tablet shows where things are.
//!
//! Besides learned mappings, Prism answers fixed addresses (layers and
//! columns numbered from 1, layer 1 at the bottom):
//!
//! | Address | Value |
//! | --- | --- |
//! | `/prism/layer/N/opacity` | 0 to 1 |
//! | `/prism/layer/N/bypass`, `/solo`, `/stop` | press |
//! | `/prism/layer/N/clip/C` | press: launches the clip |
//! | `/prism/column/C` | press: launches the column |
//! | `/prism/crossfader` | 0 to 1 |
//! | `/prism/tap`, `/prism/resync`, `/prism/stop` | press |
//!
//! Prism sends `/prism/crossfader`, `/prism/layer/N/opacity`, `/bypass`,
//! `/solo` and `/prism/tempo` (in BPM) when they change.

use std::net::{SocketAddr, UdpSocket};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::thread::JoinHandle;
use std::time::Duration;

use prism_core::{Composition, ControlSource, ControlTarget};
use rosc::{OscMessage, OscPacket, OscType};

use crate::ControlMessage;

/// Port Prism listens on by default.
pub const DEFAULT_OSC_PORT: u16 = 7000;

/// Value of an OSC argument, from 0 to 1.
fn value_of(argument: Option<&OscType>) -> f32 {
    let value = match argument {
        None => 1.0,
        Some(OscType::Float(value)) => *value,
        Some(OscType::Double(value)) => *value as f32,
        Some(OscType::Int(value)) => *value as f32,
        Some(OscType::Long(value)) => *value as f32,
        Some(OscType::Bool(value)) => f32::from(u8::from(*value)),
        Some(_) => 1.0,
    };
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn collect(packet: OscPacket, messages: &mut Vec<ControlMessage>) {
    match packet {
        OscPacket::Message(message) => {
            let value = value_of(message.args.first());
            messages.push(ControlMessage {
                source: ControlSource::Osc {
                    address: message.addr,
                },
                value,
                pressed: value >= 0.5,
            });
        }
        OscPacket::Bundle(bundle) => {
            for packet in bundle.content {
                collect(packet, messages);
            }
        }
    }
}

/// Reads the messages of an OSC packet, bundles opened. A message without
/// argument counts as a press.
pub fn parse_osc(bytes: &[u8]) -> Vec<ControlMessage> {
    let mut messages = Vec::new();
    if let Ok((_, packet)) = rosc::decoder::decode_udp(bytes) {
        collect(packet, &mut messages);
    }
    messages
}

/// The target of a fixed address of Prism, in `composition`.
pub fn osc_target(composition: &Composition, address: &str) -> Option<ControlTarget> {
    let parts: Vec<&str> = address.trim_matches('/').split('/').collect();
    let number = |text: &str| {
        text.parse::<usize>()
            .ok()
            .filter(|n| *n >= 1)
            .map(|n| n - 1)
    };
    let layer = |text: &str| Some(composition.layers().get(number(text)?)?.id());
    let column = |text: &str| Some(composition.columns().get(number(text)?)?.id());
    match parts.as_slice() {
        ["prism", "crossfader"] => Some(ControlTarget::Crossfader),
        ["prism", "tap"] => Some(ControlTarget::Tap),
        ["prism", "resync"] => Some(ControlTarget::Resync),
        ["prism", "stop"] => Some(ControlTarget::StopAll),
        ["prism", "column", c] => Some(ControlTarget::Column { column: column(c)? }),
        ["prism", "layer", n, "opacity"] => Some(ControlTarget::LayerOpacity { layer: layer(n)? }),
        ["prism", "layer", n, "bypass"] => Some(ControlTarget::LayerBypass { layer: layer(n)? }),
        ["prism", "layer", n, "solo"] => Some(ControlTarget::LayerSolo { layer: layer(n)? }),
        ["prism", "layer", n, "stop"] => Some(ControlTarget::LayerStop { layer: layer(n)? }),
        ["prism", "layer", n, "clip", c] => Some(ControlTarget::Clip {
            slot: prism_core::Slot::new(layer(n)?, column(c)?),
        }),
        _ => None,
    }
}

/// The values Prism sends, by address.
pub fn osc_feedback(composition: &Composition) -> Vec<(String, f32)> {
    let mut values = vec![
        (
            "/prism/crossfader".to_owned(),
            composition.crossfader.position.get(),
        ),
        ("/prism/tempo".to_owned(), composition.tempo.get() as f32),
    ];
    for (index, layer) in composition.layers().iter().enumerate() {
        let prefix = format!("/prism/layer/{}", index + 1);
        values.push((format!("{prefix}/opacity"), layer.opacity.get()));
        values.push((
            format!("{prefix}/bypass"),
            f32::from(u8::from(layer.bypass)),
        ));
        values.push((format!("{prefix}/solo"), f32::from(u8::from(layer.solo))));
    }
    values
}

/// Listens for OSC on a UDP port until dropped.
pub struct OscInput {
    port: u16,
    running: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl OscInput {
    /// Listens on `port` of every network interface; messages go to
    /// `sender`, then `wake` is called.
    pub fn start(
        port: u16,
        sender: Sender<ControlMessage>,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) -> std::io::Result<Self> {
        let socket = UdpSocket::bind(("0.0.0.0", port))?;
        socket.set_read_timeout(Some(Duration::from_millis(200)))?;
        let running = Arc::new(AtomicBool::new(true));
        let still = Arc::clone(&running);
        let thread = std::thread::Builder::new()
            .name("prism-osc".into())
            .spawn(move || {
                let mut buffer = [0u8; rosc::decoder::MTU];
                while still.load(Ordering::Relaxed) {
                    let Ok(size) = socket.recv(&mut buffer) else {
                        continue;
                    };
                    let messages = parse_osc(&buffer[..size]);
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
            port,
            running,
            thread: Some(thread),
        })
    }

    /// Port listened on.
    pub fn port(&self) -> u16 {
        self.port
    }
}

impl Drop for OscInput {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Sends values to an OSC receiver, each only when it changed.
pub struct OscOutput {
    socket: UdpSocket,
    target: SocketAddr,
    sent: std::collections::HashMap<String, f32>,
}

impl OscOutput {
    /// Sends to `target`, such as `192.168.1.20:9000`.
    pub fn new(target: &str) -> std::io::Result<Self> {
        let target: SocketAddr = target.parse().map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "adresse attendue : IP:port",
            )
        })?;
        let socket = UdpSocket::bind(("0.0.0.0", 0))?;
        Ok(Self {
            socket,
            target,
            sent: std::collections::HashMap::new(),
        })
    }

    /// Sends the values that changed since the last call.
    pub fn send_changes(&mut self, values: &[(String, f32)]) -> std::io::Result<()> {
        for (address, value) in values {
            if self.sent.get(address) == Some(value) {
                continue;
            }
            let packet = OscPacket::Message(OscMessage {
                addr: address.clone(),
                args: vec![OscType::Float(*value)],
            });
            let bytes = rosc::encoder::encode(&packet)
                .map_err(|error| std::io::Error::other(error.to_string()))?;
            self.socket.send_to(&bytes, self.target)?;
            self.sent.insert(address.clone(), *value);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode(address: &str, args: Vec<OscType>) -> Vec<u8> {
        rosc::encoder::encode(&OscPacket::Message(OscMessage {
            addr: address.into(),
            args,
        }))
        .unwrap()
    }

    #[test]
    fn messages_and_values_are_read() {
        let messages = parse_osc(&encode("/fader/1", vec![OscType::Float(0.25)]));
        assert_eq!(messages.len(), 1);
        assert_eq!(
            messages[0].source,
            ControlSource::Osc {
                address: "/fader/1".into()
            }
        );
        assert!(messages[0].value == 0.25 && !messages[0].pressed);
        // No argument: a press. Out of range: clamped.
        assert!(parse_osc(&encode("/go", Vec::new()))[0].pressed);
        assert_eq!(
            parse_osc(&encode("/x", vec![OscType::Int(5)]))[0].value,
            1.0
        );
        assert!(parse_osc(b"not osc").is_empty());
    }

    #[test]
    fn fixed_addresses_name_layers_and_columns_from_one() {
        let comp = Composition::default();
        let first = comp.layers()[0].id();
        let column = comp.columns()[1].id();
        assert_eq!(
            osc_target(&comp, "/prism/layer/1/opacity"),
            Some(ControlTarget::LayerOpacity { layer: first })
        );
        assert_eq!(
            osc_target(&comp, "/prism/layer/1/clip/2"),
            Some(ControlTarget::Clip {
                slot: prism_core::Slot::new(first, column)
            })
        );
        assert_eq!(osc_target(&comp, "/prism/tap"), Some(ControlTarget::Tap));
        assert_eq!(osc_target(&comp, "/prism/layer/0/opacity"), None);
        assert_eq!(osc_target(&comp, "/prism/layer/99/solo"), None);
        assert_eq!(osc_target(&comp, "/other"), None);
        let feedback = osc_feedback(&comp);
        assert!(feedback.contains(&("/prism/layer/1/opacity".to_owned(), 1.0)));
    }

    #[test]
    fn input_and_output_talk_over_udp() {
        let (sender, receiver) = std::sync::mpsc::channel();
        let wake: Arc<dyn Fn() + Send + Sync> = Arc::new(|| {});
        // A port the system chooses is free; bind it, note it, release it.
        let port = UdpSocket::bind(("127.0.0.1", 0))
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let _input = OscInput::start(port, sender, wake).unwrap();
        let mut output = OscOutput::new(&format!("127.0.0.1:{port}")).unwrap();
        output
            .send_changes(&[("/prism/crossfader".into(), 0.75)])
            .unwrap();
        let message = receiver.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(message.value, 0.75);
        // Unchanged: not sent again.
        output
            .send_changes(&[("/prism/crossfader".into(), 0.75)])
            .unwrap();
        assert!(receiver.recv_timeout(Duration::from_millis(300)).is_err());
        assert!(OscOutput::new("pas une adresse").is_err());
    }
}
