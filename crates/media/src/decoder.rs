//! Frame-accurate HAP decoding from a file.

use std::fs::File;
use std::io::{Read, Seek};
use std::path::Path;

use crate::hap::{self, HapError, HapFrame};
use crate::mov::{ContainerError, FourCc, VideoTrack};

/// Error raised while opening a file or decoding one of its frames.
#[derive(Debug, thiserror::Error)]
pub enum DecodeError {
    /// The file could not be opened.
    #[error("cannot open the file")]
    Open(#[source] std::io::Error),
    /// The container is invalid.
    #[error(transparent)]
    Container(#[from] ContainerError),
    /// A frame is invalid.
    #[error(transparent)]
    Hap(#[from] HapError),
    /// The video track does not use a HAP codec.
    #[error("codec {0} is not HAP")]
    NotHap(String),
    /// The frame index is past the end of the track.
    #[error("frame {index} is past the last frame ({count} frames)")]
    FrameOutOfRange {
        /// Requested frame.
        index: usize,
        /// Number of frames.
        count: usize,
    },
}

/// Whether a codec is one of the HAP variants.
pub fn is_hap(codec: FourCc) -> bool {
    matches!(
        &codec,
        b"Hap1" | b"Hap5" | b"HapY" | b"HapM" | b"HapA" | b"Hap7" | b"HapH"
    )
}

/// Reads and decodes HAP frames from a file, in any order.
///
/// Every HAP frame is independent, so any frame decodes directly: scratching
/// and random playback cost the same as forward playback.
pub struct HapDecoder<R = File> {
    reader: R,
    track: VideoTrack,
    packet: Vec<u8>,
}

impl HapDecoder<File> {
    /// Opens a HAP file.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, DecodeError> {
        let file = File::open(path).map_err(DecodeError::Open)?;
        Self::new(file)
    }
}

impl<R: Read + Seek> HapDecoder<R> {
    /// Reads the container of a HAP file.
    pub fn new(mut reader: R) -> Result<Self, DecodeError> {
        let track = VideoTrack::read(&mut reader)?;
        if !is_hap(track.codec) {
            return Err(DecodeError::NotHap(track.codec_name()));
        }
        Ok(Self {
            reader,
            track,
            packet: Vec::new(),
        })
    }

    /// Video track: size, timing, frame count.
    pub fn track(&self) -> &VideoTrack {
        &self.track
    }

    /// Decodes frame `index` into `frame`, reusing its buffers.
    pub fn decode_frame(&mut self, index: usize, frame: &mut HapFrame) -> Result<(), DecodeError> {
        let count = self.track.frame_count();
        if index >= count {
            return Err(DecodeError::FrameOutOfRange { index, count });
        }
        self.track
            .read_sample(&mut self.reader, index, &mut self.packet)?;
        hap::decode_frame(&self.packet, self.track.width, self.track.height, frame)?;
        Ok(())
    }
}
