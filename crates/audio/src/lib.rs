//! Sound of Prism: an audio input measured in frequency bands, whose
//! levels drive settings of effects.
//!
//! An [`AudioInput`] feeds the sound of a device to an [`Analyzer`] in the
//! audio callback; the levels of the bands ([`prism_core::Band`]) are
//! published in [`AudioLevels`], read by the render thread at each frame
//! and by the interface.

mod analysis;
mod input;
mod levels;

pub use analysis::{Analyzer, RANGE_DB, WINDOW};
pub use input::{AudioError, AudioInput, input_devices};
pub use levels::{AudioLevels, MAX_GAIN_DB};
