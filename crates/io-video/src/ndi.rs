//! NDI®: video over the local network. A sender publishes Prism's image as
//! a source; a receiver reads a source of the network.
//!
//! The NDI library is not part of Prism: it is loaded at run time from the
//! NDI runtime or SDK installed by the user. The declarations below follow
//! the headers of the NDI SDK (`Processing.NDI.*.h`, MIT licence, see
//! `NOTICE`). NDI® is a registered trademark of Vizrt NDI AB.

// Calls into the NDI library. Every block states why it is sound.
#![allow(unsafe_code)]

use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, SyncSender, TrySendError};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::JoinHandle;

use prism_render::{Gpu, wgpu};

use crate::readback::{BgraReader, Picture};

const fn fourcc(code: &[u8; 4]) -> u32 {
    u32::from_le_bytes(*code)
}

/// `NDIlib_FourCC_video_type_BGRA`.
const FOURCC_BGRA: u32 = fourcc(b"BGRA");
/// `NDIlib_FourCC_video_type_BGRX`.
const FOURCC_BGRX: u32 = fourcc(b"BGRX");
/// `NDIlib_frame_type_video`.
const FRAME_VIDEO: c_int = 1;
/// `NDIlib_frame_type_error`.
const FRAME_ERROR: c_int = 4;
/// `NDIlib_frame_format_type_progressive`.
const PROGRESSIVE: c_int = 1;
/// `NDIlib_send_timecode_synthesize`.
const TIMECODE_SYNTHESIZE: i64 = i64::MAX;
/// `NDIlib_recv_color_format_BGRX_BGRA`.
const COLOR_BGRX_BGRA: c_int = 0;
/// `NDIlib_recv_bandwidth_highest`.
const BANDWIDTH_HIGHEST: c_int = 100;

/// `NDIlib_source_t`.
#[repr(C)]
struct Source {
    ndi_name: *const c_char,
    url_address: *const c_char,
}

/// `NDIlib_find_create_t`.
#[repr(C)]
struct FindCreate {
    show_local_sources: bool,
    groups: *const c_char,
    extra_ips: *const c_char,
}

/// `NDIlib_send_create_t`.
#[repr(C)]
struct SendCreate {
    ndi_name: *const c_char,
    groups: *const c_char,
    clock_video: bool,
    clock_audio: bool,
}

/// `NDIlib_recv_create_v3_t`.
#[repr(C)]
struct RecvCreate {
    source: Source,
    color_format: c_int,
    bandwidth: c_int,
    allow_video_fields: bool,
    recv_name: *const c_char,
}

/// `NDIlib_video_frame_v2_t`.
#[repr(C)]
struct VideoFrame {
    xres: c_int,
    yres: c_int,
    fourcc: u32,
    frame_rate_n: c_int,
    frame_rate_d: c_int,
    picture_aspect_ratio: f32,
    frame_format_type: c_int,
    timecode: i64,
    data: *mut u8,
    line_stride_in_bytes: c_int,
    metadata: *const c_char,
    timestamp: i64,
}

impl VideoFrame {
    fn empty() -> Self {
        Self {
            xres: 0,
            yres: 0,
            fourcc: 0,
            frame_rate_n: 0,
            frame_rate_d: 0,
            picture_aspect_ratio: 0.0,
            frame_format_type: PROGRESSIVE,
            timecode: 0,
            data: std::ptr::null_mut(),
            line_stride_in_bytes: 0,
            metadata: std::ptr::null(),
            timestamp: 0,
        }
    }
}

type Instance = *mut c_void;

/// The functions of the NDI library used by Prism.
struct Library {
    _library: libloading::Library,
    version: unsafe extern "C" fn() -> *const c_char,
    find_create_v2: unsafe extern "C" fn(*const FindCreate) -> Instance,
    find_get_current_sources: unsafe extern "C" fn(Instance, *mut u32) -> *const Source,
    send_create: unsafe extern "C" fn(*const SendCreate) -> Instance,
    send_destroy: unsafe extern "C" fn(Instance),
    send_send_video_v2: unsafe extern "C" fn(Instance, *const VideoFrame),
    send_get_no_connections: unsafe extern "C" fn(Instance, u32) -> c_int,
    send_get_source_name: unsafe extern "C" fn(Instance) -> *const Source,
    recv_create_v3: unsafe extern "C" fn(*const RecvCreate) -> Instance,
    recv_destroy: unsafe extern "C" fn(Instance),
    recv_capture_v2:
        unsafe extern "C" fn(Instance, *mut VideoFrame, *mut c_void, *mut c_void, u32) -> c_int,
    recv_free_video_v2: unsafe extern "C" fn(Instance, *const VideoFrame),
    recv_get_no_connections: unsafe extern "C" fn(Instance) -> c_int,
}

