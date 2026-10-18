//! Spout (Windows): images shared between applications of the computer
//! through DirectX 11 shared textures, as Syphon does on macOS (ADR 0037).
//!
//! A sender registers its name in the shared memory `SpoutSenderNames`
//! (256 bytes per name), describes its texture in a shared memory named
//! after it (`SharedTextureInfo`: share handle, size, DXGI format), and
//! guards the texture with the mutex `<name>_SpoutAccessMutex`. The layout
//! follows the open Spout 2 SDK (BSD 2-clause licence, see `NOTICE`).
//!
//! The image goes through the CPU: Prism renders with wgpu, whose textures
//! cannot be shared with Direct3D 11 without native interop.

// Calls into Win32 and Direct3D 11. Every block states why it is sound.
#![allow(unsafe_code)]

use std::ffi::c_void;
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use prism_render::{Gpu, wgpu};
use windows::Win32::Foundation::{
    CloseHandle, HANDLE, HMODULE, INVALID_HANDLE_VALUE, WAIT_ABANDONED, WAIT_OBJECT_0,
};
use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE_HARDWARE, D3D_FEATURE_LEVEL_11_0};
use windows::Win32::Graphics::Direct3D11::{
    D3D11_BIND_RENDER_TARGET, D3D11_BIND_SHADER_RESOURCE, D3D11_CPU_ACCESS_READ,
    D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_MAP_READ, D3D11_MAPPED_SUBRESOURCE,
    D3D11_RESOURCE_MISC_SHARED, D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT,
    D3D11_USAGE_STAGING, D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_FORMAT, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC,
};
use windows::Win32::Graphics::Dxgi::IDXGIResource;
use windows::Win32::System::Memory::{
    CreateFileMappingW, FILE_MAP_ALL_ACCESS, MEMORY_BASIC_INFORMATION, MEMORY_MAPPED_VIEW_ADDRESS,
    MapViewOfFile, OpenFileMappingW, PAGE_READWRITE, UnmapViewOfFile, VirtualQuery,
};
use windows::Win32::System::Threading::{
    CreateMutexW, OpenMutexW, ReleaseMutex, SYNCHRONIZATION_SYNCHRONIZE, WaitForSingleObject,
};
use windows::core::{HSTRING, Interface, PCWSTR};

use crate::readback::{BgraReader, Picture};

/// Bytes of a name in the shared lists.
const NAME_BYTES: usize = 256;

/// Names the list holds when Prism creates it.
const DEFAULT_SENDERS: usize = 64;

/// Shared memory listing the senders.
const SENDER_NAMES: &str = "SpoutSenderNames";

/// Shared memory naming the sender receivers pick by default.
const ACTIVE_SENDER: &str = "ActiveSenderName";

/// Longest wait for the texture of a sender.
const ACCESS_WAIT_MS: u32 = 67;

/// `SharedTextureInfo` of the Spout SDK.
#[repr(C)]
#[derive(Clone, Copy)]
struct SharedTextureInfo {
    share_handle: u32,
    width: u32,
    height: u32,
    format: u32,
    usage: u32,
    description: [u16; 128],
    partner_id: u32,
}

/// A named shared memory, mapped, with the mutex that guards it.
struct SharedMemory {
    mapping: HANDLE,
    view: MEMORY_MAPPED_VIEW_ADDRESS,
    size: usize,
    mutex: HANDLE,
}

// SAFETY: the handles and the mapped view belong to the process; the view is
// only touched while the mutex is held.
unsafe impl Send for SharedMemory {}

impl SharedMemory {
    /// Opens the shared memory `name`, created with `size` bytes if absent.
    /// `size` is the size of the view when it already exists.
    fn open_or_create(name: &str, size: usize) -> windows::core::Result<Self> {
        let wide = HSTRING::from(name);
        // SAFETY: a memory mapping backed by the paging file; the name is a
        // valid wide string for the call.
        let mapping = unsafe {
            CreateFileMappingW(
                INVALID_HANDLE_VALUE,
                None,
                PAGE_READWRITE,
                0,
                size as u32,
                PCWSTR(wide.as_ptr()),
            )?
        };
        Self::map(name, mapping)
    }

