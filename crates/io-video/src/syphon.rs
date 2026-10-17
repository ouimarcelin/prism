//! Syphon: images shared between applications of the Mac. A server
//! publishes Prism's image under a name; a client reads the image another
//! application publishes.
//!
//! Syphon's own code (third_party/syphon, BSD licence) runs the protocol;
//! the glue in `syphon_glue.m` copies textures to and from the shared
//! surfaces on the GPU queue of wgpu.

// Calls into the Objective-C glue. Every block states why it is sound.
#![allow(unsafe_code)]

use std::ffi::{CStr, CString, c_char, c_int, c_uint, c_void};
use std::ptr::NonNull;

use prism_render::{Gpu, wgpu};

use crate::convert::Converter;
use wgpu::hal::api::Metal;

unsafe extern "C" {
    fn prism_syphon_server_new(name: *const c_char, device: *mut c_void) -> *mut c_void;
    fn prism_syphon_server_publish(server: *mut c_void, texture: *mut c_void, queue: *mut c_void);
    fn prism_syphon_server_has_clients(server: *mut c_void) -> c_int;
    fn prism_syphon_server_free(server: *mut c_void);
    fn prism_syphon_directory_start();
    fn prism_syphon_servers() -> *mut c_char;
    fn prism_syphon_client_new(
        app: *const c_char,
        name: *const c_char,
        device: *mut c_void,
    ) -> *mut c_void;
    fn prism_syphon_client_of_server(server: *mut c_void, device: *mut c_void) -> *mut c_void;
    fn prism_syphon_client_is_valid(client: *mut c_void) -> c_int;
    fn prism_syphon_client_frame(
        client: *mut c_void,
        width: *mut c_uint,
        height: *mut c_uint,
    ) -> *mut c_void;
    fn prism_syphon_texture_release(texture: *mut c_void);
    fn prism_syphon_copy(source: *mut c_void, destination: *mut c_void, queue: *mut c_void);
    fn prism_syphon_client_free(client: *mut c_void);
    fn free(pointer: *mut c_void);
}

/// The Metal device of wgpu, as a pointer for the glue.
fn metal_device(gpu: &Gpu) -> Option<*mut c_void> {
    // SAFETY: the Metal device of wgpu, borrowed for the call.
    unsafe { gpu.device.as_hal::<Metal>() }.map(|device| {
        let raw: &wgpu::hal::metal::Device = &device;
        std::ptr::from_ref(&**raw.raw_device())
            .cast::<c_void>()
            .cast_mut()
    })
}

/// Starts listening for the servers of other applications. Call it once,
/// on the main thread: the announcements of servers arrive on its run loop.
pub fn start_syphon_directory() {
    // SAFETY: creates the shared directory; no argument.
    unsafe { prism_syphon_directory_start() };
}

/// The servers other applications publish, as application and name.
pub fn syphon_servers() -> Vec<(String, String)> {
    // SAFETY: the glue returns a string allocated with strdup, freed here.
    let list = unsafe {
        let raw = prism_syphon_servers();
        if raw.is_null() {
            return Vec::new();
        }
        let list = CStr::from_ptr(raw).to_string_lossy().into_owned();
        free(raw.cast());
        list
    };
    list.lines()
        .filter_map(|line| line.split_once('\t'))
        .map(|(app, name)| (app.to_owned(), name.to_owned()))
        .collect()
}

/// A Syphon server, on the device of wgpu.
struct Server(NonNull<c_void>);

impl Drop for Server {
    fn drop(&mut self) {
        // SAFETY: the pointer came from `prism_syphon_server_new` and is
        // released once.
        unsafe { prism_syphon_server_free(self.0.as_ptr()) };
    }
}

/// The program image published over Syphon under a name.
pub struct SyphonServer {
    name: String,
    server: Option<Server>,
    converter: Converter,
    failed: bool,
}

// SAFETY: the server is created, used and dropped by one thread at a time
// (the render thread); Syphon servers may be used from any thread.
unsafe impl Send for SyphonServer {}

impl SyphonServer {
    /// A server publishing under `name` from its first frame.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            server: None,
            converter: Converter::default(),
            failed: false,
        }
    }

    fn server(&mut self, gpu: &Gpu) -> Option<&Server> {
        if self.server.is_none() && !self.failed {
            let device = metal_device(gpu);
            let name = CString::new(self.name.replace('\0', "")).unwrap_or_default();
            self.server = device.and_then(|device| {
                // SAFETY: valid name and Metal device; the server retains
                // the device.
                NonNull::new(unsafe { prism_syphon_server_new(name.as_ptr(), device) }).map(Server)
            });
            self.failed = self.server.is_none();
        }
        self.server.as_ref()
    }

    /// Whether the server runs (it starts at the first frame presented).
    pub fn is_publishing(&self) -> bool {
        self.server.is_some()
    }

    /// Whether another application reads the image.
    pub fn has_clients(&self) -> bool {
        self.server.as_ref().is_some_and(|server| {
            // SAFETY: a live server of this output.
            unsafe { prism_syphon_server_has_clients(server.0.as_ptr()) != 0 }
        })
    }
}