/// Where the NDI library may be, most specific first.
fn candidates() -> Vec<std::path::PathBuf> {
    let name = if cfg!(target_os = "macos") {
        "libndi.dylib"
    } else if cfg!(windows) {
        "Processing.NDI.Lib.x64.dll"
    } else {
        "libndi.so.6"
    };
    let mut paths = Vec::new();
    // Set by the NDI runtime installers.
    if let Some(folder) = std::env::var_os("NDI_RUNTIME_DIR_V6") {
        paths.push(std::path::PathBuf::from(folder).join(name));
    }
    if cfg!(target_os = "macos") {
        paths.push("/usr/local/lib/libndi.dylib".into());
        paths.push("/Library/NDI SDK for Apple/lib/macOS/libndi.dylib".into());
    }
    paths.push(name.into());
    paths
}

impl Library {
    fn load() -> Result<Self, String> {
        let mut last_error = String::from("introuvable");
        for path in candidates() {
            // SAFETY: loading the NDI library runs its initializers, which
            // only set up the library itself.
            match unsafe { libloading::Library::new(&path) } {
                Ok(library) => return Self::bind(library),
                Err(error) => last_error = error.to_string(),
            }
        }
        Err(format!("bibliothèque NDI absente ({last_error})"))
    }

    fn bind(library: libloading::Library) -> Result<Self, String> {
        // SAFETY: each symbol is looked up by its C name and given the
        // signature its header declares.
        unsafe {
            macro_rules! function {
                ($name:literal) => {
                    *library
                        .get(concat!($name, "\0").as_bytes())
                        .map_err(|error| format!("{} : {error}", $name))?
                };
            }
            let initialize: unsafe extern "C" fn() -> bool = function!("NDIlib_initialize");
            if !initialize() {
                return Err("processeur non pris en charge par NDI".into());
            }
            Ok(Self {
                version: function!("NDIlib_version"),
                find_create_v2: function!("NDIlib_find_create_v2"),
                find_get_current_sources: function!("NDIlib_find_get_current_sources"),
                send_create: function!("NDIlib_send_create"),
                send_destroy: function!("NDIlib_send_destroy"),
                send_send_video_v2: function!("NDIlib_send_send_video_v2"),
                send_get_no_connections: function!("NDIlib_send_get_no_connections"),
                send_get_source_name: function!("NDIlib_send_get_source_name"),
                recv_create_v3: function!("NDIlib_recv_create_v3"),
                recv_destroy: function!("NDIlib_recv_destroy"),
                recv_capture_v2: function!("NDIlib_recv_capture_v2"),
                recv_free_video_v2: function!("NDIlib_recv_free_video_v2"),
                recv_get_no_connections: function!("NDIlib_recv_get_no_connections"),
                _library: library,
            })
        }
    }
}

fn library() -> Result<&'static Library, String> {
    static LIBRARY: OnceLock<Result<Library, String>> = OnceLock::new();
    LIBRARY
        .get_or_init(Library::load)
        .as_ref()
        .map_err(Clone::clone)
}

/// Copies a C string of the library.
///
/// # Safety
///
/// `text` is null or a valid C string.
unsafe fn text(text: *const c_char) -> Option<String> {
    // SAFETY: as the caller guarantees.
    (!text.is_null()).then(|| {
        unsafe { CStr::from_ptr(text) }
            .to_string_lossy()
            .into_owned()
    })
}

/// The version of the NDI library, or why it cannot be used.
pub fn ndi_version() -> Result<String, String> {
    let library = library()?;
    // SAFETY: the library returns a static string.
    Ok(unsafe { text((library.version)()) }.unwrap_or_default())
}

