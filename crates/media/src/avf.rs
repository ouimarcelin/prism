//! H.264, ProRes and the other codecs of macOS, decoded by AVFoundation.
//!
//! An `AVAssetReader` decodes forward from a start time. Frames close ahead
//! of the last one are reached by reading on; any other frame restarts the
//! reader at that frame. Long-GOP codecs such as H.264 therefore play
//! forward smoothly, but scratching and backward play cost one restart per
//! frame: HAP remains the format for those.

// Calls into Apple frameworks. Every block states why it is sound.
#![allow(unsafe_code)]

use std::path::Path;

use objc2::rc::{Retained, autoreleasepool};
use objc2::runtime::AnyObject;
use objc2_av_foundation::{
    AVAssetReader, AVAssetReaderTrackOutput, AVAssetTrack, AVMediaTypeVideo, AVURLAsset,
};
use objc2_core_foundation::CFString;
use objc2_core_media::{CMTime, CMTimeRange, kCMTimePositiveInfinity};
use objc2_core_video::{
    CVPixelBuffer, CVPixelBufferGetBaseAddress, CVPixelBufferGetBaseAddressOfPlane,
    CVPixelBufferGetBytesPerRow, CVPixelBufferGetBytesPerRowOfPlane, CVPixelBufferGetHeight,
    CVPixelBufferGetHeightOfPlane, CVPixelBufferGetPixelFormatType, CVPixelBufferGetWidth,
    CVPixelBufferLockBaseAddress, CVPixelBufferLockFlags, CVPixelBufferUnlockBaseAddress,
    kCVImageBufferYCbCrMatrix_ITU_R_601_4, kCVImageBufferYCbCrMatrixKey,
    kCVPixelBufferPixelFormatTypeKey, kCVPixelFormatType_32BGRA,
    kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange, kCVReturnSuccess,
};
use objc2_foundation::{NSDictionary, NSError, NSNumber, NSString, NSURL};

use crate::decoder::DecodeError;
use crate::source::{ChannelOrder, Frame, FrameSource, MediaInfo, RgbaImage, YuvImage, YuvMatrix};

/// Farthest frame reached by reading on; beyond, the reader restarts.
const MAX_READ_AHEAD: usize = 45;

/// Time units per second for start times.
const TIMESCALE: i32 = 90_000;

fn platform_error(context: &str, error: Option<Retained<NSError>>) -> DecodeError {
    let detail = error.map_or_else(String::new, |error| {
        format!(" : {}", error.localizedDescription())
    });
    DecodeError::Platform(format!("{context}{detail}"))
}

/// Codecs whose frames may carry alpha: decoded as BGRA. Others are
/// decoded to 4:2:0 luma and chroma, the decoders' own output, which is
/// lighter to copy and converted to color on the GPU.
const ALPHA_CODECS: [&str; 2] = ["ap4h", "ap4x"];

/// A movie decoded by AVFoundation, frame by frame.
pub struct AvfSource {
    info: MediaInfo,
    pixel_format: u32,
    asset: Retained<AVURLAsset>,
    track: Retained<AVAssetTrack>,
    reader: Option<(Retained<AVAssetReader>, Retained<AVAssetReaderTrackOutput>)>,
    /// Index of the next frame the reader returns.
    next: usize,
}

impl AvfSource {
    /// Opens the video track of a movie; `frames` and `duration` come from
    /// the container, `format` names the codec.
    pub fn open(
        path: &Path,
        format: String,
        frames: usize,
        duration: f64,
    ) -> Result<Self, DecodeError> {
        autoreleasepool(|_| {
            let url = NSURL::from_file_path(path).ok_or_else(|| {
                DecodeError::Platform(format!("chemin invalide : {}", path.display()))
            })?;
            // SAFETY: plain constructor; no options dictionary is passed.
            let asset = unsafe { AVURLAsset::URLAssetWithURL_options(&url, None) };
            // SAFETY: reading an immutable framework constant.
            let video = unsafe { AVMediaTypeVideo }
                .ok_or_else(|| DecodeError::Platform("type de média vidéo indisponible".into()))?;
            // SAFETY: synchronous track list of a local file asset; the
            // asynchronous variant brings nothing on a worker thread.
            #[allow(deprecated)]
            let tracks = unsafe { asset.tracksWithMediaType(video) };
            let track = tracks
                .firstObject()
                .ok_or(DecodeError::Container(crate::ContainerError::NoVideoTrack))?;
            // SAFETY: property getter of a valid track.
            let size = unsafe { track.naturalSize() };
            let rate = if duration > 0.0 && frames > 0 {
                frames as f64 / duration
            } else {
                // SAFETY: property getter of a valid track.
                f64::from(unsafe { track.nominalFrameRate() })
            };
            let pixel_format = if ALPHA_CODECS.contains(&format.as_str()) {
                kCVPixelFormatType_32BGRA
            } else {
                kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange
            };
            Ok(Self {
                pixel_format,
                info: MediaInfo::uniform(
                    format,
                    size.width.round() as u32,
                    size.height.round() as u32,
                    frames,
                    rate,
                ),
                asset,
                track,
                reader: None,
                next: 0,
            })
        })
    }

