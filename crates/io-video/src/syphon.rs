//! Syphon output: the program image published under a name, for other
//! applications of the Mac (mapping tools, streaming, recording) to read.
//!
//! Syphon's own code (third_party/syphon, BSD licence) runs the protocol;
//! the glue in `syphon_glue.m` copies a BGRA texture into the shared
//! surface on the GPU queue of wgpu.

// Calls into the Objective-C glue. Every block states why it is sound.
#![allow(unsafe_code)]

use std::ffi::{CString, c_char, c_int, c_void};
use std::ptr::NonNull;

use prism_engine::{Output, ProgramOutput};
use prism_render::{Gpu, wgpu};
use wgpu::hal::api::Metal;

unsafe extern "C" {
    fn prism_syphon_server_new(name: *const c_char, device: *mut c_void) -> *mut c_void;
    fn prism_syphon_server_publish(server: *mut c_void, texture: *mut c_void, queue: *mut c_void);
    fn prism_syphon_server_has_clients(server: *mut c_void) -> c_int;
    fn prism_syphon_server_free(server: *mut c_void);
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

/// Image converted for Syphon, at the size of the program.
struct Target {
    size: (u32, u32),
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
}

/// The program image published over Syphon, as an output of the engine.
pub struct SyphonOutput {
    name: String,
    server: Option<Server>,
    pipeline: Option<(wgpu::RenderPipeline, wgpu::BindGroupLayout)>,
    target: Option<Target>,
    failed: bool,
}

// SAFETY: the server is created, used and dropped by the render thread
// only, after the output was sent to it; Syphon servers may be used from
// any thread.
unsafe impl Send for SyphonOutput {}

impl SyphonOutput {
    /// An output publishing under `name` once the engine presents to it.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            server: None,
            pipeline: None,
            target: None,
            failed: false,
        }
    }

    fn server(&mut self, gpu: &Gpu) -> Option<&Server> {
        if self.server.is_none() && !self.failed {
            // SAFETY: the Metal device of wgpu, borrowed for the call.
            let device = unsafe { gpu.device.as_hal::<Metal>() }.map(|device| {
                let raw: &wgpu::hal::metal::Device = &device;
                std::ptr::from_ref(&**raw.raw_device())
                    .cast::<c_void>()
                    .cast_mut()
            });
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

    fn convert(&mut self, gpu: &Gpu, program: &ProgramOutput) {
        let size = (program.resolution().width, program.resolution().height);
        let (pipeline, layout) = self.pipeline.get_or_insert_with(|| {
            let module = gpu
                .device
                .create_shader_module(wgpu::include_wgsl!("convert.wgsl"));
            let layout = gpu
                .device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("syphon"),
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    }],
                });
            let pipeline_layout =
                gpu.device
                    .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: Some("syphon"),
                        bind_group_layouts: &[Some(&layout)],
                        immediate_size: 0,
                    });
            let pipeline = gpu
                .device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("syphon"),
                    layout: Some(&pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &module,
                        entry_point: Some("vs_main"),
                        compilation_options: Default::default(),
                        buffers: &[],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &module,
                        entry_point: Some("fs_main"),
                        compilation_options: Default::default(),
                        targets: &[Some(wgpu::TextureFormat::Bgra8Unorm.into())],
                    }),
                    primitive: wgpu::PrimitiveState::default(),
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState::default(),
                    multiview_mask: None,
                    cache: None,
                });
            (pipeline, layout)
        });
        if self
            .target
            .as_ref()
            .is_none_or(|target| target.size != size)
        {
            let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("syphon"),
                size: wgpu::Extent3d {
                    width: size.0,
                    height: size.1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Bgra8Unorm,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("syphon"),
                layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(program.view()),
                }],
            });
            self.target = Some(Target {
                size,
                texture,
                bind_group,
            });
        }
        let Some(target) = &self.target else {
            return;
        };
        let view = target
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("syphon"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &target.bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
        gpu.queue.submit([encoder.finish()]);
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

impl Output for SyphonOutput {
    fn present(&mut self, gpu: &Gpu, program: &ProgramOutput) {
        if self.server(gpu).is_none() {
            return;
        }
        // The program image may have been replaced (new resolution): the
        // conversion reads it again.
        if self.target.as_ref().is_some_and(|target| {
            target.size != (program.resolution().width, program.resolution().height)
        }) {
            self.target = None;
        }
        self.convert(gpu, program);
        let (Some(server), Some(target)) = (&self.server, &self.target) else {
            return;
        };
        // SAFETY: wgpu's Metal queue and the converted texture, borrowed
        // for the call; the copy is queued after the conversion submitted
        // above, on the same queue.
        unsafe {
            let (Some(queue), Some(texture)) = (
                gpu.queue.as_hal::<Metal>(),
                target.texture.as_hal::<Metal>(),
            ) else {
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
}
