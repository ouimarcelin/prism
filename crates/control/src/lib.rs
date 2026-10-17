//! Control of Prism from outside: MIDI controllers today.
//!
//! [`MidiInputs`] connects to the MIDI inputs of the computer and turns
//! their notes and control changes into [`ControlMessage`]s. [`Controls`]
//! applies them to the composition through its mappings
//! ([`prism_core::Mapping`]); [`learn`] adds a mapping.

mod apply;
mod message;
mod midi;

pub use apply::{Controls, Outcome, learn};
pub use message::{ControlMessage, describe_source, parse_midi};
pub use midi::{MidiInputs, midi_ports};
