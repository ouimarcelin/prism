//! Media Foundation (Windows): movies in the codecs of the system (H.264,
//! HEVC…), their sound, and cameras and capture cards (ADR 0038).
//!
//! A source reader decodes and converts: video to 32-bit BGRX, sound to
//! 16-bit stereo PCM at [`AUDIO_RATE`]. Not tried on a Windows PC yet.

// Calls into Media Foundation and COM. Every block states why it is sound.
#![allow(unsafe_code)]

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::thread::JoinHandle;

use windows::Win32::Media::MediaFoundation::{
    IMFActivate, IMFAttributes, IMFMediaSource, IMFSample, IMFSourceReader,
    MF_DEVSOURCE_ATTRIBUTE_FRIENDLY_NAME, MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE,
    MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID,
    MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK, MF_MT_AUDIO_BITS_PER_SAMPLE,
    MF_MT_AUDIO_NUM_CHANNELS, MF_MT_AUDIO_SAMPLES_PER_SECOND, MF_MT_DEFAULT_STRIDE,
    MF_MT_FRAME_SIZE, MF_MT_MAJOR_TYPE, MF_MT_SUBTYPE, MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS,
    MF_SOURCE_READER_ALL_STREAMS, MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING,
    MF_SOURCE_READER_FIRST_AUDIO_STREAM, MF_SOURCE_READER_FIRST_VIDEO_STREAM,
    MF_SOURCE_READERF_ENDOFSTREAM, MF_VERSION, MFAudioFormat_PCM, MFCreateAttributes,
    MFCreateMediaType, MFCreateSourceReaderFromMediaSource, MFCreateSourceReaderFromURL,
    MFEnumDeviceSources, MFMediaType_Audio, MFMediaType_Video, MFSTARTUP_FULL, MFStartup,
    MFVideoFormat_RGB32,
};
use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoTaskMemFree};
use windows::Win32::System::Variant::VT_I8;
use windows::core::{GUID, HSTRING, PCWSTR, PWSTR};

use crate::source::{ChannelOrder, Frame, FrameSource, MediaInfo, RgbaImage};
use crate::{AUDIO_RATE, ClipAudio, DecodeError};

/// Frames read past the one asked for before seeking is cheaper.
const MAX_READ_AHEAD: usize = 30;

fn error(context: &str, error: windows::core::Error) -> DecodeError {
    DecodeError::Platform(format!("{context} : {error}"))
}

/// Starts COM on this thread and Media Foundation once per process.
fn startup() -> Result<(), DecodeError> {
    thread_local! {
        static COM: () = {
            // SAFETY: joins the multithreaded apartment; called once per
            // thread. A thread already in an apartment keeps it.
            let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        };
    }
    COM.with(|()| {});
    static STARTED: OnceLock<Result<(), String>> = OnceLock::new();
    STARTED
        .get_or_init(|| {
            // SAFETY: starts Media Foundation for the process; never stopped.
            unsafe { MFStartup(MF_VERSION, MFSTARTUP_FULL) }.map_err(|error| error.to_string())
        })
        .clone()
        .map_err(|message| DecodeError::Platform(format!("Media Foundation : {message}")))
}

/// Attributes asking the source reader to convert formats.
fn reader_attributes() -> Result<IMFAttributes, DecodeError> {
    let mut attributes = None;
    // SAFETY: creates an attribute store; the out pointer lives for the call.
    unsafe { MFCreateAttributes(&mut attributes, 2) }.map_err(|e| error("attributs", e))?;
    let attributes =
        attributes.ok_or_else(|| DecodeError::Platform("attributs indisponibles".into()))?;
    // SAFETY: sets two flags of a live store.
    unsafe {
        attributes
            .SetUINT32(&MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING, 1)
            .map_err(|e| error("conversion vidéo", e))?;
        attributes
            .SetUINT32(&MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, 1)
            .map_err(|e| error("décodage matériel", e))?;
    }
    Ok(attributes)
}

