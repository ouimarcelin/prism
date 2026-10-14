//! Media of any supported kind, behind one interface: open a file or folder,
//! read its size and timing, decode any frame.

use std::path::Path;

use std::fs::File;

use crate::decoder::{DecodeError, HapDecoder, is_hap};
use crate::hap::HapFrame;
use crate::image::{IMAGE_EXTENSIONS, ImageSequence, StillImage};
use crate::mov::VideoTrack;

/// Order of the four bytes of a pixel.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ChannelOrder {
    /// Red, green, blue, alpha.
    #[default]
    Rgba,
    /// Blue, green, red, alpha, as video decoders often deliver.
    Bgra,
}

/// 8-bit pixels with alpha, straight (non-premultiplied), rows from the top.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RgbaImage {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Order of the bytes of each pixel.
    pub order: ChannelOrder,
    /// `width × height × 4` bytes.
    pub pixels: Vec<u8>,
}

/// Matrix turning luma and chroma into red, green and blue.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum YuvMatrix {
    /// ITU-R BT.601, standard definition.
    Bt601,
    /// ITU-R BT.709, high definition.
    #[default]
    Bt709,
}

/// Video pixels as decoders deliver them, 4:2:0: one luma byte per pixel,
/// then one interleaved pair of chroma bytes (Cb, Cr) per 2 × 2 pixels.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct YuvImage {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// `width × height` luma bytes.
    pub luma: Vec<u8>,
    /// `⌈width / 2⌉ × ⌈height / 2⌉` pairs of chroma bytes.
    pub chroma: Vec<u8>,
    /// Color matrix.
    pub matrix: YuvMatrix,
    /// Whether values use the full 0–255 range rather than the video range
    /// (16–235 for luma).
    pub full_range: bool,
}

impl YuvImage {
    /// Size of the chroma plane, in chroma samples.
    pub fn chroma_size(&self) -> (u32, u32) {
        (self.width.div_ceil(2), self.height.div_ceil(2))
    }

    /// Color of a pixel, computed on the CPU as the GPU does. For tests and
    /// thumbnails.
    pub fn rgb_at(&self, x: u32, y: u32) -> [u8; 3] {
        let luma = f32::from(self.luma[(y * self.width + x) as usize]) / 255.0;
        let (chroma_width, _) = self.chroma_size();
        let at = ((y / 2 * chroma_width + x / 2) * 2) as usize;
        let cb = f32::from(self.chroma[at]) / 255.0 - 128.0 / 255.0;
        let cr = f32::from(self.chroma[at + 1]) / 255.0 - 128.0 / 255.0;
        let (luma, cb, cr) = if self.full_range {
            (luma, cb, cr)
        } else {
            (
                (luma - 16.0 / 255.0) * (255.0 / 219.0),
                cb * (255.0 / 224.0),
                cr * (255.0 / 224.0),
            )
        };
        let [r, g, b] = match self.matrix {
            YuvMatrix::Bt601 => [
                luma + 1.402 * cr,
                luma - 0.344_136 * cb - 0.714_136 * cr,
                luma + 1.772 * cb,
            ],
            YuvMatrix::Bt709 => [
                luma + 1.5748 * cr,
                luma - 0.187_324 * cb - 0.468_124 * cr,
                luma + 1.8556 * cb,
            ],
        };
        [r, g, b].map(|channel| (channel.clamp(0.0, 1.0) * 255.0).round() as u8)
    }
}

/// A decoded frame, ready to be uploaded to the GPU.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Frame {
    /// Block-compressed planes of a HAP frame.
    Hap(HapFrame),
    /// Uncompressed pixels.
    Rgba(RgbaImage),
    /// Luma and chroma planes, as video decoders deliver them.
    Yuv(YuvImage),
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
            _ => unreachable!("replaced just above"),
        }
    }

    /// The pixel buffer, turning the frame into an RGBA frame if needed.
    pub fn rgba_mut(&mut self) -> &mut RgbaImage {
        if !matches!(self, Self::Rgba(_)) {
            *self = Self::Rgba(RgbaImage::default());
        }
        match self {
            Self::Rgba(image) => image,
            _ => unreachable!("replaced just above"),
        }
    }

    /// The luma and chroma planes, turning the frame into a YUV frame if
    /// needed.
    pub fn yuv_mut(&mut self) -> &mut YuvImage {
        if !matches!(self, Self::Yuv(_)) {
            *self = Self::Yuv(YuvImage::default());
        }
        match self {
            Self::Yuv(image) => image,
            _ => unreachable!("replaced just above"),
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
///
/// A source stays on the thread that opened it: platform decoders are not
/// required to move between threads.
pub trait FrameSource {
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

/// Opens a media file: a still image (PNG, JPEG), a HAP movie, or on macOS
/// a movie in any codec AVFoundation decodes (H.264, ProRes…).
pub fn open_file(path: &Path) -> Result<Box<dyn FrameSource>, DecodeError> {
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    if IMAGE_EXTENSIONS.contains(&extension.as_str()) {
        return Ok(Box::new(StillImage::open(path)?));
    }
    let mut file = File::open(path).map_err(DecodeError::Open)?;
    let track = VideoTrack::read(&mut file)?;
    if is_hap(track.codec) {
        return Ok(Box::new(HapDecoder::new(file)?));
    }
    open_platform(path, &track)
}

#[cfg(target_os = "macos")]
fn open_platform(path: &Path, track: &VideoTrack) -> Result<Box<dyn FrameSource>, DecodeError> {
    Ok(Box::new(crate::avf::AvfSource::open(
        path,
        track.codec_name(),
        track.frame_count(),
        track.duration_seconds(),
    )?))
}

#[cfg(not(target_os = "macos"))]
fn open_platform(_path: &Path, track: &VideoTrack) -> Result<Box<dyn FrameSource>, DecodeError> {
    Err(DecodeError::UnsupportedCodec(track.codec_name()))
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
    fn yuv_reference_colors() {
        let gray = YuvImage {
            width: 2,
            height: 2,
            luma: vec![126; 4],
            chroma: vec![128, 128],
            ..YuvImage::default()
        };
        assert_eq!(gray.rgb_at(1, 1), [128, 128, 128]);
        // Video-range red in BT.709.
        let red = YuvImage {
            width: 1,
            height: 1,
            luma: vec![63],
            chroma: vec![102, 240],
            ..YuvImage::default()
        };
        let [r, g, b] = red.rgb_at(0, 0);
        assert!(r > 240 && g < 20 && b < 20, "{:?}", [r, g, b]);
        let full = YuvImage {
            full_range: true,
            matrix: YuvMatrix::Bt601,
            luma: vec![255],
            chroma: vec![128, 128],
            width: 1,
            height: 1,
        };
        assert_eq!(full.rgb_at(0, 0), [255, 255, 255]);
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