/// A finder kept for the life of the process: sources appear in it over
/// the first seconds.
struct Finder(Instance);

// SAFETY: the finder is only used under the mutex below.
unsafe impl Send for Finder {}

/// The NDI sources of the network, by full name (`MACHINE (SOURCE)`). The
/// first call starts the search: sources appear over the next seconds.
pub fn ndi_sources() -> Vec<String> {
    static FINDER: OnceLock<Option<Mutex<Finder>>> = OnceLock::new();
    let Ok(library) = library() else {
        return Vec::new();
    };
    let finder = FINDER.get_or_init(|| {
        let settings = FindCreate {
            show_local_sources: true,
            groups: std::ptr::null(),
            extra_ips: std::ptr::null(),
        };
        // SAFETY: valid settings, read during the call.
        let instance = unsafe { (library.find_create_v2)(&settings) };
        (!instance.is_null()).then(|| Mutex::new(Finder(instance)))
    });
    let Some(finder) = finder else {
        return Vec::new();
    };
    let Ok(finder) = finder.lock() else {
        return Vec::new();
    };
    let mut count = 0;
    // SAFETY: a live finder, used by one thread at a time; the sources stay
    // valid until the next call, made under the same lock.
    unsafe {
        let sources = (library.find_get_current_sources)(finder.0, &mut count);
        if sources.is_null() {
            return Vec::new();
        }
        std::slice::from_raw_parts(sources, count as usize)
            .iter()
            .filter_map(|source| text(source.ndi_name))
            .collect()
    }
}

/// A source published on the network, sent to from a thread of its own.
struct Sender {
    instance: Instance,
}

// SAFETY: an NDI sender may be used from any thread; it is used by one at
// a time.
unsafe impl Send for Sender {}

impl Drop for Sender {
    fn drop(&mut self) {
        if let Ok(library) = library() {
            // SAFETY: a live sender, destroyed once.
            unsafe { (library.send_destroy)(self.instance) };
        }
    }
}

impl Sender {
    fn create(name: &str) -> Result<Self, String> {
        let library = library()?;
        let name = CString::new(name.replace('\0', "")).unwrap_or_default();
        let settings = SendCreate {
            ndi_name: name.as_ptr(),
            groups: std::ptr::null(),
            // The engine paces the frames.
            clock_video: false,
            clock_audio: false,
        };
        // SAFETY: valid settings, read during the call.
        let instance = unsafe { (library.send_create)(&settings) };
        if instance.is_null() {
            return Err("création de la source NDI impossible".into());
        }
        Ok(Self { instance })
    }

    fn send(&self, picture: &Picture, rate: (i32, i32)) {
        let Ok(library) = library() else {
            return;
        };
        let frame = VideoFrame {
            xres: picture.width as c_int,
            yres: picture.height as c_int,
            fourcc: FOURCC_BGRA,
            frame_rate_n: rate.0,
            frame_rate_d: rate.1,
            picture_aspect_ratio: 0.0,
            frame_format_type: PROGRESSIVE,
            timecode: TIMECODE_SYNTHESIZE,
            data: picture.pixels.as_ptr().cast_mut(),
            line_stride_in_bytes: (picture.width * 4) as c_int,
            metadata: std::ptr::null(),
            timestamp: 0,
        };
        // SAFETY: a live sender; the frame and its pixels outlive the call,
        // which copies them (synchronous send).
        unsafe { (library.send_send_video_v2)(self.instance, &frame) };
    }

    fn connections(&self) -> usize {
        library().map_or(0, |library| {
            // SAFETY: a live sender; no wait.
            let count = unsafe { (library.send_get_no_connections)(self.instance, 0) };
            count.max(0) as usize
        })
    }

    fn source_name(&self) -> Option<String> {
        let library = library().ok()?;
        // SAFETY: a live sender; the source stays valid while it lives.
        unsafe {
            let source = (library.send_get_source_name)(self.instance);
            if source.is_null() {
                return None;
            }
            text((*source).ndi_name)
        }
    }
}

