//! MIDI inputs of the computer, connected by name.

use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use prism_clock::MidiClock;

use midir::{MidiInput, MidiInputConnection};

use crate::{ControlMessage, parse_midi};

const CLIENT: &str = "Prism";

/// Names of the MIDI inputs of the computer.
pub fn midi_ports() -> Vec<String> {
    let Ok(input) = MidiInput::new(CLIENT) else {
        return Vec::new();
    };
    input
        .ports()
        .iter()
        .filter_map(|port| input.port_name(port).ok())
        .collect()
}

/// Connections to MIDI inputs; each message read goes to a channel, then
/// `wake` is called so the application looks at it.
pub struct MidiInputs {
    connections: Vec<(String, MidiInputConnection<()>)>,
    sender: Sender<ControlMessage>,
    wake: std::sync::Arc<dyn Fn() + Send + Sync>,
    clock: Arc<Mutex<ClockState>>,
}

/// A MIDI clock heard on the inputs.
#[derive(Default)]
struct ClockState {
    clock: MidiClock,
    last_pulse: Option<Instant>,
}

/// What the MIDI clock of the inputs says.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ClockReading {
    /// Tempo, while pulses come in.
    pub bpm: Option<f64>,
    /// Times a sender started (MIDI Start), counted.
    pub starts: u64,
}

/// MIDI clock pulse, start, continue and stop (system real time).
const CLOCK_PULSE: u8 = 0xF8;
const CLOCK_START: u8 = 0xFA;
const CLOCK_STOP: u8 = 0xFC;

/// Longest silence of a clock still taken as running.
const CLOCK_SILENCE: Duration = Duration::from_millis(500);

impl MidiInputs {
    /// No connection yet; messages will go to `sender`.
    pub fn new(
        sender: Sender<ControlMessage>,
        wake: std::sync::Arc<dyn Fn() + Send + Sync>,
    ) -> Self {
        Self {
            connections: Vec::new(),
            sender,
            wake,
            clock: Arc::default(),
        }
    }

    /// The tempo of the MIDI clock received on any input, and the starts
    /// counted.
    pub fn clock(&self) -> ClockReading {
        let Ok(state) = self.clock.lock() else {
            return ClockReading::default();
        };
        let running = state
            .last_pulse
            .is_some_and(|last| last.elapsed() <= CLOCK_SILENCE);
        ClockReading {
            bpm: state.clock.bpm().filter(|_| running),
            starts: state.clock.starts(),
        }
    }

    /// Names of the inputs connected.
    pub fn connected(&self) -> Vec<&str> {
        self.connections
            .iter()
            .map(|(name, _)| name.as_str())
            .collect()
    }

    /// Connects every input present whose name `wanted` accepts, and
    /// disconnects the others. Returns the errors met, by input.
    pub fn sync(&mut self, wanted: impl Fn(&str) -> bool) -> Vec<String> {
        let present = midi_ports();
        self.connections
            .retain(|(name, _)| present.contains(name) && wanted(name));
        let mut errors = Vec::new();
        for name in present {
            if !wanted(&name)
                || self
                    .connections
                    .iter()
                    .any(|(connected, _)| *connected == name)
            {
                continue;
            }
            match self.connect(&name) {
                Ok(connection) => self.connections.push((name, connection)),
                Err(error) => errors.push(format!("{name} : {error}")),
            }
        }
        errors
    }

    fn connect(&self, name: &str) -> Result<MidiInputConnection<()>, String> {
        let input = MidiInput::new(CLIENT).map_err(|error| error.to_string())?;
        let ports = input.ports();
        let port = ports
            .iter()
            .find(|port| {
                input
                    .port_name(port)
                    .is_ok_and(|port_name| port_name == name)
            })
            .ok_or_else(|| "introuvable".to_owned())?;
        let sender = self.sender.clone();
        let wake = std::sync::Arc::clone(&self.wake);
        let clock = Arc::clone(&self.clock);
        let epoch = Instant::now();
        input
            .connect(
                port,
                "prism-in",
                move |micros, bytes, ()| {
                    if let Some(&status) = bytes.first()
                        && matches!(status, CLOCK_PULSE | CLOCK_START | CLOCK_STOP)
                    {
                        if let Ok(mut state) = clock.lock() {
                            match status {
                                CLOCK_PULSE => {
                                    // Messages sent without a time (virtual
                                    // ports) are timed on arrival.
                                    let micros = if micros > 0 {
                                        micros
                                    } else {
                                        epoch.elapsed().as_micros() as u64
                                    };
                                    state.clock.pulse(micros);
                                    state.last_pulse = Some(Instant::now());
                                }
                                CLOCK_START => {
                                    state.clock.start();
                                    wake();
                                }
                                _ => state.last_pulse = None,
                            }
                        }
                        return;
                    }
                    if let Some(message) = parse_midi(bytes) {
                        let _ = sender.send(message);
                        wake();
                    }
                },
                (),
            )
            .map_err(|error| error.to_string())
    }
}