/// Asks the first video stream of `reader` for 32-bit BGRX; returns the
/// size and the stride of the rows (negative: bottom row first).
fn video_output(reader: &IMFSourceReader) -> Result<(u32, u32, i32), DecodeError> {
    let stream = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;
    // SAFETY: calls on live objects; GUID pointers point to constants.
    unsafe {
        let wanted = MFCreateMediaType().map_err(|e| error("type de sortie", e))?;
        wanted
            .SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)
            .map_err(|e| error("type de sortie", e))?;
        wanted
            .SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32)
            .map_err(|e| error("type de sortie", e))?;
        reader
            .SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS.0 as u32, false)
            .map_err(|e| error("choix des pistes", e))?;
        reader
            .SetStreamSelection(stream, true)
            .map_err(|e| error("piste vidéo absente", e))?;
        reader
            .SetCurrentMediaType(stream, None, &wanted)
            .map_err(|e| error("conversion en BGRX refusée", e))?;
        let actual = reader
            .GetCurrentMediaType(stream)
            .map_err(|e| error("type de sortie", e))?;
        let size = actual
            .GetUINT64(&MF_MT_FRAME_SIZE)
            .map_err(|e| error("taille d'image", e))?;
        let (width, height) = ((size >> 32) as u32, size as u32);
        let stride = actual
            .GetUINT32(&MF_MT_DEFAULT_STRIDE)
            .map_or(width as i32 * 4, |stride| stride as i32);
        Ok((width, height, stride))
    }
}

/// Copies a BGRX sample into `image`, rows top first, alpha opaque.
fn copy_sample(
    sample: &IMFSample,
    width: u32,
    height: u32,
    stride: i32,
    image: &mut RgbaImage,
) -> Result<(), DecodeError> {
    // SAFETY: a live sample; the buffer is locked while read and holds
    // `height` rows of `|stride|` bytes.
    unsafe {
        let buffer = sample
            .ConvertToContiguousBuffer()
            .map_err(|e| error("image", e))?;
        let mut data = std::ptr::null_mut();
        let mut length = 0;
        buffer
            .Lock(&mut data, None, Some(&mut length))
            .map_err(|e| error("image", e))?;
        let pitch = stride.unsigned_abs() as usize;
        let row = width as usize * 4;
        if pitch < row || (length as usize) < pitch * height as usize {
            let _ = buffer.Unlock();
            return Err(DecodeError::Platform("image plus petite que prévu".into()));
        }
        let bytes = std::slice::from_raw_parts(data, length as usize);
        image.width = width;
        image.height = height;
        image.order = ChannelOrder::Bgra;
        image.pixels.clear();
        image.pixels.reserve(row * height as usize);
        for line in 0..height as usize {
            let source = if stride < 0 {
                height as usize - 1 - line
            } else {
                line
            };
            let start = source * pitch;
            image.pixels.extend_from_slice(&bytes[start..start + row]);
        }
        let _ = buffer.Unlock();
    }
    for pixel in image.pixels.as_chunks_mut::<4>().0 {
        pixel[3] = 255;
    }
    Ok(())
}

/// A movie decoded by Media Foundation.
pub(crate) struct MfSource {
    reader: IMFSourceReader,
    info: MediaInfo,
    stride: i32,
    /// Index of the frame the next read returns.
    next: usize,
}

// SAFETY: the reader lives in the multithreaded apartment and is used by
// one thread at a time (the decoding thread owning the source).
unsafe impl Send for MfSource {}