/// The program image published as an NDI source of the network.
///
/// Each frame is converted to BGRA on the GPU, read back without waiting
/// (a few frames later) and sent by a thread of its own.
pub struct NdiSender {
    name: String,
    frames: Option<SyncSender<Picture>>,
    thread: Option<JoinHandle<()>>,
    state: Arc<Mutex<SenderState>>,
    reader: BgraReader,
    error: Option<String>,
    /// Frame rate announced, as numerator and denominator.
    rate: (i32, i32),
}

#[derive(Default)]
struct SenderState {
    connections: usize,
    source_name: Option<String>,
}

impl NdiSender {
    /// A source published under `name` from its first frame, announced at
    /// `frame_rate` images per second.
    pub fn new(name: impl Into<String>, frame_rate: f64) -> Self {
        Self {
            name: name.into(),
            frames: None,
            thread: None,
            state: Arc::default(),
            reader: BgraReader::default(),
            error: None,
            rate: ((frame_rate * 1000.0).round() as i32, 1000),
        }
    }

    /// Why the source cannot be published, if it cannot.
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Full name of the source on the network, once published.
    pub fn source_name(&self) -> Option<String> {
        self.state.lock().ok()?.source_name.clone()
    }

    /// Number of receivers reading the source.
    pub fn connections(&self) -> usize {
        self.state.lock().map_or(0, |state| state.connections)
    }

    fn start(&mut self) -> bool {
        if self.frames.is_some() {
            return true;
        }
        if self.error.is_some() {
            return false;
        }
        let sender = match Sender::create(&self.name) {
            Ok(sender) => sender,
            Err(error) => {
                self.error = Some(error);
                return false;
            }
        };
        // One frame waits at most: a slow network drops frames instead of
        // delaying the engine.
        let (frames, pictures) = mpsc::sync_channel::<Picture>(1);
        let state = Arc::clone(&self.state);
        let rate = self.rate;
        let thread = std::thread::Builder::new()
            .name("prism-ndi-send".into())
            .spawn(move || {
                if let Ok(mut state) = state.lock() {
                    state.source_name = sender.source_name();
                }
                for picture in pictures {
                    sender.send(&picture, rate);
                    if let Ok(mut state) = state.lock() {
                        state.connections = sender.connections();
                    }
                }
            });
        match thread {
            Ok(thread) => {
                self.frames = Some(frames);
                self.thread = Some(thread);
                true
            }
            Err(error) => {
                self.error = Some(error.to_string());
                false
            }
        }
    }

    /// Publishes `program`, the program image (16-bit float, `size`
    /// pixels).
    pub fn publish(&mut self, gpu: &Gpu, program: &wgpu::TextureView, size: (u32, u32)) {
        if size.0 == 0 || size.1 == 0 || !self.start() {
            return;
        }
        for picture in self.reader.finished(gpu) {
            if let Some(frames) = &self.frames
                && let Err(TrySendError::Disconnected(_)) = frames.try_send(picture)
            {
                self.frames = None;
            }
        }
        self.reader.submit(gpu, program, size);
    }
}