    /// Opens the existing shared memory `name`.
    fn open(name: &str) -> windows::core::Result<Self> {
        let wide = HSTRING::from(name);
        // SAFETY: as above.
        let mapping =
            unsafe { OpenFileMappingW(FILE_MAP_ALL_ACCESS.0, false, PCWSTR(wide.as_ptr()))? };
        Self::map(name, mapping)
    }

    fn map(name: &str, mapping: HANDLE) -> windows::core::Result<Self> {
        // SAFETY: maps the whole of a valid mapping.
        let view = unsafe { MapViewOfFile(mapping, FILE_MAP_ALL_ACCESS, 0, 0, 0) };
        if view.Value.is_null() {
            // SAFETY: a handle this function owns.
            unsafe {
                let _ = CloseHandle(mapping);
            }
            return Err(windows::core::Error::from_thread());
        }
        let mut info = MEMORY_BASIC_INFORMATION::default();
        // SAFETY: describes the region of a live view into `info`.
        let described = unsafe {
            VirtualQuery(
                Some(view.Value),
                &mut info,
                size_of::<MEMORY_BASIC_INFORMATION>(),
            )
        };
        let size = if described == 0 { 0 } else { info.RegionSize };
        let mutex_name = HSTRING::from(format!("{name}_mutex"));
        // SAFETY: a named mutex, opened if it exists.
        let mutex = unsafe { CreateMutexW(None, false, PCWSTR(mutex_name.as_ptr()))? };
        Ok(Self {
            mapping,
            view,
            size,
            mutex,
        })
    }

    /// Runs `access` on the bytes of the memory, mutex held.
    fn with<T>(&self, access: impl FnOnce(&mut [u8]) -> T) -> Option<T> {
        // SAFETY: waits on a live mutex handle.
        let waited = unsafe { WaitForSingleObject(self.mutex, ACCESS_WAIT_MS) };
        if waited != WAIT_OBJECT_0 && waited != WAIT_ABANDONED {
            return None;
        }
        // SAFETY: the view holds `size` bytes and no one else writes while
        // the mutex is held.
        let bytes =
            unsafe { std::slice::from_raw_parts_mut(self.view.Value.cast::<u8>(), self.size) };
        let result = access(bytes);
        // SAFETY: the mutex is held by this thread.
        unsafe {
            let _ = ReleaseMutex(self.mutex);
        }
        Some(result)
    }
}

impl Drop for SharedMemory {
    fn drop(&mut self) {
        // SAFETY: the view and handles were opened by this value.
        unsafe {
            let _ = UnmapViewOfFile(self.view);
            let _ = CloseHandle(self.mapping);
            let _ = CloseHandle(self.mutex);
        }
    }
}

/// The name in a 256-byte slot, up to its first zero.
fn slot_name(slot: &[u8]) -> Option<String> {
    let end = slot
        .iter()
        .position(|&byte| byte == 0)
        .unwrap_or(slot.len());
    (end > 0).then(|| String::from_utf8_lossy(&slot[..end]).into_owned())
}

/// Writes `name` in a slot, zero-terminated and cut to fit.
fn write_slot(slot: &mut [u8], name: &str) {
    slot.fill(0);
    let bytes = name.as_bytes();
    let length = bytes.len().min(slot.len() - 1);
    slot[..length].copy_from_slice(&bytes[..length]);
}

/// The names of a sender list.
fn list_names(bytes: &[u8]) -> Vec<String> {
    bytes
        .as_chunks::<NAME_BYTES>()
        .0
        .iter()
        .filter_map(|slot| slot_name(slot))
        .collect()
}

/// Adds `name` to a sender list. Returns false when the list is full.
fn add_name(bytes: &mut [u8], name: &str) -> bool {
    if list_names(bytes).iter().any(|existing| existing == name) {
        return true;
    }
    match bytes
        .as_chunks_mut::<NAME_BYTES>()
        .0
        .iter_mut()
        .find(|slot| slot[0] == 0)
    {
        Some(slot) => {
            write_slot(slot, name);
            true
        }
        None => false,
    }
}

/// Removes `name` from a sender list; the names after it move up.
fn remove_name(bytes: &mut [u8], name: &str) {
    let names: Vec<String> = list_names(bytes)
        .into_iter()
        .filter(|existing| existing != name)
        .collect();
    for (index, slot) in bytes.as_chunks_mut::<NAME_BYTES>().0.iter_mut().enumerate() {
        match names.get(index) {
            Some(name) => write_slot(slot, name),
            None => slot.fill(0),
        }
    }
}