impl SyphonServer {
    /// Publishes `program`, the program image (16-bit float, `size`
    /// pixels): converted to 8-bit BGRA, then copied to the shared surface.
    pub fn publish(&mut self, gpu: &Gpu, program: &wgpu::TextureView, size: (u32, u32)) {
        if self.server(gpu).is_none() {
            return;
        }
        let Some(converted) = self.converter.convert(gpu, program, size) else {
            return;
        };
        let Some(server) = &self.server else {
            return;
        };
        // SAFETY: wgpu's Metal queue and the converted texture, borrowed
        // for the call; the copy is queued after the conversion submitted
        // above, on the same queue.
        unsafe {
            let (Some(queue), Some(texture)) =
                (gpu.queue.as_hal::<Metal>(), converted.as_hal::<Metal>())
            else {
                return;
            };
            prism_syphon_server_publish(
                server.0.as_ptr(),
                std::ptr::from_ref(texture.raw_handle())
                    .cast::<c_void>()
                    .cast_mut(),
                std::ptr::from_ref(queue.as_raw())
                    .cast::<c_void>()
                    .cast_mut(),
            );
        }
    }

    /// A client of this server, in this process, without the directory:
    /// to check what the server publishes.
    pub fn local_client(&self, gpu: &Gpu) -> Option<SyphonClient> {
        let server = self.server.as_ref()?;
        let device = metal_device(gpu)?;
        // SAFETY: a live server of this process and the Metal device.
        let raw = unsafe { prism_syphon_client_of_server(server.0.as_ptr(), device) };
        NonNull::new(raw).map(SyphonClient::from_raw)
    }
}

/// The image another application publishes over Syphon, copied into a
/// texture of wgpu at each update.
pub struct SyphonClient {
    raw: NonNull<c_void>,
    texture: Option<((u32, u32), wgpu::Texture, wgpu::TextureView)>,
}

// SAFETY: the client is used by one thread at a time (the render thread);
// Syphon clients may be used from any thread.
unsafe impl Send for SyphonClient {}

impl SyphonClient {
    fn from_raw(raw: NonNull<c_void>) -> Self {
        Self { raw, texture: None }
    }

    /// Connects to the server `name` of the application `app`, if it is
    /// published (see [`syphon_servers`]).
    pub fn connect(gpu: &Gpu, app: &str, name: &str) -> Option<Self> {
        let device = metal_device(gpu)?;
        let app = CString::new(app.replace('\0', "")).ok()?;
        let name = CString::new(name.replace('\0', "")).ok()?;
        // SAFETY: valid strings and Metal device.
        let raw = unsafe { prism_syphon_client_new(app.as_ptr(), name.as_ptr(), device) };
        NonNull::new(raw).map(Self::from_raw)
    }

    /// Whether the server is still there.
    pub fn is_valid(&self) -> bool {
        // SAFETY: a live client.
        unsafe { prism_syphon_client_is_valid(self.raw.as_ptr()) != 0 }
    }

    /// Copies the current image of the server into the texture of this
    /// client; returns whether there is an image. The copy is queued on the
    /// GPU queue: work submitted after this call sees it.
    pub fn update(&mut self, gpu: &Gpu) -> bool {
        let (mut width, mut height) = (0, 0);
        // SAFETY: a live client; the texture returned is released below.
        let frame =
            unsafe { prism_syphon_client_frame(self.raw.as_ptr(), &mut width, &mut height) };
        if frame.is_null() {
            return false;
        }
        let size = (width, height);
        if width > 0 && height > 0 && self.texture.as_ref().is_none_or(|(old, ..)| *old != size) {
            let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("syphon input"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Bgra8Unorm,
                usage: wgpu::TextureUsages::COPY_DST
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            self.texture = Some((size, texture, view));
        }
        let copied = match &self.texture {
            // SAFETY: the server's texture and ours, same format, on wgpu's
            // queue; the frame is released after the copy is queued.
            Some((_, texture, _)) => unsafe {
                match (gpu.queue.as_hal::<Metal>(), texture.as_hal::<Metal>()) {
                    (Some(queue), Some(destination)) => {
                        prism_syphon_copy(
                            frame,
                            std::ptr::from_ref(destination.raw_handle())
                                .cast::<c_void>()
                                .cast_mut(),
                            std::ptr::from_ref(queue.as_raw())
                                .cast::<c_void>()
                                .cast_mut(),
                        );
                        true
                    }
                    _ => false,
                }
            },
            None => false,
        };
        // SAFETY: releases the frame returned above, once.
        unsafe { prism_syphon_texture_release(frame) };
        copied
    }

    /// The last image copied: 8-bit BGRA.
    pub fn view(&self) -> Option<&wgpu::TextureView> {
        self.texture.as_ref().map(|(_, _, view)| view)
    }

    /// The texture of the last image copied.
    pub fn texture(&self) -> Option<&wgpu::Texture> {
        self.texture.as_ref().map(|(_, texture, _)| texture)
    }
}

impl Drop for SyphonClient {
    fn drop(&mut self) {
        // SAFETY: the pointer came from the glue and is released once.
        unsafe { prism_syphon_client_free(self.raw.as_ptr()) };
    }
}