impl Drop for NdiSender {
    fn drop(&mut self) {
        // The thread ends when its channel closes.
        self.frames = None;
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// An image received: 8-bit BGRA rows, packed.
#[derive(Debug, Default)]
pub struct NdiFrame {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// `width × height × 4` bytes, blue, green, red, alpha.
    pub pixels: Vec<u8>,
}

#[derive(Default)]
struct Received {
    /// Newest image, not yet taken.
    frame: Option<NdiFrame>,
    /// Image given back, to be filled again.
    spare: Option<NdiFrame>,
    connected: bool,
}

/// A source of the network read by a thread of its own.
pub struct NdiReceiver {
    received: Arc<Mutex<Received>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

/// A receiver instance, owned by its thread.
struct Recv(Instance);

// SAFETY: moved once to the receiving thread, which alone uses it.
unsafe impl Send for Recv {}

impl NdiReceiver {
    /// Starts reading the source `name` (`MACHINE (SOURCE)`): the library
    /// finds it on the network, now or when it appears.
    pub fn connect(name: &str) -> Result<Self, String> {
        let library = library()?;
        let name = CString::new(name.replace('\0', "")).map_err(|error| error.to_string())?;
        let settings = RecvCreate {
            source: Source {
                ndi_name: name.as_ptr(),
                url_address: std::ptr::null(),
            },
            color_format: COLOR_BGRX_BGRA,
            bandwidth: BANDWIDTH_HIGHEST,
            allow_video_fields: false,
            recv_name: c"Prism".as_ptr(),
        };
        // SAFETY: valid settings, read during the call.
        let instance = unsafe { (library.recv_create_v3)(&settings) };
        if instance.is_null() {
            return Err("création du récepteur NDI impossible".into());
        }
        let recv = Recv(instance);
        let received: Arc<Mutex<Received>> = Arc::default();
        let stop = Arc::new(AtomicBool::new(false));
        let thread = {
            let received = Arc::clone(&received);
            let stop = Arc::clone(&stop);
            std::thread::Builder::new()
                .name("prism-ndi-receive".into())
                .spawn(move || receive(library, recv, &received, &stop))
                .map_err(|error| error.to_string())?
        };
        Ok(Self {
            received,
            stop,
            thread: Some(thread),
        })
    }

    /// The newest image, if a new one came.
    pub fn take_frame(&self) -> Option<NdiFrame> {
        self.received.lock().ok()?.frame.take()
    }

    /// Gives an image back to be filled again.
    pub fn recycle(&self, frame: NdiFrame) {
        if let Ok(mut received) = self.received.lock() {
            received.spare = Some(frame);
        }
    }

    /// Whether the source is found and sends.
    pub fn is_connected(&self) -> bool {
        self.received
            .lock()
            .is_ok_and(|received| received.connected)
    }
}

impl Drop for NdiReceiver {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Reads images until `stop`, keeping the newest in `received`.
fn receive(library: &Library, recv: Recv, received: &Mutex<Received>, stop: &AtomicBool) {
    while !stop.load(Ordering::Acquire) {
        let mut video = VideoFrame::empty();
        // SAFETY: a live receiver of this thread; audio and metadata are not
        // asked for. Waits at most 100 ms, so `stop` is seen quickly.
        let kind = unsafe {
            (library.recv_capture_v2)(
                recv.0,
                &mut video,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                100,
            )
        };
        // SAFETY: a live receiver; no wait.
        let connected = unsafe { (library.recv_get_no_connections)(recv.0) } > 0;
        let Ok(mut state) = received.lock() else {
            break;
        };
        state.connected = connected && kind != FRAME_ERROR;
        if kind != FRAME_VIDEO {
            continue;
        }
        let (width, height) = (video.xres.max(0) as u32, video.yres.max(0) as u32);
        let stride = video.line_stride_in_bytes.max(0) as usize;
        let row = width as usize * 4;
        if (video.fourcc == FOURCC_BGRA || video.fourcc == FOURCC_BGRX)
            && !video.data.is_null()
            && stride >= row
        {
            let mut frame = state.spare.take().unwrap_or_default();
            frame.width = width;
            frame.height = height;
            frame.pixels.clear();
            // SAFETY: the library returned `height` rows of `stride` bytes.
            let data = unsafe { std::slice::from_raw_parts(video.data, stride * height as usize) };
            for line in data.chunks(stride) {
                frame.pixels.extend_from_slice(&line[..row]);
            }
            if video.fourcc == FOURCC_BGRX {
                for pixel in frame.pixels.as_chunks_mut::<4>().0 {
                    pixel[3] = 255;
                }
            }
            // The previous image, never taken, is filled again next time.
            if let Some(old) = state.frame.replace(frame) {
                state.spare = Some(old);
            }
        }
        drop(state);
        // SAFETY: the frame captured above, freed once.
        unsafe { (library.recv_free_video_v2)(recv.0, &video) };
    }
    // SAFETY: the receiver of this thread, destroyed once.
    unsafe { (library.recv_destroy)(recv.0) };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fourcc_codes_match_the_headers() {
        // NDI_LIB_FOURCC('B','G','R','A'): first character in the low byte.
        assert_eq!(FOURCC_BGRA, 0x4152_4742);
        assert_eq!(FOURCC_BGRX, 0x5852_4742);
    }
}
