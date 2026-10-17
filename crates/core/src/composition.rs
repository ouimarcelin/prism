//! The composition: output format, clip grid, crossfader and clip triggering.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::{
    BAND_COUNT, Bpm, Clip, ColumnId, Crossfader, Effect, Layer, LayerId, Mapping, Moment, Slot,
    Stage, Unit,
};

/// Largest width or height accepted for a composition, in pixels.
pub const MAX_DIMENSION: u32 = 16_384;

/// Number of layers in a new default composition.
pub const DEFAULT_LAYER_COUNT: usize = 4;

/// Number of columns in a new default composition.
pub const DEFAULT_COLUMN_COUNT: usize = 8;

/// Size of the composition output, in pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Resolution {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

impl Resolution {
    /// 1920 × 1080.
    pub const HD_1080: Self = Self {
        width: 1920,
        height: 1080,
    };

    /// Whether both dimensions are within `1..=MAX_DIMENSION`.
    pub fn is_valid(self) -> bool {
        (1..=MAX_DIMENSION).contains(&self.width) && (1..=MAX_DIMENSION).contains(&self.height)
    }
}

/// Frame rate of the composition, as a fraction (for example 30000/1001).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FrameRate {
    /// Frames per `denominator` seconds.
    pub numerator: u32,
    /// Seconds per `numerator` frames.
    pub denominator: u32,
}

impl FrameRate {
    /// 60 frames per second.
    pub const FPS_60: Self = Self {
        numerator: 60,
        denominator: 1,
    };

    /// Whether both terms are positive.
    pub fn is_valid(self) -> bool {
        self.numerator > 0 && self.denominator > 0
    }

    /// Frames per second.
    pub fn as_f64(self) -> f64 {
        f64::from(self.numerator) / f64::from(self.denominator)
    }
}

/// Error raised by an invalid edit of the composition.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ModelError {
    /// The layer is not in the composition.
    #[error("{0} does not exist")]
    UnknownLayer(LayerId),
    /// The column is not in the composition.
    #[error("{0} does not exist")]
    UnknownColumn(ColumnId),
    /// A dimension is zero or larger than [`MAX_DIMENSION`].
    #[error("resolution {}x{} is outside 1..={}", .0.width, .0.height, MAX_DIMENSION)]
    InvalidResolution(Resolution),
    /// A term of the frame rate is zero.
    #[error("frame rate {}/{} is not positive", .0.numerator, .0.denominator)]
    InvalidFrameRate(FrameRate),
    /// A target position is past the end of the list.
    #[error("index {index} is past the end (length {len})")]
    IndexOutOfRange {
        /// Requested position.
        index: usize,
        /// Length of the list.
        len: usize,
    },
    /// Loaded data breaks an invariant of the model.
    #[error("{0}")]
    Inconsistent(String),
}

/// A column of the grid. Triggering a column starts all its clips at once.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Column {
    id: ColumnId,
    /// Name shown in the interface.
    pub name: String,
}

impl Column {
    /// Stable identifier of the column.
    pub fn id(&self) -> ColumnId {
        self.id
    }
}

/// The composition: output format, layers × columns grid of clips and crossfader.
///
/// Layers are ordered bottom to top: index 0 is rendered first. Every layer
/// always holds exactly one slot per column.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "CompositionRepr")]
pub struct Composition {
    /// Name shown in the interface.
    pub name: String,
    resolution: Resolution,
    frame_rate: FrameRate,
    columns: Vec<Column>,
    layers: Vec<Layer>,
    /// A/B crossfader.
    pub crossfader: Crossfader,
    /// Tempo, for everything that follows the beat.
    pub tempo: Bpm,
    /// Effects applied to the mixed image, first to last.
    pub effects: Vec<Effect>,
    /// Controller messages and what they drive.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub mappings: Vec<Mapping>,
    /// Screens and slices of the output.
    #[serde(skip_serializing_if = "Stage::is_empty")]
    pub stage: Stage,
    next_id: u32,
}

/// Unchecked form read from a file, validated by `TryFrom`.
#[derive(Deserialize)]
struct CompositionRepr {
    name: String,
    resolution: Resolution,
    frame_rate: FrameRate,
    columns: Vec<Column>,
    layers: Vec<Layer>,
    #[serde(default)]
    crossfader: Crossfader,
    #[serde(default)]
    tempo: Bpm,
    #[serde(default)]
    effects: Vec<Effect>,
    #[serde(default)]
    mappings: Vec<Mapping>,
    #[serde(default)]
    stage: Stage,
    #[serde(default)]
    next_id: u32,
}