/// The Spout senders of the computer, by name.
pub fn spout_senders() -> Vec<String> {
    SharedMemory::open(SENDER_NAMES)
        .ok()
        .and_then(|names| names.with(|bytes| list_names(bytes)))
        .unwrap_or_default()
}

/// A Direct3D 11 device of the default adapter.
fn d3d11_device() -> windows::core::Result<(ID3D11Device, ID3D11DeviceContext)> {
    let mut device = None;
    let mut context = None;
    // SAFETY: creates a device on the default hardware adapter; the out
    // pointers are valid for the call.
    unsafe {
        D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_HARDWARE,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            Some(&[D3D_FEATURE_LEVEL_11_0]),
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            Some(&mut context),
        )?;
    }
    match (device, context) {
        (Some(device), Some(context)) => Ok((device, context)),
        _ => Err(windows::core::Error::from_thread()),
    }
}

fn texture_desc(width: u32, height: u32, format: DXGI_FORMAT) -> D3D11_TEXTURE2D_DESC {
    D3D11_TEXTURE2D_DESC {
        Width: width,
        Height: height,
        MipLevels: 1,
        ArraySize: 1,
        Format: format,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Usage: D3D11_USAGE_DEFAULT,
        BindFlags: (D3D11_BIND_SHADER_RESOURCE.0 | D3D11_BIND_RENDER_TARGET.0) as u32,
        CPUAccessFlags: 0,
        MiscFlags: D3D11_RESOURCE_MISC_SHARED.0 as u32,
    }
}

/// A named mutex, created if absent.
fn named_mutex(name: &str) -> windows::core::Result<HANDLE> {
    let wide = HSTRING::from(name);
    // SAFETY: a named mutex, opened if it exists.
    unsafe { CreateMutexW(None, false, PCWSTR(wide.as_ptr())) }
}

/// Runs `access` with the texture mutex `mutex` held, if it can be taken.
fn with_mutex<T>(mutex: HANDLE, access: impl FnOnce() -> T) -> Option<T> {
    // SAFETY: waits on a live mutex handle.
    let waited = unsafe { WaitForSingleObject(mutex, ACCESS_WAIT_MS) };
    if waited != WAIT_OBJECT_0 && waited != WAIT_ABANDONED {
        return None;
    }
    let result = access();
    // SAFETY: the mutex is held by this thread.
    unsafe {
        let _ = ReleaseMutex(mutex);
    }
    Some(result)
}

/// A registered sender and its texture, on the thread that sends.
struct Published {
    name: String,
    names: SharedMemory,
    info: SharedMemory,
    access: HANDLE,
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    texture: Option<(ID3D11Texture2D, u32, u32)>,
}

impl Published {
    fn register(name: &str) -> windows::core::Result<Self> {
        let (device, context) = d3d11_device()?;
        let names = SharedMemory::open_or_create(SENDER_NAMES, NAME_BYTES * DEFAULT_SENDERS)?;
        // Another sender of that name: a number is added.
        let taken = names.with(|bytes| list_names(bytes)).unwrap_or_default();
        let mut unique = name.to_owned();
        let mut number = 2;
        while taken.contains(&unique) {
            unique = format!("{name} {number}");
            number += 1;
        }
        let info = SharedMemory::open_or_create(&unique, size_of::<SharedTextureInfo>())?;
        let access = named_mutex(&format!("{unique}_SpoutAccessMutex"))?;
        if names.with(|bytes| add_name(bytes, &unique)) != Some(true) {
            return Err(windows::core::Error::from_thread());
        }
        if let Ok(active) = SharedMemory::open_or_create(ACTIVE_SENDER, NAME_BYTES) {
            active.with(|bytes| {
                let length = NAME_BYTES.min(bytes.len());
                if slot_name(&bytes[..length]).is_none() {
                    write_slot(&mut bytes[..length], &unique);
                }
            });
        }
        Ok(Self {
            name: unique,
            names,
            info,
            access,
            device,
            context,
            texture: None,
        })
    }

