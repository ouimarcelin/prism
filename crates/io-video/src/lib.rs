//! Video shared with other applications.
//!
//! On macOS, a [`SyphonServer`] publishes the program image over Syphon,
//! and a [`SyphonClient`] reads the image of another application. With the
//! `ndi` feature, an [`NdiSender`] publishes it on the local network and an
//! [`NdiReceiver`] reads a source of the network, through the NDI® library
//! installed on the computer.

#[cfg(any(target_os = "macos", target_os = "windows", feature = "ndi"))]
mod convert;
#[cfg(feature = "ndi")]
mod ndi;
#[cfg(any(target_os = "windows", feature = "ndi"))]
mod readback;
#[cfg(target_os = "macos")]
mod syphon;

#[cfg(feature = "ndi")]
pub use ndi::{NdiFrame, NdiReceiver, NdiSender, ndi_sources, ndi_version};
#[cfg(target_os = "macos")]
pub use syphon::{SyphonClient, SyphonServer, start_syphon_directory, syphon_servers};