impl TryFrom<CompositionRepr> for Composition {
    type Error = ModelError;

    fn try_from(repr: CompositionRepr) -> Result<Self, ModelError> {
        check_resolution(repr.resolution)?;
        check_frame_rate(repr.frame_rate)?;

        let mut column_ids = HashSet::new();
        for column in &repr.columns {
            if !column_ids.insert(column.id) {
                return Err(ModelError::Inconsistent(format!("duplicate {}", column.id)));
            }
        }
        let mut layer_ids = HashSet::new();
        for layer in &repr.layers {
            if !layer_ids.insert(layer.id()) {
                return Err(ModelError::Inconsistent(format!(
                    "duplicate {}",
                    layer.id()
                )));
            }
            if layer.clips.len() != repr.columns.len() {
                return Err(ModelError::Inconsistent(format!(
                    "{} has {} slots for {} columns",
                    layer.id(),
                    layer.clips.len(),
                    repr.columns.len()
                )));
            }
        }

        let highest = column_ids
            .iter()
            .map(|id| id.0)
            .chain(layer_ids.iter().map(|id| id.0))
            .max();
        let next_free = match highest {
            Some(id) => id
                .checked_add(1)
                .ok_or_else(|| ModelError::Inconsistent("identifier space exhausted".into()))?,
            None => 0,
        };

        Ok(Self {
            name: repr.name,
            resolution: repr.resolution,
            frame_rate: repr.frame_rate,
            columns: repr.columns,
            layers: repr.layers,
            crossfader: repr.crossfader,
            tempo: repr.tempo,
            effects: repr.effects,
            mappings: repr.mappings,
            stage: repr.stage,
            next_id: repr.next_id.max(next_free),
        })
    }
}

impl Default for Composition {
    /// 1920 × 1080 at 60 fps, with [`DEFAULT_LAYER_COUNT`] layers and
    /// [`DEFAULT_COLUMN_COUNT`] columns.
    fn default() -> Self {
        Self::empty("Composition".into(), Resolution::HD_1080, FrameRate::FPS_60)
            .with_grid(DEFAULT_LAYER_COUNT, DEFAULT_COLUMN_COUNT)
    }
}

impl Composition {
    /// Creates an empty composition, with no layer and no column.
    pub fn new(
        name: impl Into<String>,
        resolution: Resolution,
        frame_rate: FrameRate,
    ) -> Result<Self, ModelError> {
        check_resolution(resolution)?;
        check_frame_rate(frame_rate)?;
        Ok(Self::empty(name.into(), resolution, frame_rate))
    }

    fn empty(name: String, resolution: Resolution, frame_rate: FrameRate) -> Self {
        Self {
            name,
            resolution,
            frame_rate,
            columns: Vec::new(),
            layers: Vec::new(),
            crossfader: Crossfader::default(),
            tempo: Bpm::DEFAULT,
            effects: Vec::new(),
            mappings: Vec::new(),
            stage: Stage::default(),
            next_id: 0,
        }
    }

    /// Adds `layer_count` layers and `column_count` columns with default names.
    pub fn with_grid(mut self, layer_count: usize, column_count: usize) -> Self {
        for i in 0..column_count {
            self.add_column(format!("Colonne {}", i + 1));
        }
        for i in 0..layer_count {
            self.add_layer(format!("Couche {}", i + 1));
        }
        self
    }

    /// Output size.
    pub fn resolution(&self) -> Resolution {
        self.resolution
    }

    /// Changes the output size.
    pub fn set_resolution(&mut self, resolution: Resolution) -> Result<(), ModelError> {
        check_resolution(resolution)?;
        self.resolution = resolution;
        Ok(())
    }

    /// Output frame rate.
    pub fn frame_rate(&self) -> FrameRate {
        self.frame_rate
    }

    /// Changes the output frame rate.
    pub fn set_frame_rate(&mut self, frame_rate: FrameRate) -> Result<(), ModelError> {
        check_frame_rate(frame_rate)?;
        self.frame_rate = frame_rate;
        Ok(())
    }

    /// Layers, bottom to top.
    pub fn layers(&self) -> &[Layer] {
        &self.layers
    }

    /// Columns, left to right.
    pub fn columns(&self) -> &[Column] {
        &self.columns
    }

