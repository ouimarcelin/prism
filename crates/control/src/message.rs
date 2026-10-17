//! Messages of controllers, reduced to what mappings need.

use prism_core::ControlSource;

/// A message of a controller: where it comes from and its value.
#[derive(Clone, Debug, PartialEq)]
pub struct ControlMessage {
    /// The control that sent it.
    pub source: ControlSource,
    /// Value from 0 to 1: a fader's position, a pad's velocity.
    pub value: f32,
    /// Whether the control is down: a note on, or a value from half up.
    pub pressed: bool,
}

/// Reads a MIDI message. Only notes and control changes are kept.
pub fn parse_midi(bytes: &[u8]) -> Option<ControlMessage> {
    let [status, data1, data2, ..] = *bytes else {
        return None;
    };
    let channel = status & 0x0F;
    let (data1, data2) = (data1 & 0x7F, data2 & 0x7F);
    let value = f32::from(data2) / 127.0;
    match status & 0xF0 {
        0x90 => Some(ControlMessage {
            source: ControlSource::MidiNote {
                channel,
                note: data1,
            },
            value,
            pressed: data2 > 0,
        }),
        0x80 => Some(ControlMessage {
            source: ControlSource::MidiNote {
                channel,
                note: data1,
            },
            value: 0.0,
            pressed: false,
        }),
        0xB0 => Some(ControlMessage {
            source: ControlSource::MidiControl {
                channel,
                number: data1,
            },
            value,
            pressed: data2 >= 64,
        }),
        _ => None,
    }
}

/// How a source is written in the interface: « Note 36, canal 1 ».
pub fn describe_source(source: &ControlSource) -> String {
    match source {
        ControlSource::MidiNote { channel, note } => {
            format!("Note {note}, canal {}", channel + 1)
        }
        ControlSource::MidiControl { channel, number } => {
            format!("CC {number}, canal {}", channel + 1)
        }
        ControlSource::Osc { address } => format!("OSC {address}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_and_controls_are_read() {
        let on = parse_midi(&[0x91, 36, 127]).unwrap();
        assert_eq!(
            on.source,
            ControlSource::MidiNote {
                channel: 1,
                note: 36
            }
        );
        assert!(on.pressed && on.value == 1.0);
        // Note on at velocity 0 is a note off.
        assert!(!parse_midi(&[0x91, 36, 0]).unwrap().pressed);
        assert!(!parse_midi(&[0x81, 36, 64]).unwrap().pressed);
        let fader = parse_midi(&[0xB0, 7, 64]).unwrap();
        assert_eq!(
            fader.source,
            ControlSource::MidiControl {
                channel: 0,
                number: 7
            }
        );
        assert!((fader.value - 64.0 / 127.0).abs() < 1e-6 && fader.pressed);
        // Clock, pitch bend, short messages: ignored.
        assert_eq!(parse_midi(&[0xF8]), None);
        assert_eq!(parse_midi(&[0xE0, 0, 64]), None);
        assert_eq!(describe_source(&on.source), "Note 36, canal 2");
    }
}
