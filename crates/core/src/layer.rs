//! Layers: rows of the clip grid, composited bottom to top.

use serde::{Deserialize, Serialize};

use crate::{Clip, ColumnId, CrossfaderSide, LayerId, Unit};

/// How a layer is combined with the result of the layers below it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlendMode {
    /// Drawn over the layers below, weighted by opacity and alpha.
    #[default]
    Normal,
    /// Colors added together: bright areas build up.
    Add,
    /// Colors multiplied: the result is darker.
    Multiply,
    /// Inverted colors multiplied, then inverted: the result is lighter.
    Screen,
    /// The lighter of the two colors, channel by channel.
    Lighten,
    /// The darker of the two colors, channel by channel.
    Darken,
    /// Absolute difference: identical colors give black.
    Difference,
    /// Layer subtracted from the layers below, floored at black.
    Subtract,
    /// Multiply in the darks of the layers below, screen in their lights:
    /// raises contrast.
    Overlay,
    /// Like difference, with lower contrast.
    Exclusion,
}

impl BlendMode {
    /// Every blend mode, in display order.
    pub const ALL: [Self; 10] = [
        Self::Normal,
        Self::Add,
        Self::Multiply,
        Self::Screen,
        Self::Lighten,
        Self::Darken,
        Self::Difference,
        Self::Subtract,
        Self::Overlay,
        Self::Exclusion,
    ];
}

/// One row of the grid: a stack position, mixing settings and one optional
/// clip per column.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    pub(crate) id: LayerId,
    /// Name shown in the interface.
    pub name: String,
    /// Layer opacity.
    pub opacity: Unit,
    /// Blend mode against the layers below.
    #[serde(default)]
    pub blend_mode: BlendMode,
    /// When set, the layer is not rendered.
    #[serde(default)]
    pub bypass: bool,
    /// When any layer is solo, only solo layers are rendered.
    #[serde(default)]
    pub solo: bool,
    /// Crossfader side; `None` means the crossfader has no effect.
    #[serde(default)]
    pub crossfader_side: Option<CrossfaderSide>,
    /// Duration of the fade when the layer switches clips; `0` is a cut.
    #[serde(default)]
    pub transition_ms: u32,
    /// One entry per column, in column order.
    pub(crate) clips: Vec<Option<Clip>>,
    /// Column of the clip currently playing. Runtime state, not saved.
    #[serde(skip)]
    pub(crate) active: Option<ColumnId>,
    /// Number of clip starts on this layer. Runtime state, not saved.
    #[serde(skip)]
    pub(crate) trigger_count: u64,
}

impl Layer {
    pub(crate) fn new(id: LayerId, name: String, column_count: usize) -> Self {
        Self {
            id,
            name,
            opacity: Unit::ONE,
            blend_mode: BlendMode::default(),
            bypass: false,
            solo: false,
            crossfader_side: None,
            transition_ms: 0,
            clips: vec![None; column_count],
            active: None,
            trigger_count: 0,
        }
    }

    /// Stable identifier of the layer.
    pub fn id(&self) -> LayerId {
        self.id
    }

    /// Clips of this layer, one entry per column, in column order.
    pub fn clips(&self) -> &[Option<Clip>] {
        &self.clips
    }

    /// Column of the clip currently playing on this layer.
    pub fn active_column(&self) -> Option<ColumnId> {
        self.active
    }

    /// Increases every time a clip starts on this layer, including when the
    /// playing clip is triggered again: the engine then restarts it.
    pub fn trigger_count(&self) -> u64 {
        self.trigger_count
    }

    /// Whether both layers hold the same saved content, ignoring what is
    /// playing.
    pub fn same_content(&self, other: &Self) -> bool {
        self.id == other.id
            && self.name == other.name
            && self.opacity == other.opacity
            && self.blend_mode == other.blend_mode
            && self.bypass == other.bypass
            && self.solo == other.solo
            && self.crossfader_side == other.crossfader_side
            && self.transition_ms == other.transition_ms
            && self.clips == other.clips
    }
}