    /// Layer with this identifier.
    pub fn layer(&self, id: LayerId) -> Option<&Layer> {
        self.layers.iter().find(|layer| layer.id() == id)
    }

    /// Mutable layer with this identifier.
    pub fn layer_mut(&mut self, id: LayerId) -> Option<&mut Layer> {
        self.layers.iter_mut().find(|layer| layer.id() == id)
    }

    /// Column with this identifier.
    pub fn column(&self, id: ColumnId) -> Option<&Column> {
        self.columns.iter().find(|column| column.id == id)
    }

    /// Mutable column with this identifier.
    pub fn column_mut(&mut self, id: ColumnId) -> Option<&mut Column> {
        self.columns.iter_mut().find(|column| column.id == id)
    }

    /// Stack position of a layer, 0 being the bottom.
    pub fn layer_index(&self, id: LayerId) -> Option<usize> {
        self.layers.iter().position(|layer| layer.id() == id)
    }

    /// Position of a column, 0 being the left.
    pub fn column_index(&self, id: ColumnId) -> Option<usize> {
        self.columns.iter().position(|column| column.id == id)
    }

    fn allocate_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Adds an empty layer on top of the stack.
    pub fn add_layer(&mut self, name: impl Into<String>) -> LayerId {
        let id = LayerId(self.allocate_id());
        self.layers
            .push(Layer::new(id, name.into(), self.columns.len()));
        id
    }

    /// Removes a layer and its clips.
    pub fn remove_layer(&mut self, id: LayerId) -> Result<Layer, ModelError> {
        let index = self.layer_index(id).ok_or(ModelError::UnknownLayer(id))?;
        Ok(self.layers.remove(index))
    }

    /// Moves a layer to another stack position.
    pub fn move_layer(&mut self, id: LayerId, new_index: usize) -> Result<(), ModelError> {
        let index = self.layer_index(id).ok_or(ModelError::UnknownLayer(id))?;
        check_index(new_index, self.layers.len())?;
        let layer = self.layers.remove(index);
        self.layers.insert(new_index, layer);
        Ok(())
    }

    /// Adds an empty column on the right.
    pub fn add_column(&mut self, name: impl Into<String>) -> ColumnId {
        let id = ColumnId(self.allocate_id());
        self.columns.push(Column {
            id,
            name: name.into(),
        });
        for layer in &mut self.layers {
            layer.clips.push(None);
        }
        id
    }

    /// Removes a column and its clips. Layers playing a clip of this column stop.
    pub fn remove_column(&mut self, id: ColumnId) -> Result<Column, ModelError> {
        let index = self.column_index(id).ok_or(ModelError::UnknownColumn(id))?;
        for layer in &mut self.layers {
            layer.clips.remove(index);
            if layer.active == Some(id) {
                layer.active = None;
            }
        }
        Ok(self.columns.remove(index))
    }

    /// Moves a column, with its clips, to another position.
    pub fn move_column(&mut self, id: ColumnId, new_index: usize) -> Result<(), ModelError> {
        let index = self.column_index(id).ok_or(ModelError::UnknownColumn(id))?;
        check_index(new_index, self.columns.len())?;
        let column = self.columns.remove(index);
        self.columns.insert(new_index, column);
        for layer in &mut self.layers {
            let clip = layer.clips.remove(index);
            layer.clips.insert(new_index, clip);
        }
        Ok(())
    }

    fn slot_indices(&self, slot: Slot) -> Result<(usize, usize), ModelError> {
        let layer = self
            .layer_index(slot.layer)
            .ok_or(ModelError::UnknownLayer(slot.layer))?;
        let column = self
            .column_index(slot.column)
            .ok_or(ModelError::UnknownColumn(slot.column))?;
        Ok((layer, column))
    }

    /// Clip in a slot.
    pub fn clip(&self, slot: Slot) -> Option<&Clip> {
        let (layer, column) = self.slot_indices(slot).ok()?;
        self.layers[layer].clips[column].as_ref()
    }

    /// Mutable clip in a slot.
    pub fn clip_mut(&mut self, slot: Slot) -> Option<&mut Clip> {
        let (layer, column) = self.slot_indices(slot).ok()?;
        self.layers[layer].clips[column].as_mut()
    }

