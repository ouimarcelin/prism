//! Media of any supported kind, behind one interface: open a file or folder,
//! read its size and timing, decode any frame.

use std::path::Path;

use crate::decoder::{DecodeError, HapDecoder};
use crate::hap::HapFrame;
use crate::image::{IMAGE_EXTENSIONS, ImageSequence, StillImage};
use crate::mov::VideoTrack;

/// 8-bit RGBA pixels, straight (non-premultiplied) alpha, rows from the top.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RgbaImage {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// `width × height × 4` bytes.
    pub pixels: Vec<u8>,
}

/// A decoded frame, ready to be uploaded to the GPU.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Frame {
    /// Block-compressed planes of a HAP frame.
    Hap(HapFrame),
    /// Uncompressed pixels.
    Rgba(RgbaImage),
}

impl Default for Frame {
    fn default() -> Self {
        Self::Hap(HapFrame::default())
    }
}

impl Frame {
    /// The HAP planes, turning the frame into a HAP frame if needed and
    /// keeping its buffers when it already is one.
    pub fn hap_mut(&mut self) -> &mut HapFrame {
        if !matches!(self, Self::Hap(_)) {
            *self = Self::Hap(HapFrame::default());
        }
        match self {
            Self::Hap(frame) => frame,
            Self::Rgba(_) => unreachable!("replaced just above"),
        }
    }

    /// The pixel buffer, turning the frame into an RGBA frame if needed.
    pub fn rgba_mut(&mut self) -> &mut RgbaImage {
        if !matches!(self, Self::Rgba(_)) {
            *self = Self::Rgba(RgbaImage::default());
        }
        match self {
            Self::Rgba(image) => image,
            Self::Hap(_) => unreachable!("replaced just above"),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Timing {
    /// Times of every sample of a movie track.
    Track(VideoTrack),
    /// Evenly spaced frames.
    Uniform { frames: usize, rate: f64 },
}

/// Size and timing of a media.
#[derive(Clone, Debug, PartialEq)]
pub struct MediaInfo {
    /// Format, for display: a codec such as `Hap1`, or `PNG`.
    pub format: String,
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    timing: Timing,
}

impl MediaInfo {
    /// Information of a movie track.
    pub fn from_track(track: &VideoTrack) -> Self {
        Self {
            format: track.codec_name(),
            width: track.width,
            height: track.height,
            timing: Timing::Track(track.clone()),
        }
    }

    /// Information of `frames` frames shown `rate` times per second.
    pub fn uniform(
        format: impl Into<String>,
        width: u32,
        height: u32,
        frames: usize,
        rate: f64,
    ) -> Self {
        Self {
            format: format.into(),
            width,
            height,
            timing: Timing::Uniform {
                frames,
                rate: if rate > 0.0 { rate } else { 1.0 },
            },
        }
    }

    /// Number of frames.
    pub fn frame_count(&self) -> usize {
        match &self.timing {
            Timing::Track(track) => track.frame_count(),
            Timing::Uniform { frames, .. } => *frames,
        }
    }

    /// Duration in seconds.
    pub fn duration_seconds(&self) -> f64 {
        match &self.timing {
            Timing::Track(track) => track.duration_seconds(),
            Timing::Uniform { frames, rate } => *frames as f64 / rate,
        }
    }

    /// Average frames per second.
    pub fn frame_rate(&self) -> f64 {
        match &self.timing {
            Timing::Track(track) => track.frame_rate(),
            Timing::Uniform { rate, .. } => *rate,
        }
    }

    /// Index of the frame shown at `seconds`, clamped to the media.
    pub fn frame_at(&self, seconds: f64) -> usize {
        match &self.timing {
            Timing::Track(track) => track.frame_at(seconds),
            Timing::Uniform { frames, rate } => {
                let index = (seconds.max(0.0) * rate) as usize;
                index.min(frames.saturating_sub(1))
            }
        }
    }
}

/// A media that decodes frames by index, in any order.
pub trait FrameSource: Send {
    /// Size and timing.
    fn info(&self) -> &MediaInfo;

    /// Decodes frame `index` into `frame`, reusing its buffers when it can.
    fn decode(&mut self, index: usize, frame: &mut Frame) -> Result<(), DecodeError>;
}

impl FrameSource for HapDecoder {
    fn info(&self) -> &MediaInfo {
        self.media_info()
    }

    fn decode(&mut self, index: usize, frame: &mut Frame) -> Result<(), DecodeError> {
        self.decode_frame(index, frame.hap_mut())
    }
}

/// Opens a media file: a HAP movie, or a still image (PNG, JPEG).
pub fn open_file(path: &Path) -> Result<Box<dyn FrameSource>, DecodeError> {
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    if IMAGE_EXTENSIONS.contains(&extension.as_str()) {
        return Ok(Box::new(StillImage::open(path)?));
    }
    Ok(Box::new(HapDecoder::open(path)?))
}

/// Opens a folder of PNG images, played in name order at `rate` frames per
/// second.
pub fn open_sequence(folder: &Path, rate: f64) -> Result<Box<dyn FrameSource>, DecodeError> {
    Ok(Box::new(ImageSequence::open(folder, rate)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uniform_timing() {
        let info = MediaInfo::uniform("PNG", 4, 4, 10, 25.0);
        assert_eq!(info.frame_count(), 10);
        assert!((info.duration_seconds() - 0.4).abs() < 1e-12);
        assert_eq!(info.frame_at(0.0), 0);
        assert_eq!(info.frame_at(0.039), 0);
        assert_eq!(info.frame_at(0.04), 1);
        assert_eq!(info.frame_at(100.0), 9);
        assert_eq!(MediaInfo::uniform("PNG", 1, 1, 1, 0.0).frame_rate(), 1.0);
    }

    #[test]
    fn frames_change_kind_on_demand() {
        let mut frame = Frame::default();
        frame.rgba_mut().width = 3;
        assert!(matches!(&frame, Frame::Rgba(image) if image.width == 3));
        frame.hap_mut();
        assert!(matches!(frame, Frame::Hap(_)));
    }
}