    /// Starts a reader returning frames from `index` on, in the pixel format
    /// of this source.
    fn start(&mut self, index: usize) -> Result<(), DecodeError> {
        if let Some((reader, _)) = self.reader.take() {
            // SAFETY: stops a reader this source owns.
            unsafe { reader.cancelReading() };
        }
        // Half a frame early, so rounding never skips the wanted frame.
        let seconds = ((index as f64 - 0.5) / self.info.frame_rate()).max(0.0);
        // SAFETY: plain value constructors.
        let range = unsafe {
            CMTimeRange::new(
                CMTime::with_seconds(seconds, TIMESCALE),
                kCMTimePositiveInfinity,
            )
        };
        // SAFETY: the asset is valid for the life of this source.
        let reader = unsafe { AVAssetReader::assetReaderWithAsset_error(&self.asset) }
            .map_err(|error| platform_error("lecteur AVFoundation impossible", Some(error)))?;

        // SAFETY: CFString and NSString are toll-free bridged: the same
        // object may be used as either type.
        let key: &NSString = unsafe {
            &*std::ptr::from_ref::<CFString>(kCVPixelBufferPixelFormatTypeKey).cast::<NSString>()
        };
        let format = NSNumber::numberWithUnsignedInt(self.pixel_format);
        let value: &AnyObject = &format;
        let settings = NSDictionary::from_slices(&[key], &[value]);
        // SAFETY: the settings dictionary maps a pixel format key to a
        // number, as AVFoundation documents.
        let output = unsafe {
            AVAssetReaderTrackOutput::assetReaderTrackOutputWithTrack_outputSettings(
                &self.track,
                Some(&settings),
            )
        };
        // SAFETY: setters and start of a fresh reader, before reading.
        let started = unsafe {
            output.setAlwaysCopiesSampleData(false);
            reader.setTimeRange(range);
            reader.addOutput(&output);
            reader.startReading()
        };
        if !started {
            // SAFETY: property getter of a valid reader.
            return Err(platform_error("lecture AVFoundation impossible", unsafe {
                reader.error()
            }));
        }
        self.reader = Some((reader, output));
        self.next = index;
        Ok(())
    }
}

impl FrameSource for AvfSource {
    fn info(&self) -> &MediaInfo {
        &self.info
    }

    fn decode(&mut self, index: usize, frame: &mut Frame) -> Result<(), DecodeError> {
        let count = self.info.frame_count();
        if index >= count {
            return Err(DecodeError::FrameOutOfRange { index, count });
        }
        autoreleasepool(|_| {
            if self.reader.is_none() || index < self.next || index > self.next + MAX_READ_AHEAD {
                self.start(index)?;
            }
            match self.read_until(index, frame) {
                // A reader can fail while the system decoder starts up:
                // one more try from a fresh reader.
                Err(_) => {
                    self.start(index)?;
                    self.read_until(index, frame)
                }
                done => done,
            }
        })
    }
}

impl AvfSource {
    /// Reads on until frame `index`, and copies it into `frame`.
    fn read_until(&mut self, index: usize, frame: &mut Frame) -> Result<(), DecodeError> {
        let rate = self.info.frame_rate();
        loop {
            let Some((reader, output)) = &self.reader else {
                return Err(DecodeError::Platform("lecteur arrêté".into()));
            };
            // SAFETY: the output belongs to a started reader.
            let Some(sample) = (unsafe { output.copyNextSampleBuffer() }) else {
                // SAFETY: property getter of a valid reader.
                let error = unsafe { reader.error() };
                self.reader = None;
                return Err(platform_error("fin du média avant l'image demandée", error));
            };
            // SAFETY: getters of a valid sample buffer.
            let seconds = unsafe { sample.presentation_time_stamp().seconds() };
            let sample_index = (seconds * rate).round().max(0.0) as usize;
            self.next = sample_index + 1;
            if sample_index < index {
                continue;
            }
            // SAFETY: getter of a valid sample buffer.
            let Some(buffer) = (unsafe { sample.image_buffer() }) else {
                continue;
            };
            return if self.pixel_format == kCVPixelFormatType_32BGRA {
                copy_pixels(&buffer, frame.rgba_mut())
            } else {
                copy_planes(&buffer, frame.yuv_mut())
            };
        }
    }
}