    /// Places a clip in a slot, or empties it with `None`. Returns the previous
    /// clip. Emptying the playing slot stops its layer.
    pub fn set_clip(&mut self, slot: Slot, clip: Option<Clip>) -> Result<Option<Clip>, ModelError> {
        let (layer, column) = self.slot_indices(slot)?;
        let layer = &mut self.layers[layer];
        if clip.is_none() && layer.active == Some(slot.column) {
            layer.active = None;
        }
        Ok(std::mem::replace(&mut layer.clips[column], clip))
    }

    /// Moves a clip to another slot, possibly on another layer. Returns the clip
    /// it replaces. Does nothing when `from` is empty. The source layer stops if
    /// it was playing the moved clip.
    pub fn move_clip(&mut self, from: Slot, to: Slot) -> Result<Option<Clip>, ModelError> {
        let (from_layer, from_column) = self.slot_indices(from)?;
        let (to_layer, to_column) = self.slot_indices(to)?;
        if from == to {
            return Ok(None);
        }
        let Some(clip) = self.layers[from_layer].clips[from_column].take() else {
            return Ok(None);
        };
        if self.layers[from_layer].active == Some(from.column) {
            self.layers[from_layer].active = None;
        }
        Ok(self.layers[to_layer].clips[to_column].replace(clip))
    }

    /// Starts the clip of a slot on its layer. Returns `false`, and changes
    /// nothing, when the slot is empty.
    pub fn trigger_clip(&mut self, slot: Slot) -> Result<bool, ModelError> {
        let (layer, column) = self.slot_indices(slot)?;
        let layer = &mut self.layers[layer];
        if layer.clips[column].is_none() {
            return Ok(false);
        }
        layer.active = Some(slot.column);
        layer.trigger_count += 1;
        Ok(true)
    }

    /// Starts every clip of a column at once. Layers with an empty slot in this
    /// column stop, so the column plays as a complete scene.
    pub fn trigger_column(&mut self, id: ColumnId) -> Result<(), ModelError> {
        let column = self.column_index(id).ok_or(ModelError::UnknownColumn(id))?;
        for layer in &mut self.layers {
            layer.active = layer.clips[column].as_ref().map(|_| id);
            if layer.active.is_some() {
                layer.trigger_count += 1;
            }
        }
        Ok(())
    }

    /// Stops the clip playing on a layer.
    pub fn stop_layer(&mut self, id: LayerId) -> Result<(), ModelError> {
        let layer = self.layer_mut(id).ok_or(ModelError::UnknownLayer(id))?;
        layer.active = None;
        Ok(())
    }

    /// Stops every layer.
    pub fn stop_all(&mut self) {
        for layer in &mut self.layers {
            layer.active = None;
        }
    }

    /// A copy without what plays (clips playing, crossfader position): the
    /// content the user edits.
    pub fn without_playback(&self) -> Self {
        let mut copy = self.clone();
        copy.crossfader.position = Crossfader::default().position;
        for layer in &mut copy.layers {
            layer.active = None;
            layer.trigger_count = 0;
        }
        copy
    }

    /// Takes what plays from `other`: the crossfader position, the clip
    /// playing and the trigger count of each layer present in both, if its
    /// column still exists.
    pub fn take_playback_from(&mut self, other: &Composition) {
        self.crossfader.position = other.crossfader.position;
        let columns: Vec<ColumnId> = self.columns.iter().map(|column| column.id).collect();
        for layer in &mut self.layers {
            if let Some(playing) = other.layer(layer.id()) {
                layer.trigger_count = playing.trigger_count;
                layer.active = playing.active.filter(|column| columns.contains(column));
            }
        }
    }

    /// Clip playing on a layer, with its column.
    pub fn active_clip(&self, id: LayerId) -> Option<(ColumnId, &Clip)> {
        let layer = self.layer(id)?;
        let column = layer.active?;
        let index = self.column_index(column)?;
        layer.clips[index].as_ref().map(|clip| (column, clip))
    }

    /// Opacity applied when rendering a layer: its own opacity times its
    /// crossfader level. Zero when the layer is bypassed, or when another layer
    /// is solo and this one is not.
    pub fn effective_opacity(&self, id: LayerId) -> Option<f32> {
        self.opacity_with(id, |layer| layer.opacity.get())
    }

