//! Data model of a Prism project.
//!
//! A [`Project`] holds one [`Composition`]: a grid of [`Clip`]s organised in
//! [`Layer`]s (rows, composited bottom to top) and [`Column`]s. Triggering a
//! clip starts it on its layer; triggering a column starts all its clips at
//! once. Layers are mixed with an opacity, a [`BlendMode`] and an optional
//! side of the A/B [`Crossfader`].
//!
//! This crate has no GPU, media or user interface dependency: it only holds
//! state, enforces its invariants and reads and writes project files.

#![forbid(unsafe_code)]

mod clip;
mod composition;
mod control;
mod crossfader;
mod effect;
mod ids;
mod layer;
mod lighting;
mod preset;
mod project;
mod stage;
mod value;
mod warp;

pub use clip::{
    CUE_POINT_COUNT, Clip, DASHBOARD_SIZE, DEFAULT_SEQUENCE_RATE, Direction, LoopMode, MAX_SPEED,
    MediaSource, Playback,
};
pub use composition::{
    Column, Composition, DEFAULT_COLUMN_COUNT, DEFAULT_LAYER_COUNT, FrameRate, MAX_DIMENSION,
    ModelError, Resolution,
};
pub use control::{ControlSource, ControlTarget, EffectOwner, Mapping};
pub use crossfader::{Crossfader, CrossfaderCurve, CrossfaderSide};
pub use effect::{Animation, BAND_COUNT, Band, Effect, MIN_PERIOD, Moment, Period, Shape};
pub use ids::{ColumnId, LayerId, Slot};
pub use layer::{BlendMode, Layer};
pub use lighting::{ColorOrder, DmxProtocol, Fixture, FixtureId, Lighting, MAX_FIXTURE_PIXELS};
pub use preset::{
    LayerPreset, PRESET_EXTENSION, PRESET_FORMAT_VERSION, Preset, PresetContent, preset_file_name,
};
pub use project::{
    FILE_EXTENSION, FORMAT_VERSION, MissingMedia, Project, ProjectError, STAGE_EXTENSION,
    load_stage, save_stage,
};
pub use stage::{
    Area, EdgeBlend, Flip, MAX_MASK_POINTS, MAX_SCREEN_SIZE, Mask, Point, Quad, Screen, ScreenId,
    Simulation, Slice, SliceId, Stage,
};
pub use value::{Bpm, MAX_BPM, MIN_BPM, Unit};
pub use warp::{MAX_WARP_CELLS, Warp, WarpCurve};
