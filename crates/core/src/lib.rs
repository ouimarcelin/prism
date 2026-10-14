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
mod crossfader;
mod effect;
mod ids;
mod layer;
mod project;
mod value;

pub use clip::{
    CUE_POINT_COUNT, Clip, DEFAULT_SEQUENCE_RATE, Direction, LoopMode, MAX_SPEED, MediaSource,
    Playback,
};
pub use composition::{
    Column, Composition, DEFAULT_COLUMN_COUNT, DEFAULT_LAYER_COUNT, FrameRate, MAX_DIMENSION,
    ModelError, Resolution,
};
pub use crossfader::{Crossfader, CrossfaderCurve, CrossfaderSide};
pub use effect::Effect;
pub use ids::{ColumnId, LayerId, Slot};
pub use layer::{BlendMode, Layer};
pub use project::{FILE_EXTENSION, FORMAT_VERSION, MissingMedia, Project, ProjectError};
pub use value::{Bpm, MAX_BPM, MIN_BPM, Unit};
