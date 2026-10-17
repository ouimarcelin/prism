//! Video shared with other applications of the computer.
//!
//! On macOS, a [`SyphonServer`] publishes the program image over Syphon,
//! and a [`SyphonClient`] reads the image of another application.

#[cfg(target_os = "macos")]
mod syphon;

#[cfg(target_os = "macos")]
pub use syphon::{SyphonClient, SyphonServer, start_syphon_directory, syphon_servers};