impl MfSource {
    /// Opens the video of the movie at `path`; `frames` and `duration` come
    /// from the container, `format` names the codec.
    pub(crate) fn open(
        path: &Path,
        format: String,
        frames: usize,
        duration: f64,
    ) -> Result<Self, DecodeError> {
        startup()?;
        let attributes = reader_attributes()?;
        let url = HSTRING::from(path.as_os_str());
        // SAFETY: the path is a valid wide string for the call.
        let reader = unsafe { MFCreateSourceReaderFromURL(PCWSTR(url.as_ptr()), &attributes) }
            .map_err(|e| error(&format!("ouverture de {}", path.display()), e))?;
        let (width, height, stride) = video_output(&reader)?;
        let rate = if duration > 0.0 && frames > 0 {
            frames as f64 / duration
        } else {
            30.0
        };
        Ok(Self {
            reader,
            info: MediaInfo::uniform(format, width, height, frames, rate),
            stride,
            next: 0,
        })
    }

    /// Moves the reader just before frame `index`.
    fn seek(&mut self, index: usize) -> Result<(), DecodeError> {
        let seconds = ((index as f64 - 0.5) / self.info.frame_rate()).max(0.0);
        let mut position = PROPVARIANT::default();
        // SAFETY: writes the type and the 64-bit value of a zeroed variant;
        // a VT_I8 variant owns nothing to release.
        unsafe {
            let inner = &mut *position.Anonymous.Anonymous;
            inner.vt = VT_I8;
            inner.Anonymous.hVal = (seconds * 1e7) as i64;
            self.reader
                .SetCurrentPosition(&GUID::zeroed(), &position)
                .map_err(|e| error("déplacement", e))?;
        }
        self.next = index.saturating_sub(1);
        Ok(())
    }

    /// Reads on until frame `index`, and copies it into `frame`.
    fn read_until(&mut self, index: usize, frame: &mut Frame) -> Result<(), DecodeError> {
        let rate = self.info.frame_rate();
        loop {
            let mut flags = 0;
            let mut time = 0i64;
            let mut sample = None;
            // SAFETY: synchronous read; the out pointers live for the call.
            unsafe {
                self.reader
                    .ReadSample(
                        MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32,
                        0,
                        None,
                        Some(&mut flags),
                        Some(&mut time),
                        Some(&mut sample),
                    )
                    .map_err(|e| error("lecture", e))?;
            }
            if flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
                return Err(DecodeError::Platform(
                    "fin du média avant l'image demandée".into(),
                ));
            }
            let Some(sample) = sample else {
                continue;
            };
            let sample_index = (time as f64 / 1e7 * rate).round().max(0.0) as usize;
            self.next = sample_index + 1;
            if sample_index < index {
                continue;
            }
            let (width, height) = (self.info.width, self.info.height);
            return copy_sample(&sample, width, height, self.stride, frame.rgba_mut());
        }
    }
}

impl FrameSource for MfSource {
    fn info(&self) -> &MediaInfo {
        &self.info
    }

    fn decode(&mut self, index: usize, frame: &mut Frame) -> Result<(), DecodeError> {
        let count = self.info.frame_count();
        if index >= count {
            return Err(DecodeError::FrameOutOfRange { index, count });
        }
        if index < self.next || index > self.next + MAX_READ_AHEAD {
            self.seek(index)?;
        }
        self.read_until(index, frame)
    }
}

