//! Prism playback engine.
//!
//! The [`Engine`] runs a render thread at the composition frame rate. Every
//! frame it advances the playhead of each playing clip, uploads the frames
//! decoded by one worker thread per clip, mixes the layers and copies the
//! result into the [`ProgramOutput`] read by the outputs and the interface.
//!
//! The interface never waits for the render thread: it sends [`Command`]s
//! and reads the [`EngineStatus`] published after each frame. The render
//! thread never waits for the interface nor for decoding: a frame not decoded
//! in time leaves the previous one on screen.

mod engine;
mod lighting;
mod live;
mod playhead;
mod priority;
mod random;
mod sound;
mod status;
mod stream;
mod worker;

pub use engine::{Command, Engine, EngineState, Output, OutputImages, ProgramOutput};
pub use live::{cameras, ndi_sources, ndi_status, spout_senders, syphon_servers};
pub use status::{ClipState, EngineStatus, FrameSteps, Hitch, LayerStatus};
