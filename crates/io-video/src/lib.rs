//! Video shared with other applications of the computer.
//!
//! On macOS, [`SyphonOutput`] publishes the program image over Syphon.

#[cfg(target_os = "macos")]
mod syphon;

#[cfg(target_os = "macos")]
pub use syphon::SyphonOutput;