    /// Like [`Composition::effective_opacity`], at moment `at` with the
    /// sound at levels `audio` (in the order of [`crate::Band::ALL`]): an animated
    /// opacity takes its value then, an opacity following the sound the
    /// level of its band.
    pub fn effective_opacity_at(
        &self,
        id: LayerId,
        at: Moment,
        audio: [f32; BAND_COUNT],
    ) -> Option<f32> {
        self.opacity_with(id, |layer| {
            match (&layer.opacity_animation, layer.opacity_audio) {
                (Some(animation), _) => Unit::new(animation.value_at(at)).get(),
                (None, Some(band)) => Unit::new(audio[band.index()]).get(),
                (None, None) => layer.opacity.get(),
            }
        })
    }

    fn opacity_with(&self, id: LayerId, opacity: impl Fn(&Layer) -> f32) -> Option<f32> {
        let layer = self.layer(id)?;
        let any_solo = self.layers.iter().any(|layer| layer.solo);
        if layer.bypass || (any_solo && !layer.solo) {
            return Some(0.0);
        }
        let gain = layer
            .crossfader_side
            .map_or(1.0, |side| self.crossfader.gain(side));
        Some(opacity(layer) * gain)
    }

    /// Every clip of the grid with its slot, layer by layer.
    pub fn clips(&self) -> impl Iterator<Item = (Slot, &Clip)> {
        self.layers.iter().flat_map(|layer| {
            layer
                .clips
                .iter()
                .zip(&self.columns)
                .filter_map(|(clip, column)| {
                    clip.as_ref()
                        .map(|clip| (Slot::new(layer.id(), column.id), clip))
                })
        })
    }

    /// Whether both compositions hold the same saved content, ignoring what
    /// is playing: triggering clips does not modify a project.
    pub fn same_content(&self, other: &Self) -> bool {
        self.name == other.name
            && self.resolution == other.resolution
            && self.frame_rate == other.frame_rate
            && self.columns == other.columns
            && self.crossfader == other.crossfader
            && self.tempo == other.tempo
            && self.effects == other.effects
            && self.mappings == other.mappings
            && self.layers.len() == other.layers.len()
            && self
                .layers
                .iter()
                .zip(&other.layers)
                .all(|(a, b)| a.same_content(b))
    }

    /// Every clip of the grid, mutable.
    pub fn clips_mut(&mut self) -> impl Iterator<Item = &mut Clip> {
        self.layers
            .iter_mut()
            .flat_map(|layer| layer.clips.iter_mut().flatten())
    }

    /// Every effect of the composition, its layers and clips, generators
    /// included, mutable.
    pub fn all_effects_mut(&mut self) -> impl Iterator<Item = &mut Effect> {
        let layers = self.layers.iter_mut().flat_map(|layer| {
            let clips = layer.clips.iter_mut().flatten().flat_map(|clip| {
                let Clip {
                    source, effects, ..
                } = clip;
                source.generator_mut().into_iter().chain(effects.iter_mut())
            });
            layer.effects.iter_mut().chain(clips)
        });
        self.effects.iter_mut().chain(layers)
    }
}

fn check_resolution(resolution: Resolution) -> Result<(), ModelError> {
    if resolution.is_valid() {
        Ok(())
    } else {
        Err(ModelError::InvalidResolution(resolution))
    }
}

fn check_frame_rate(frame_rate: FrameRate) -> Result<(), ModelError> {
    if frame_rate.is_valid() {
        Ok(())
    } else {
        Err(ModelError::InvalidFrameRate(frame_rate))
    }
}

