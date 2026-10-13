//! Media decoding for Prism.
//!
//! - [`mov`] reads QuickTime and MP4 containers and locates every frame.
//! - [`hap`] decodes HAP frames into block-compressed textures that the GPU
//!   samples directly (see ADR 0003).
//! - [`HapDecoder`] combines both to decode any frame of a HAP file.
//!
//! This crate does not depend on the GPU: uploading textures is the job of
//! `prism-render`.

#![forbid(unsafe_code)]

mod decoder;
pub mod hap;
pub mod mov;
#[cfg(any(test, feature = "test-support"))]
pub mod test_support;

pub use decoder::{DecodeError, HapDecoder, is_hap};
pub use hap::{BlockFormat, ColorLayout, HapError, HapFrame, Plane};
pub use mov::{ContainerError, FourCc, Sample, VideoTrack};