    /// Copies `picture` into the shared texture, made again at a new size.
    fn send(&mut self, picture: &Picture) -> windows::core::Result<()> {
        let size_changed = self
            .texture
            .as_ref()
            .is_none_or(|(_, width, height)| (*width, *height) != (picture.width, picture.height));
        if size_changed {
            let desc = texture_desc(picture.width, picture.height, DXGI_FORMAT_B8G8R8A8_UNORM);
            let mut texture = None;
            // SAFETY: a valid description; the out pointer lives for the call.
            unsafe {
                self.device
                    .CreateTexture2D(&desc, None, Some(&mut texture))?
            };
            let texture = texture.ok_or_else(windows::core::Error::from_thread)?;
            let resource: IDXGIResource = texture.cast()?;
            // SAFETY: a live shareable texture.
            let handle = unsafe { resource.GetSharedHandle()? };
            let mut info = SharedTextureInfo {
                // Spout shares 32-bit handles, as legacy shared handles are.
                share_handle: handle.0 as usize as u32,
                width: picture.width,
                height: picture.height,
                format: DXGI_FORMAT_B8G8R8A8_UNORM.0 as u32,
                usage: 0,
                description: [0; 128],
                partner_id: 0,
            };
            for (slot, unit) in info.description.iter_mut().zip("Prism".encode_utf16()) {
                *slot = unit;
            }
            self.info.with(|bytes| {
                let size = size_of::<SharedTextureInfo>().min(bytes.len());
                // SAFETY: `info` is plain data of at least `size` bytes.
                let raw =
                    unsafe { std::slice::from_raw_parts((&raw const info).cast::<u8>(), size) };
                bytes[..size].copy_from_slice(raw);
            });
            self.texture = Some((texture, picture.width, picture.height));
        }
        let Some((texture, _, _)) = &self.texture else {
            return Ok(());
        };
        with_mutex(self.access, || {
            // SAFETY: the pixels hold `height` rows of `width × 4` bytes.
            unsafe {
                self.context.UpdateSubresource(
                    texture,
                    0,
                    None,
                    picture.pixels.as_ptr().cast::<c_void>(),
                    picture.width * 4,
                    0,
                );
                self.context.Flush();
            }
        });
        Ok(())
    }
}

impl Drop for Published {
    fn drop(&mut self) {
        let name = self.name.clone();
        self.names.with(|bytes| remove_name(bytes, &name));
        if let Ok(active) = SharedMemory::open(ACTIVE_SENDER) {
            active.with(|bytes| {
                let length = NAME_BYTES.min(bytes.len());
                let slot = &mut bytes[..length];
                if slot_name(slot).as_deref() == Some(name.as_str()) {
                    slot.fill(0);
                }
            });
        }
        // SAFETY: a handle this value owns.
        unsafe {
            let _ = CloseHandle(self.access);
        }
    }
}

#[derive(Default)]
struct SenderState {
    name: Option<String>,
    error: Option<String>,
}

/// The program image published as a Spout sender.
///
/// Each frame is converted to BGRA on the GPU, read back without waiting
/// (a few frames later) and copied into the shared texture by a thread of
/// its own.
pub struct SpoutSender {
    name: String,
    frames: Option<SyncSender<Picture>>,
    thread: Option<JoinHandle<()>>,
    state: Arc<Mutex<SenderState>>,
    reader: BgraReader,
}