fn check_index(index: usize, len: usize) -> Result<(), ModelError> {
    if index < len {
        Ok(())
    } else {
        Err(ModelError::IndexOutOfRange { index, len })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CrossfaderSide, Unit};

    fn grid(layers: usize, columns: usize) -> Composition {
        Composition::new("test", Resolution::HD_1080, FrameRate::FPS_60)
            .unwrap()
            .with_grid(layers, columns)
    }

    fn slot(comp: &Composition, layer: usize, column: usize) -> Slot {
        Slot::new(comp.layers()[layer].id(), comp.columns()[column].id())
    }

    fn assert_rectangular(comp: &Composition) {
        for layer in comp.layers() {
            assert_eq!(layer.clips().len(), comp.columns().len());
        }
    }

    #[test]
    fn default_composition_is_1080p60_with_a_grid() {
        let comp = Composition::default();
        assert_eq!(comp.resolution(), Resolution::HD_1080);
        assert_eq!(comp.frame_rate(), FrameRate::FPS_60);
        assert_eq!(comp.layers().len(), DEFAULT_LAYER_COUNT);
        assert_eq!(comp.columns().len(), DEFAULT_COLUMN_COUNT);
        assert_rectangular(&comp);
    }

    #[test]
    fn rejects_invalid_format() {
        let zero = Resolution {
            width: 0,
            height: 1080,
        };
        assert_eq!(
            Composition::new("x", zero, FrameRate::FPS_60),
            Err(ModelError::InvalidResolution(zero))
        );
        let mut comp = grid(1, 1);
        let bad_rate = FrameRate {
            numerator: 60,
            denominator: 0,
        };
        assert_eq!(
            comp.set_frame_rate(bad_rate),
            Err(ModelError::InvalidFrameRate(bad_rate))
        );
        assert_eq!(comp.frame_rate(), FrameRate::FPS_60);
    }

    #[test]
    fn grid_stays_rectangular_through_edits() {
        let mut comp = grid(2, 3);
        comp.add_column("extra");
        comp.add_layer("top");
        assert_rectangular(&comp);
        let second_column = comp.columns()[1].id();
        comp.remove_column(second_column).unwrap();
        let bottom = comp.layers()[0].id();
        comp.remove_layer(bottom).unwrap();
        assert_eq!(comp.layers().len(), 2);
        assert_eq!(comp.columns().len(), 3);
        assert_rectangular(&comp);
    }

    #[test]
    fn tempo_is_saved_and_defaults_to_120() {
        let mut comp = grid(1, 1);
        assert_eq!(comp.tempo.get(), 120.0);
        let saved = comp.clone();
        comp.tempo = Bpm::new(128.0);
        assert!(!comp.same_content(&saved));

        let json = serde_json::to_string(&comp).unwrap();
        let read: Composition = serde_json::from_str(&json).unwrap();
        assert_eq!(read.tempo.get(), 128.0);

        // Files written before the tempo existed.
        let mut value: serde_json::Value = serde_json::from_str(&json).unwrap();
        value.as_object_mut().unwrap().remove("tempo");
        let old: Composition = serde_json::from_value(value).unwrap();
        assert_eq!(old.tempo, Bpm::DEFAULT);
    }

    #[test]
    fn animated_opacity_follows_the_beat() {
        let mut comp = grid(1, 1);
        let id = comp.layers()[0].id();
        comp.layer_mut(id).unwrap().opacity = Unit::new(0.4);
        assert_eq!(
            comp.effective_opacity_at(id, Moment::new(1.0, 0.0), [0.0; BAND_COUNT]),
            Some(0.4)
        );
        let mut animation = crate::Animation::new(0.0, 2.0);
        animation.shape = crate::Shape::Ramp;
        animation.period = crate::Period::Beats(4.0);
        comp.layer_mut(id).unwrap().opacity_animation = Some(animation);
        assert_eq!(
            comp.effective_opacity_at(id, Moment::new(1.0, 0.0), [0.0; BAND_COUNT]),
            Some(0.5)
        );
        // Out of range values are clamped; the set opacity is kept.
        assert_eq!(
            comp.effective_opacity_at(id, Moment::new(3.0, 0.0), [0.0; BAND_COUNT]),
            Some(1.0)
        );
        assert_eq!(comp.effective_opacity(id), Some(0.4));
        // Following the bass, unless animated.
        comp.layer_mut(id).unwrap().opacity_audio = Some(crate::Band::Bass);
        assert_eq!(
            comp.effective_opacity_at(id, Moment::new(3.0, 0.0), [0.0, 0.25, 0.0, 0.0]),
            Some(1.0)
        );
        comp.layer_mut(id).unwrap().opacity_animation = None;
        assert_eq!(
            comp.effective_opacity_at(id, Moment::new(3.0, 0.0), [0.0, 0.25, 0.0, 0.0]),
            Some(0.25)
        );
        comp.layer_mut(id).unwrap().bypass = true;
        assert_eq!(
            comp.effective_opacity_at(id, Moment::new(1.0, 0.0), [0.0; BAND_COUNT]),
            Some(0.0)
        );
    }

    #[test]
    fn dashboards_and_links_are_saved() {
        let mut comp = grid(1, 1);
        let slot = slot(&comp, 0, 0);
        let mut clip = Clip::from_file("/a.mov");
        clip.dashboard[2] = Unit::new(0.75);
        let mut blur = crate::Effect::new("blur");
        blur.links.insert("radius".into(), 2);
        clip.effects.push(blur);
        comp.set_clip(slot, Some(clip)).unwrap();
        let json = serde_json::to_string(&comp).unwrap();
        let read: Composition = serde_json::from_str(&json).unwrap();
        assert_eq!(read, comp);
        // A clip saved before dashboards existed starts with its sliders at 0.
        let old: Clip =
            serde_json::from_str(r#"{"name":"a","source":{"kind":"file","path":"/a.mov"}}"#)
                .unwrap();
        assert_eq!(old.dashboard, [Unit::ZERO; crate::DASHBOARD_SIZE]);
    }

    #[test]
    fn identifiers_are_never_reused() {
        let mut comp = grid(1, 1);
        let column = comp.columns()[0].id();
        comp.remove_column(column).unwrap();
        let new_column = comp.add_column("again");
        assert_ne!(column, new_column);
    }

    #[test]
    fn moving_a_column_moves_its_clips() {
        let mut comp = grid(1, 3);
        let first = slot(&comp, 0, 0);
        comp.set_clip(first, Some(Clip::from_file("/a.mov")))
            .unwrap();
        comp.move_column(first.column, 2).unwrap();
        assert_eq!(comp.column_index(first.column), Some(2));
        assert_eq!(
            comp.layers()[0].clips()[2]
                .as_ref()
                .map(|c| c.name.as_str()),
            Some("a")
        );
        assert_eq!(comp.clip(first).map(|c| c.name.as_str()), Some("a"));
        assert_eq!(
            comp.move_column(first.column, 3),
            Err(ModelError::IndexOutOfRange { index: 3, len: 3 })
        );
    }

    #[test]
    fn moving_a_layer_keeps_its_identity() {
        let mut comp = grid(3, 1);
        let bottom = comp.layers()[0].id();
        comp.move_layer(bottom, 2).unwrap();
        assert_eq!(comp.layer_index(bottom), Some(2));
    }

    #[test]
    fn set_and_move_clips() {
        let mut comp = grid(2, 2);
        let a = slot(&comp, 0, 0);
        let b = slot(&comp, 1, 1);
        assert_eq!(comp.set_clip(a, Some(Clip::from_file("/a.mov"))), Ok(None));
        comp.set_clip(b, Some(Clip::from_file("/b.mov"))).unwrap();

        let displaced = comp.move_clip(a, b).unwrap();
        assert_eq!(displaced.map(|c| c.name), Some("b".to_string()));
        assert!(comp.clip(a).is_none());
        assert_eq!(comp.clip(b).map(|c| c.name.as_str()), Some("a"));

        // Moving from an empty slot leaves the target untouched.
        assert_eq!(comp.move_clip(a, b), Ok(None));
        assert!(comp.clip(b).is_some());
    }

    #[test]
    fn triggering_an_empty_slot_changes_nothing() {
        let mut comp = grid(1, 2);
        let full = slot(&comp, 0, 0);
        let empty = slot(&comp, 0, 1);
        comp.set_clip(full, Some(Clip::from_file("/a.mov")))
            .unwrap();
        assert_eq!(comp.trigger_clip(full), Ok(true));
        assert_eq!(comp.trigger_clip(empty), Ok(false));
        assert_eq!(comp.layers()[0].active_column(), Some(full.column));
    }

    #[test]
    fn triggering_a_column_plays_it_as_a_scene() {
        let mut comp = grid(3, 2);
        // Layer 0 has a clip in both columns, layer 1 only in column 0,
        // layer 2 only in column 1.
        for (layer, column) in [(0, 0), (0, 1), (1, 0), (2, 1)] {
            let s = slot(&comp, layer, column);
            comp.set_clip(s, Some(Clip::from_file("/x.mov"))).unwrap();
        }
        let first = comp.columns()[0].id();
        let second = comp.columns()[1].id();

        comp.trigger_column(first).unwrap();
        let active: Vec<_> = comp.layers().iter().map(Layer::active_column).collect();
        assert_eq!(active, [Some(first), Some(first), None]);

        comp.trigger_column(second).unwrap();
        let active: Vec<_> = comp.layers().iter().map(Layer::active_column).collect();
        assert_eq!(active, [Some(second), None, Some(second)]);
    }

    #[test]
    fn every_start_increases_the_trigger_count() {
        let mut comp = grid(2, 2);
        let s = slot(&comp, 0, 0);
        comp.set_clip(s, Some(Clip::from_file("/a.mov"))).unwrap();
        assert_eq!(comp.layers()[0].trigger_count(), 0);
        comp.trigger_clip(s).unwrap();
        comp.trigger_clip(s).unwrap();
        assert_eq!(comp.layers()[0].trigger_count(), 2);
        comp.trigger_column(s.column).unwrap();
        assert_eq!(comp.layers()[0].trigger_count(), 3);
        // The second layer has no clip in this column: nothing starts.
        assert_eq!(comp.layers()[1].trigger_count(), 0);
        // An empty slot starts nothing either.
        comp.trigger_clip(slot(&comp, 0, 1)).unwrap();
        assert_eq!(comp.layers()[0].trigger_count(), 3);
    }

    #[test]
    fn triggering_does_not_change_the_content() {
        let mut comp = grid(1, 1);
        let s = slot(&comp, 0, 0);
        comp.set_clip(s, Some(Clip::from_file("/a.mov"))).unwrap();
        let saved = comp.clone();
        comp.trigger_clip(s).unwrap();
        assert_ne!(comp, saved);
        assert!(comp.same_content(&saved));
        comp.layer_mut(s.layer).unwrap().opacity = Unit::new(0.5);
        assert!(!comp.same_content(&saved));
    }

    #[test]
    fn removing_the_playing_clip_stops_the_layer() {
        let mut comp = grid(1, 2);
        let s = slot(&comp, 0, 1);
        comp.set_clip(s, Some(Clip::from_file("/a.mov"))).unwrap();
        comp.trigger_clip(s).unwrap();
        assert!(comp.active_clip(s.layer).is_some());
        comp.set_clip(s, None).unwrap();
        assert!(comp.active_clip(s.layer).is_none());

        comp.set_clip(s, Some(Clip::from_file("/a.mov"))).unwrap();
        comp.trigger_clip(s).unwrap();
        comp.remove_column(s.column).unwrap();
        assert_eq!(comp.layers()[0].active_column(), None);
    }

    #[test]
    fn effective_opacity_combines_bypass_solo_and_crossfader() {
        let mut comp = grid(3, 1);
        let ids: Vec<_> = comp.layers().iter().map(Layer::id).collect();
        comp.crossfader.curve = crate::CrossfaderCurve::Linear;
        comp.crossfader.position = Unit::new(0.25);
        {
            let layer = comp.layer_mut(ids[0]).unwrap();
            layer.opacity = Unit::new(0.5);
            layer.crossfader_side = Some(CrossfaderSide::B);
        }
        assert_eq!(comp.effective_opacity(ids[0]), Some(0.125));
        assert_eq!(comp.effective_opacity(ids[1]), Some(1.0));

        comp.layer_mut(ids[1]).unwrap().bypass = true;
        assert_eq!(comp.effective_opacity(ids[1]), Some(0.0));

        comp.layer_mut(ids[2]).unwrap().solo = true;
        assert_eq!(comp.effective_opacity(ids[0]), Some(0.0));
        assert_eq!(comp.effective_opacity(ids[2]), Some(1.0));
    }

    #[test]
    fn deserializing_rejects_a_ragged_grid() {
        let comp = grid(1, 2);
        let mut json = serde_json::to_value(&comp).unwrap();
        json["layers"][0]["clips"] = serde_json::json!([null]);
        let error = serde_json::from_value::<Composition>(json).unwrap_err();
        assert!(
            error.to_string().contains("1 slots for 2 columns"),
            "{error}"
        );
    }

    #[test]
    fn deserializing_rejects_duplicate_identifiers() {
        let mut comp = grid(2, 1);
        comp.layers[1].id = comp.layers[0].id;
        let json = serde_json::to_value(&comp).unwrap();
        let error = serde_json::from_value::<Composition>(json).unwrap_err();
        assert!(error.to_string().contains("duplicate"), "{error}");
    }

    #[test]
    fn loaded_composition_allocates_fresh_identifiers() {
        let comp = grid(2, 2);
        let mut json = serde_json::to_value(&comp).unwrap();
        json.as_object_mut().unwrap().remove("next_id");
        let mut loaded: Composition = serde_json::from_value(json).unwrap();
        let new_layer = loaded.add_layer("new");
        assert!(comp.layers().iter().all(|layer| layer.id() != new_layer));
        assert!(new_layer.0 >= 4);
    }
}
