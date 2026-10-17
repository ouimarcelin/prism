//! Control of Prism from outside: MIDI controllers and OSC.
//!
//! [`MidiInputs`] connects to the MIDI inputs of the computer and turns
//! their notes and control changes into [`ControlMessage`]s. [`Controls`]
//! applies them to the composition through its mappings
//! ([`prism_core::Mapping`]); [`learn`] adds a mapping.

mod apply;
mod dmx;
mod feedback;
mod message;
mod midi;
mod osc;

pub use apply::{Controls, Outcome, learn};
pub use dmx::{DmxInput, dmx_changes};
pub use feedback::{MidiOutputs, midi_feedback};
pub use message::{ControlMessage, describe_source, parse_midi};
pub use midi::{ClockReading, MidiInputs, midi_ports};
pub use osc::{DEFAULT_OSC_PORT, OscInput, OscOutput, osc_feedback, osc_target, parse_osc};