impl SpoutSender {
    /// A sender named `name` (a number is added if the name is taken),
    /// registered at its first frame.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            frames: None,
            thread: None,
            state: Arc::default(),
            reader: BgraReader::default(),
        }
    }

    /// Name under which receivers see it, once registered.
    pub fn sender_name(&self) -> Option<String> {
        self.state.lock().ok()?.name.clone()
    }

    /// Why it cannot be published, if it cannot.
    pub fn error(&self) -> Option<String> {
        self.state.lock().ok()?.error.clone()
    }

    fn start(&mut self) {
        if self.thread.is_some() {
            return;
        }
        let (frames, pictures) = mpsc::sync_channel::<Picture>(1);
        let state = Arc::clone(&self.state);
        let name = self.name.clone();
        let thread = std::thread::Builder::new()
            .name("prism-spout-send".into())
            .spawn(move || send_loop(&name, &pictures, &state));
        if let Ok(thread) = thread {
            self.frames = Some(frames);
            self.thread = Some(thread);
        }
    }

    /// Publishes `program`, the program image (16-bit float, `size`
    /// pixels).
    pub fn publish(&mut self, gpu: &Gpu, program: &wgpu::TextureView, size: (u32, u32)) {
        if size.0 == 0 || size.1 == 0 {
            return;
        }
        self.start();
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

fn send_loop(name: &str, pictures: &Receiver<Picture>, state: &Mutex<SenderState>) {
    let mut published = match Published::register(name) {
        Ok(published) => published,
        Err(error) => {
            if let Ok(mut state) = state.lock() {
                state.error = Some(format!("Spout : {error}"));
            }
            return;
        }
    };
    if let Ok(mut state) = state.lock() {
        state.name = Some(published.name.clone());
    }
    for picture in pictures {
        if let Err(error) = published.send(&picture)
            && let Ok(mut state) = state.lock()
        {
            state.error = Some(format!("Spout : {error}"));
        }
    }
}

impl Drop for SpoutSender {
    fn drop(&mut self) {
        // The thread ends, and unregisters, when its channel closes.
        self.frames = None;
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// An image received: 8-bit BGRA rows, packed.
pub struct SpoutFrame {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Pixels, `width × 4` bytes per row.
    pub pixels: Vec<u8>,
}

/// Reads the image of a Spout sender, on a thread of its own.
pub struct SpoutReceiver {
    latest: Arc<Mutex<Option<SpoutFrame>>>,
    connected: Arc<std::sync::atomic::AtomicBool>,
    running: Arc<std::sync::atomic::AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl SpoutReceiver {
    /// Reads the sender `name`, waiting for it while it is absent.
    pub fn connect(name: &str) -> Self {
        let latest = Arc::new(Mutex::new(None));
        let connected = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let running = Arc::new(std::sync::atomic::AtomicBool::new(true));
        let thread = {
            let (latest, connected, running) = (
                Arc::clone(&latest),
                Arc::clone(&connected),
                Arc::clone(&running),
            );
            let name = name.to_owned();
            std::thread::Builder::new()
                .name("prism-spout-receive".into())
                .spawn(move || receive_loop(&name, &latest, &connected, &running))
                .ok()
        };
        Self {
            latest,
            connected,
            running,
            thread,
        }
    }

    /// The newest image since the last call.
    pub fn take_frame(&self) -> Option<SpoutFrame> {
        self.latest.lock().ok()?.take()
    }

    /// Whether the sender is there.
    pub fn is_connected(&self) -> bool {
        self.connected.load(std::sync::atomic::Ordering::Relaxed)
    }
}

impl Drop for SpoutReceiver {
    fn drop(&mut self) {
        self.running
            .store(false, std::sync::atomic::Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// What a receiver opened of a sender.
struct Opened {
    handle: u32,
    width: u32,
    height: u32,
    shared: ID3D11Texture2D,
    staging: ID3D11Texture2D,
}

fn receive_loop(
    name: &str,
    latest: &Mutex<Option<SpoutFrame>>,
    connected: &std::sync::atomic::AtomicBool,
    running: &std::sync::atomic::AtomicBool,
) {
    use std::sync::atomic::Ordering;
    let Ok((device, context)) = d3d11_device() else {
        return;
    };
    let mut opened: Option<Opened> = None;
    let mut info_memory: Option<SharedMemory> = None;
    let access = {
        let wide = HSTRING::from(format!("{name}_SpoutAccessMutex"));
        // SAFETY: opens a named mutex if it exists.
        unsafe { OpenMutexW(SYNCHRONIZATION_SYNCHRONIZE, false, PCWSTR(wide.as_ptr())).ok() }
    };
    while running.load(Ordering::Relaxed) {
        std::thread::sleep(Duration::from_millis(15));
        if info_memory.is_none() {
            info_memory = SharedMemory::open(name).ok();
        }
        let info = info_memory.as_ref().and_then(|memory| {
            memory.with(|bytes| {
                (bytes.len() >= size_of::<SharedTextureInfo>()).then(|| {
                    // SAFETY: the memory holds a `SharedTextureInfo`, read
                    // unaligned.
                    unsafe { bytes.as_ptr().cast::<SharedTextureInfo>().read_unaligned() }
                })
            })?
        });
        let Some(info) = info.filter(|info| info.share_handle != 0 && info.width > 0) else {
            connected.store(false, Ordering::Relaxed);
            opened = None;
            info_memory = None;
            continue;
        };
        let current = opened.as_ref().is_some_and(|opened| {
            (opened.handle, opened.width, opened.height)
                == (info.share_handle, info.width, info.height)
        });
        if !current {
            opened = open_shared(&device, &info).ok();
        }
        let Some(open) = &opened else {
            connected.store(false, Ordering::Relaxed);
            continue;
        };
        connected.store(true, Ordering::Relaxed);
        let copy = || {
            // SAFETY: both textures are live and of the same size and format.
            unsafe { context.CopyResource(&open.staging, &open.shared) };
        };
        match access {
            Some(mutex) => {
                if with_mutex(mutex, copy).is_none() {
                    continue;
                }
            }
            None => copy(),
        }
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        // SAFETY: maps a staging texture with CPU read access.
        if unsafe { context.Map(&open.staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped)) }.is_err() {
            continue;
        }
        let row = open.width as usize * 4;
        let mut pixels = Vec::with_capacity(row * open.height as usize);
        for line in 0..open.height as usize {
            // SAFETY: the mapping holds `height` rows of `RowPitch` bytes,
            // each at least `width × 4` long.
            let source = unsafe {
                std::slice::from_raw_parts(
                    mapped
                        .pData
                        .cast::<u8>()
                        .add(line * mapped.RowPitch as usize),
                    row,
                )
            };
            pixels.extend_from_slice(source);
        }
        // SAFETY: unmaps what was mapped above.
        unsafe { context.Unmap(&open.staging, 0) };
        if let Ok(mut latest) = latest.lock() {
            *latest = Some(SpoutFrame {
                width: open.width,
                height: open.height,
                pixels,
            });
        }
    }
    if let Some(mutex) = access {
        // SAFETY: a handle this function opened.
        unsafe {
            let _ = CloseHandle(mutex);
        }
    }
}

fn open_shared(device: &ID3D11Device, info: &SharedTextureInfo) -> windows::core::Result<Opened> {
    let mut shared: Option<ID3D11Texture2D> = None;
    // SAFETY: opens the texture another process shared under this handle.
    unsafe {
        device.OpenSharedResource(
            HANDLE(info.share_handle as usize as *mut c_void),
            &mut shared,
        )?;
    }
    let shared = shared.ok_or_else(windows::core::Error::from_thread)?;
    let mut desc = D3D11_TEXTURE2D_DESC::default();
    // SAFETY: reads the description of a live texture.
    unsafe { shared.GetDesc(&mut desc) };
    if desc.Format != DXGI_FORMAT_B8G8R8A8_UNORM {
        return Err(windows::core::Error::from_thread());
    }
    let staging_desc = D3D11_TEXTURE2D_DESC {
        Usage: D3D11_USAGE_STAGING,
        BindFlags: 0,
        CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
        MiscFlags: 0,
        ..desc
    };
    let mut staging = None;
    // SAFETY: a valid description; the out pointer lives for the call.
    unsafe { device.CreateTexture2D(&staging_desc, None, Some(&mut staging))? };
    Ok(Opened {
        handle: info.share_handle,
        width: desc.Width,
        height: desc.Height,
        shared,
        staging: staging.ok_or_else(windows::core::Error::from_thread)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_texture_info_has_the_sdk_size() {
        assert_eq!(size_of::<SharedTextureInfo>(), 280);
    }

    #[test]
    fn names_are_added_and_removed() {
        let mut bytes = vec![0u8; NAME_BYTES * 3];
        assert!(add_name(&mut bytes, "A"));
        assert!(add_name(&mut bytes, "Prism"));
        assert!(add_name(&mut bytes, "Prism"));
        assert!(add_name(&mut bytes, "C"));
        assert!(!add_name(&mut bytes, "D"));
        assert_eq!(list_names(&bytes), ["A", "Prism", "C"]);
        remove_name(&mut bytes, "Prism");
        assert_eq!(list_names(&bytes), ["A", "C"]);
    }

    #[test]
    fn a_sender_is_seen_and_gone() {
        // Machines without a Direct3D 11 GPU (some CI runners) skip.
        let Ok(mut sender) = Published::register("Prism test") else {
            eprintln!("no Direct3D 11 device: skipped");
            return;
        };
        assert!(spout_senders().contains(&sender.name));
        sender
            .send(&Picture {
                width: 4,
                height: 2,
                pixels: vec![255; 32],
            })
            .unwrap();
        let name = sender.name.clone();
        drop(sender);
        assert!(!spout_senders().contains(&name));
    }
}