/// Copies a locked 32-bit BGRA pixel buffer into `image`, row by row.
fn copy_pixels(buffer: &CVPixelBuffer, image: &mut RgbaImage) -> Result<(), DecodeError> {
    if CVPixelBufferGetPixelFormatType(buffer) != kCVPixelFormatType_32BGRA {
        return Err(DecodeError::Platform("format de pixels inattendu".into()));
    }
    let width = CVPixelBufferGetWidth(buffer);
    let height = CVPixelBufferGetHeight(buffer);
    let stride = CVPixelBufferGetBytesPerRow(buffer);
    // SAFETY: the buffer is valid; it is unlocked below on every path.
    if unsafe { CVPixelBufferLockBaseAddress(buffer, CVPixelBufferLockFlags::ReadOnly) }
        != kCVReturnSuccess
    {
        return Err(DecodeError::Platform("pixels inaccessibles".into()));
    }
    let base = CVPixelBufferGetBaseAddress(buffer)
        .cast::<u8>()
        .cast_const();
    let result = if base.is_null() || stride < width * 4 {
        Err(DecodeError::Platform("pixels inaccessibles".into()))
    } else {
        image.width = width as u32;
        image.height = height as u32;
        image.order = ChannelOrder::Bgra;
        image.pixels.clear();
        image.pixels.reserve(width * height * 4);
        for row in 0..height {
            // SAFETY: while locked, the base address points to `height`
            // rows of `stride` bytes, each starting with `width` pixels.
            let pixels = unsafe { std::slice::from_raw_parts(base.add(row * stride), width * 4) };
            image.pixels.extend_from_slice(pixels);
        }
        Ok(())
    };
    // SAFETY: balances the lock above.
    unsafe { CVPixelBufferUnlockBaseAddress(buffer, CVPixelBufferLockFlags::ReadOnly) };
    result
}

/// Copies a locked 4:2:0 video-range pixel buffer (luma plane, then
/// interleaved chroma plane) into `image`.
fn copy_planes(buffer: &CVPixelBuffer, image: &mut YuvImage) -> Result<(), DecodeError> {
    if CVPixelBufferGetPixelFormatType(buffer) != kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange {
        return Err(DecodeError::Platform("format de pixels inattendu".into()));
    }
    let width = CVPixelBufferGetWidth(buffer);
    let height = CVPixelBufferGetHeight(buffer);
    image.width = width as u32;
    image.height = height as u32;
    image.full_range = false;
    image.matrix = matrix(buffer, height);
    let (chroma_width, chroma_height) = image.chroma_size();
    let (chroma_width, chroma_height) = (chroma_width as usize, chroma_height as usize);

    // SAFETY: the buffer is valid; it is unlocked below on every path.
    if unsafe { CVPixelBufferLockBaseAddress(buffer, CVPixelBufferLockFlags::ReadOnly) }
        != kCVReturnSuccess
    {
        return Err(DecodeError::Platform("pixels inaccessibles".into()));
    }
    let planes = [(0, width, height), (1, chroma_width * 2, chroma_height)];
    let mut result = Ok(());
    for (plane, row_bytes, rows) in planes {
        let base = CVPixelBufferGetBaseAddressOfPlane(buffer, plane)
            .cast::<u8>()
            .cast_const();
        let stride = CVPixelBufferGetBytesPerRowOfPlane(buffer, plane);
        if base.is_null()
            || stride < row_bytes
            || CVPixelBufferGetHeightOfPlane(buffer, plane) < rows
        {
            result = Err(DecodeError::Platform(
                "plans de pixels inaccessibles".into(),
            ));
            break;
        }
        let target = if plane == 0 {
            &mut image.luma
        } else {
            &mut image.chroma
        };
        target.clear();
        target.reserve(row_bytes * rows);
        for row in 0..rows {
            // SAFETY: while locked, the plane holds `rows` rows of `stride`
            // bytes, each starting with `row_bytes` bytes of samples.
            let samples = unsafe { std::slice::from_raw_parts(base.add(row * stride), row_bytes) };
            target.extend_from_slice(samples);
        }
    }
    // SAFETY: balances the lock above.
    unsafe { CVPixelBufferUnlockBaseAddress(buffer, CVPixelBufferLockFlags::ReadOnly) };
    result
}

/// Color matrix tagged on the buffer; by default BT.601 below 720 lines and
/// BT.709 from 720 lines up.
fn matrix(buffer: &CVPixelBuffer, height: usize) -> YuvMatrix {
    // SAFETY: a null attachment mode pointer is allowed.
    let tagged = unsafe { buffer.attachment(kCVImageBufferYCbCrMatrixKey, std::ptr::null_mut()) };
    let tagged = tagged
        .as_deref()
        .and_then(|value| value.downcast_ref::<CFString>())
        .map(ToString::to_string);
    // SAFETY: reading an immutable framework constant.
    let bt601 = unsafe { kCVImageBufferYCbCrMatrix_ITU_R_601_4 }.to_string();
    match tagged {
        Some(name) if name == bt601 => YuvMatrix::Bt601,
        Some(_) => YuvMatrix::Bt709,
        None if height < 720 => YuvMatrix::Bt601,
        None => YuvMatrix::Bt709,
    }
}
