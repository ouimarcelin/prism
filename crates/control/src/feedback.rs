//! Values sent back to MIDI controllers: lit pads for the clips playing,
//! switched bypass and solo, faders and rings where the values are.

use std::collections::HashMap;

use midir::{MidiOutput, MidiOutputConnection};
use prism_core::{Composition, ControlSource, ControlTarget, Effect, EffectOwner};

/// Value, from 0 to 127, of what a mapping drives; `None` for actions
/// without a state (tap, resync, stops).
fn state(composition: &Composition, target: &ControlTarget) -> Option<u8> {
    let on = |value: bool| if value { 127 } else { 0 };
    let level = |value: f32| (value.clamp(0.0, 1.0) * 127.0).round() as u8;
    let effect = |owner: &EffectOwner, index: usize| -> Option<&Effect> {
        match owner {
            EffectOwner::Composition => composition.effects.get(index),
            EffectOwner::Layer { layer } => composition.layer(*layer)?.effects.get(index),
            EffectOwner::Group { group } => composition.group(*group)?.effects.get(index),
            EffectOwner::Clip { slot } => composition.clip(*slot)?.effects.get(index),
            EffectOwner::Generator { slot } => composition.clip(*slot)?.source.generator(),
        }
    };
    match target {
        ControlTarget::Clip { slot } => Some(on(composition
            .layer(slot.layer)
            .is_some_and(|layer| layer.active_column() == Some(slot.column)))),
        ControlTarget::Column { column } => Some(on(composition
            .layers()
            .iter()
            .any(|layer| layer.active_column() == Some(*column)))),
        ControlTarget::LayerOpacity { layer } => {
            Some(level(composition.layer(*layer)?.opacity.get()))
        }
        ControlTarget::LayerBypass { layer } => Some(on(composition.layer(*layer)?.bypass)),
        ControlTarget::LayerSolo { layer } => Some(on(composition.layer(*layer)?.solo)),
        ControlTarget::Crossfader => Some(level(composition.crossfader.position.get())),
        ControlTarget::Dashboard { slot, slider } => Some(level(
            composition.clip(*slot)?.dashboard.get(*slider)?.get(),
        )),
        ControlTarget::EffectParam {
            owner,
            index,
            param,
        } => {
            let effect = effect(owner, *index)?;
            let info = prism_fx::info_for(effect)?;
            let range = info.params.iter().find(|info| info.id == param)?;
            let value = range.clamp(effect.param(param).unwrap_or(range.default));
            let span = range.max - range.min;
            Some(level(if span > 0.0 {
                (value - range.min) / span
            } else {
                0.0
            }))
        }
        ControlTarget::EffectBypass { owner, index } => Some(on(!effect(owner, *index)?.bypass)),
        ControlTarget::LayerStop { .. }
        | ControlTarget::Tap
        | ControlTarget::Resync
        | ControlTarget::StopAll => None,
    }
}

/// The MIDI messages that show the state of every mapped control.
pub fn midi_feedback(composition: &Composition) -> Vec<[u8; 3]> {
    composition
        .mappings
        .iter()
        .filter_map(|mapping| {
            let value = state(composition, &mapping.target)?;
            match mapping.source {
                ControlSource::MidiNote { channel, note } => {
                    Some([0x90 | (channel & 0x0F), note & 0x7F, value])
                }
                ControlSource::MidiControl { channel, number } => {
                    Some([0xB0 | (channel & 0x0F), number & 0x7F, value])
                }
                ControlSource::Osc { .. }
                | ControlSource::Key { .. }
                | ControlSource::Dmx { .. } => None,
            }
        })
        .collect()
}

/// Connections to MIDI outputs; each message goes to all of them, once
/// per change.
#[derive(Default)]
pub struct MidiOutputs {
    connections: Vec<(String, MidiOutputConnection)>,
    /// Last value sent, by status and number.
    sent: HashMap<(u8, u8), u8>,
}

impl MidiOutputs {
    /// Connects the outputs named in `names` (the inputs listened to: a
    /// controller usually has both, with the same name) and disconnects the
    /// others.
    pub fn sync(&mut self, names: &[String]) {
        self.connections.retain(|(name, _)| names.contains(name));
        let missing: Vec<&String> = names
            .iter()
            .filter(|name| {
                !self
                    .connections
                    .iter()
                    .any(|(connected, _)| connected == *name)
            })
            .collect();
        if missing.is_empty() {
            return;
        }
        let Ok(output) = MidiOutput::new("Prism") else {
            return;
        };
        let ports = output.ports();
        for name in missing {
            let Some(port) = ports.iter().find(|port| {
                output
                    .port_name(port)
                    .is_ok_and(|port_name| port_name == *name)
            }) else {
                continue;
            };
            let Ok(output) = MidiOutput::new("Prism") else {
                return;
            };
            if let Ok(connection) = output.connect(port, "prism-out") {
                self.connections.push((name.clone(), connection));
                // A new controller learns every value.
                self.sent.clear();
            }
        }
    }

    /// Sends the messages whose value changed since they were last sent.
    pub fn send_changes(&mut self, messages: &[[u8; 3]]) {
        for message in messages {
            let key = (message[0], message[1]);
            if self.sent.get(&key) == Some(&message[2]) {
                continue;
            }
            for (_, connection) in &mut self.connections {
                let _ = connection.send(message);
            }
            self.sent.insert(key, message[2]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use prism_core::{Clip, Mapping, Slot, Unit};

    #[test]
    fn mapped_controls_show_their_state() {
        let mut comp = Composition::default();
        let layer = comp.layers()[0].id();
        let slot = Slot::new(layer, comp.columns()[0].id());
        comp.set_clip(slot, Some(Clip::from_file("/a.mov")))
            .unwrap();
        comp.layer_mut(layer).unwrap().opacity = Unit::new(0.5);
        comp.mappings = vec![
            Mapping {
                source: ControlSource::MidiNote {
                    channel: 9,
                    note: 36,
                },
                target: ControlTarget::Clip { slot },
            },
            Mapping {
                source: ControlSource::MidiControl {
                    channel: 0,
                    number: 7,
                },
                target: ControlTarget::LayerOpacity { layer },
            },
            Mapping {
                source: ControlSource::MidiNote {
                    channel: 0,
                    note: 1,
                },
                target: ControlTarget::Tap,
            },
            Mapping {
                source: ControlSource::Key { key: "A".into() },
                target: ControlTarget::LayerSolo { layer },
            },
        ];
        assert_eq!(midi_feedback(&comp), [[0x99, 36, 0], [0xB0, 7, 64]]);
        comp.trigger_clip(slot).unwrap();
        assert_eq!(midi_feedback(&comp)[0], [0x99, 36, 127]);
    }

    #[test]
    fn unchanged_values_are_not_sent_again() {
        let mut outputs = MidiOutputs::default();
        outputs.send_changes(&[[0x90, 36, 127]]);
        assert_eq!(outputs.sent.get(&(0x90, 36)), Some(&127));
        outputs.send_changes(&[[0x90, 36, 0]]);
        assert_eq!(outputs.sent.get(&(0x90, 36)), Some(&0));
    }
}
