//! Media decoding for Prism.
//!
//! - [`mov`] reads QuickTime and MP4 containers and locates every frame.
//! - [`hap`] decodes HAP frames into block-compressed textures that the GPU
//!   samples directly (see ADR 0003).
//! - [`HapDecoder`] combines both to decode any frame of a HAP file.
//! - [`image`] decodes still images (PNG, JPEG) and PNG sequences.
//! - [`FrameSource`] puts every kind of media behind one interface:
//!   [`open_file`] and [`open_sequence`] pick the right decoder. On macOS,
//!   movies in other codecs (H.264, ProRes…) are decoded by AVFoundation.
//!
//! Unsafe code is limited to the AVFoundation decoder (`avf`), which calls
//! Apple frameworks.
//!
//! This crate does not depend on the GPU: uploading textures is the job of
//! `prism-render`.

#[cfg(target_os = "macos")]
mod avf;
mod decoder;
pub mod hap;
pub mod image;
pub mod mov;
mod source;
#[cfg(any(test, feature = "test-support"))]
pub mod test_support;

pub use decoder::{DecodeError, HapDecoder, is_hap};
pub use hap::{BlockFormat, ColorLayout, HapError, HapFrame, Plane};
pub use image::{IMAGE_EXTENSIONS, ImageSequence, StillImage, decode_image, natural_order};
pub use mov::{ContainerError, FourCc, Sample, VideoTrack};
pub use source::{
    ChannelOrder, Frame, FrameSource, MediaInfo, RgbaImage, YuvImage, YuvMatrix, open_file,
    open_sequence,
};
