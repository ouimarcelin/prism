//! Sound of Prism: an audio input measured in frequency bands, whose
//! levels drive settings of effects, and the sound of the clips, mixed and
//! played on an output.
//!
//! An [`AudioInput`] feeds the sound of a device to an [`Analyzer`] in the
//! audio callback; the levels of the bands ([`prism_core::Band`]) are
//! published in [`AudioLevels`], read by the render thread at each frame
//! and by the interface. The render thread sends [`VoiceCommand`]s to a
//! [`Mixer`], which an [`AudioOutput`] plays.

mod analysis;
mod input;
mod levels;
mod mixer;
mod output;

pub use analysis::{Analyzer, RANGE_DB, WINDOW};
pub use input::{AudioError, AudioInput, input_devices};
pub use levels::{AudioLevels, MAX_GAIN_DB};
pub use mixer::{Mixer, VoiceCommand};
pub use output::{AudioOutput, Volume};