/// The sound of the movie at `path`, 16-bit stereo at [`AUDIO_RATE`];
/// `None` without a sound track.
pub(crate) fn decode_audio(path: &Path) -> Result<Option<ClipAudio>, DecodeError> {
    startup()?;
    let url = HSTRING::from(path.as_os_str());
    // SAFETY: the path is a valid wide string for the call.
    let reader = unsafe { MFCreateSourceReaderFromURL(PCWSTR(url.as_ptr()), None) }
        .map_err(|e| error(&format!("ouverture de {}", path.display()), e))?;
    let stream = MF_SOURCE_READER_FIRST_AUDIO_STREAM.0 as u32;
    // SAFETY: calls on live objects; GUID pointers point to constants.
    unsafe {
        if reader
            .SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS.0 as u32, false)
            .is_err()
            || reader.SetStreamSelection(stream, true).is_err()
        {
            return Ok(None);
        }
        let wanted = MFCreateMediaType().map_err(|e| error("type audio", e))?;
        wanted
            .SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)
            .map_err(|e| error("type audio", e))?;
        wanted
            .SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_PCM)
            .map_err(|e| error("type audio", e))?;
        wanted
            .SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, 2)
            .map_err(|e| error("type audio", e))?;
        wanted
            .SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, AUDIO_RATE)
            .map_err(|e| error("type audio", e))?;
        wanted
            .SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16)
            .map_err(|e| error("type audio", e))?;
        if reader.SetCurrentMediaType(stream, None, &wanted).is_err() {
            return Ok(None);
        }
    }
    let mut samples = Vec::new();
    loop {
        let mut flags = 0;
        let mut sample = None;
        // SAFETY: synchronous read; the out pointers live for the call.
        unsafe {
            reader
                .ReadSample(stream, 0, None, Some(&mut flags), None, Some(&mut sample))
                .map_err(|e| error("lecture du son", e))?;
        }
        if let Some(sample) = sample {
            // SAFETY: a live sample; the buffer is locked while read.
            unsafe {
                let buffer = sample
                    .ConvertToContiguousBuffer()
                    .map_err(|e| error("son", e))?;
                let mut data = std::ptr::null_mut();
                let mut length = 0;
                buffer
                    .Lock(&mut data, None, Some(&mut length))
                    .map_err(|e| error("son", e))?;
                let bytes = std::slice::from_raw_parts(data, length as usize);
                samples.extend(
                    bytes
                        .as_chunks::<2>()
                        .0
                        .iter()
                        .map(|pair| i16::from_le_bytes(*pair)),
                );
                let _ = buffer.Unlock();
            }
        }
        if flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
            break;
        }
    }
    Ok((!samples.is_empty()).then_some(ClipAudio { samples }))
}

/// A camera or capture card, as Media Foundation lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CameraDevice {
    /// Identifier: the symbolic link of the device.
    pub id: String,
    /// Name shown to the user.
    pub name: String,
}

/// Reads a string attribute of a device.
fn device_string(activate: &IMFActivate, key: &GUID) -> Option<String> {
    let mut text = PWSTR::null();
    let mut length = 0;
    // SAFETY: the attribute is allocated by Media Foundation and freed
    // here once copied.
    unsafe {
        activate
            .GetAllocatedString(key, &mut text, &mut length)
            .ok()?;
        let value = text.to_string().ok();
        CoTaskMemFree(Some(text.0.cast()));
        value
    }
}

/// The video capture devices, created for `each`.
fn devices<T>(mut each: impl FnMut(&IMFActivate, CameraDevice) -> Option<T>) -> Vec<T> {
    if startup().is_err() {
        return Vec::new();
    }
    let mut found = Vec::new();
    let mut attributes = None;
    // SAFETY: creates an attribute store and lists the devices it selects;
    // the array is freed after each device is released.
    unsafe {
        if MFCreateAttributes(&mut attributes, 1).is_err() {
            return found;
        }
        let Some(attributes) = attributes else {
            return found;
        };
        if attributes
            .SetGUID(
                &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE,
                &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID,
            )
            .is_err()
        {
            return found;
        }
        let mut list: *mut Option<IMFActivate> = std::ptr::null_mut();
        let mut count = 0u32;
        if MFEnumDeviceSources(&attributes, &mut list, &mut count).is_err() || list.is_null() {
            return found;
        }
        for index in 0..count as usize {
            let slot = &mut *list.add(index);
            if let Some(activate) = slot.take() {
                let id = device_string(
                    &activate,
                    &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK,
                );
                let name = device_string(&activate, &MF_DEVSOURCE_ATTRIBUTE_FRIENDLY_NAME);
                if let (Some(id), Some(name)) = (id, name)
                    && let Some(item) = each(&activate, CameraDevice { id, name })
                {
                    found.push(item);
                }
            }
        }
        CoTaskMemFree(Some(list.cast()));
    }
    found
}

