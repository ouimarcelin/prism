//! Mappings: which message of a controller drives which part of the
//! composition. Saved with the composition: a show keeps its controls.

use serde::{Deserialize, Serialize};

use crate::{ColumnId, LayerId, Slot};

/// A message of a controller a mapping listens to.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ControlSource {
    /// A MIDI note (pad, key, button), on a channel from 0 to 15.
    MidiNote {
        /// Channel, 0 to 15 (shown 1 to 16).
        channel: u8,
        /// Note number, 0 to 127.
        note: u8,
    },
    /// A MIDI control change (knob, fader, button), on a channel.
    MidiControl {
        /// Channel, 0 to 15 (shown 1 to 16).
        channel: u8,
        /// Controller number, 0 to 127.
        number: u8,
    },
    /// An OSC address, such as `/fader/1`.
    Osc {
        /// The address of the messages.
        address: String,
    },
    /// A key of the keyboard, without modifier, by its English name (`A`,
    /// `1`, `Space`, `Up`…).
    Key {
        /// Name of the key.
        key: String,
    },
    /// A channel of a DMX universe received from a lighting console.
    Dmx {
        /// Universe, as the protocol numbers it.
        universe: u16,
        /// Channel, 1 to 512.
        channel: u16,
    },
}

/// Whose effect a mapping drives.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EffectOwner {
    /// The composition's effects.
    Composition,
    /// A layer's effects.
    Layer {
        /// The layer.
        layer: LayerId,
    },
    /// A clip's effects; index 0 of a generated clip is not its generator.
    Clip {
        /// Where the clip is.
        slot: Slot,
    },
    /// The generator of a generated clip (the index is ignored).
    Generator {
        /// Where the clip is.
        slot: Slot,
    },
}

/// What a mapping drives. Continuous targets follow the value of the
/// message across their range; the others act when a button is pressed.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ControlTarget {
    /// Launches a clip.
    Clip {
        /// Where the clip is.
        slot: Slot,
    },
    /// Launches a column.
    Column {
        /// The column.
        column: ColumnId,
    },
    /// Opacity of a layer.
    LayerOpacity {
        /// The layer.
        layer: LayerId,
    },
    /// Bypass of a layer, switched on each press.
    LayerBypass {
        /// The layer.
        layer: LayerId,
    },
    /// Solo of a layer, switched on each press.
    LayerSolo {
        /// The layer.
        layer: LayerId,
    },
    /// Stops the clip of a layer.
    LayerStop {
        /// The layer.
        layer: LayerId,
    },
    /// Position of the crossfader.
    Crossfader,
    /// A slider of a clip's dashboard.
    Dashboard {
        /// Where the clip is.
        slot: Slot,
        /// Slider, from 0.
        slider: usize,
    },
    /// A setting of an effect, across its range.
    EffectParam {
        /// Whose chain.
        owner: EffectOwner,
        /// Place of the effect in the chain.
        index: usize,
        /// Name of the setting.
        param: String,
    },
    /// Bypass of an effect, switched on each press.
    EffectBypass {
        /// Whose chain.
        owner: EffectOwner,
        /// Place of the effect in the chain.
        index: usize,
    },
    /// A tap of the tap tempo.
    Tap,
    /// Starts a bar.
    Resync,
    /// Stops every clip.
    StopAll,
}

impl ControlTarget {
    /// Whether the target follows a value rather than acting on a press.
    pub fn is_continuous(&self) -> bool {
        matches!(
            self,
            Self::LayerOpacity { .. }
                | Self::Crossfader
                | Self::Dashboard { .. }
                | Self::EffectParam { .. }
        )
    }
}

/// One mapping.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Mapping {
    /// The message listened to.
    pub source: ControlSource,
    /// What it drives.
    pub target: ControlTarget,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mappings_are_saved_by_kind() {
        let mapping = Mapping {
            source: ControlSource::MidiControl {
                channel: 0,
                number: 21,
            },
            target: ControlTarget::EffectParam {
                owner: EffectOwner::Composition,
                index: 1,
                param: "radius".into(),
            },
        };
        let json = serde_json::to_string(&mapping).unwrap();
        assert!(json.contains(r#""kind":"midi_control""#), "{json}");
        assert!(json.contains(r#""kind":"effect_param""#), "{json}");
        assert_eq!(serde_json::from_str::<Mapping>(&json).unwrap(), mapping);
        assert!(mapping.target.is_continuous());
        assert!(!ControlTarget::Tap.is_continuous());
    }
}