/// The cameras and capture cards connected now.
pub fn cameras() -> Vec<CameraDevice> {
    devices(|_, device| Some(device))
}

/// Frames shared between the capture thread and the engine.
#[derive(Default)]
struct Shared {
    latest: Mutex<Option<Frame>>,
    spare: Mutex<Option<Frame>>,
}

/// A camera capturing on a thread of its own. Capture stops when it is
/// dropped.
pub struct Camera {
    shared: Arc<Shared>,
    running: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Camera {
    /// Starts capturing the camera of identifier `id`.
    pub fn open(id: &str) -> Result<Self, DecodeError> {
        let shared = Arc::new(Shared::default());
        let running = Arc::new(AtomicBool::new(true));
        let (opened, result) = std::sync::mpsc::channel();
        let thread = {
            let (shared, running, id) = (Arc::clone(&shared), Arc::clone(&running), id.to_owned());
            std::thread::Builder::new()
                .name("prism-camera".into())
                .spawn(move || match open_reader(&id) {
                    Ok((reader, width, height, stride)) => {
                        let _ = opened.send(Ok(()));
                        capture(&reader, (width, height, stride), &shared, &running);
                    }
                    Err(error) => {
                        let _ = opened.send(Err(error));
                    }
                })
                .map_err(|e| DecodeError::Platform(e.to_string()))?
        };
        result
            .recv()
            .map_err(|_| DecodeError::Platform("caméra : thread arrêté".into()))??;
        Ok(Self {
            shared,
            running,
            thread: Some(thread),
        })
    }

    /// The newest frame captured since the last call, if any.
    pub fn take_frame(&self) -> Option<Frame> {
        self.shared
            .latest
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
    }

    /// Gives back a frame taken, to be filled again.
    pub fn recycle(&self, frame: Frame) {
        *self
            .shared
            .spare
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(frame);
    }
}

impl Drop for Camera {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// A source reader on the camera `id`, converting to BGRX.
fn open_reader(id: &str) -> Result<(IMFSourceReader, u32, u32, i32), DecodeError> {
    let attributes = reader_attributes()?;
    let mut sources = devices(|activate, device| {
        (device.id == id).then(|| {
            // SAFETY: creates the media source of a listed device.
            unsafe { activate.ActivateObject::<IMFMediaSource>() }
        })
    });
    let source = sources
        .pop()
        .ok_or_else(|| DecodeError::Platform("caméra débranchée".into()))?
        .map_err(|e| error("caméra", e))?;
    // SAFETY: a live media source and attribute store.
    let reader = unsafe { MFCreateSourceReaderFromMediaSource(&source, &attributes) }
        .map_err(|e| error("caméra", e))?;
    let (width, height, stride) = video_output(&reader)?;
    Ok((reader, width, height, stride))
}

/// Reads frames until `running` turns false; keeps the newest.
fn capture(
    reader: &IMFSourceReader,
    (width, height, stride): (u32, u32, i32),
    shared: &Shared,
    running: &AtomicBool,
) {
    while running.load(Ordering::Relaxed) {
        let mut flags = 0;
        let mut sample = None;
        // SAFETY: synchronous read; the out pointers live for the call.
        let read = unsafe {
            reader.ReadSample(
                MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32,
                0,
                None,
                Some(&mut flags),
                None,
                Some(&mut sample),
            )
        };
        if read.is_err() || flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
            break;
        }
        let Some(sample) = sample else {
            continue;
        };
        let mut frame = shared
            .spare
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
            .unwrap_or_default();
        if copy_sample(&sample, width, height, stride, frame.rgba_mut()).is_ok() {
            let old = shared
                .latest
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .replace(frame);
            if let Some(old) = old {
                *shared.spare.lock().unwrap_or_else(PoisonError::into_inner) = Some(old);
            }
        }
    }
}
