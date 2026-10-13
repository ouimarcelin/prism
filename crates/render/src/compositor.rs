//! Layer compositing on the GPU.

use std::num::NonZeroU64;

use prism_core::{BlendMode, Resolution};

use crate::WORKING_FORMAT;

/// One layer to composite: its current frame and how to mix it.
#[derive(Clone, Copy, Debug)]
pub struct LayerInput<'a> {
    /// Current frame of the layer, premultiplied, in a non-sRGB format so
    /// values are sampled as stored. Stretched to the composition size.
    pub texture: &'a wgpu::TextureView,
    /// Opacity in `0.0..=1.0`, crossfader and solo already applied.
    pub opacity: f32,
    /// Blend mode against the layers below.
    pub blend_mode: BlendMode,
    /// Clip being replaced, while the layer fades from one clip to the next.
    pub transition: Option<Transition<'a>>,
}

impl<'a> LayerInput<'a> {
    /// A layer showing one frame, without transition.
    pub fn new(texture: &'a wgpu::TextureView, opacity: f32, blend_mode: BlendMode) -> Self {
        Self {
            texture,
            opacity,
            blend_mode,
            transition: None,
        }
    }
}

/// Fade between the previous clip of a layer and its new clip. The layer
/// shows `previous` mixed with the new frame by `progress`, before blending.
#[derive(Clone, Copy, Debug)]
pub struct Transition<'a> {
    /// Current frame of the clip being replaced.
    pub previous: &'a wgpu::TextureView,
    /// `0.0` shows only `previous`, `1.0` only the new clip.
    pub progress: f32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct LayerParams {
    opacity: f32,
    blend_mode: u32,
    progress: f32,
    _pad: u32,
}

fn unit(value: f32) -> f32 {
    if value.is_nan() {
        0.0
    } else {
        value.clamp(0.0, 1.0)
    }
}

const PARAMS_SIZE: u64 = std::mem::size_of::<LayerParams>() as u64;

fn blend_code(mode: BlendMode) -> u32 {
    // Must match the constants of composite.wgsl.
    match mode {
        BlendMode::Normal => 0,
        BlendMode::Add => 1,
        BlendMode::Multiply => 2,
        BlendMode::Screen => 3,
    }
}

struct Target {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
}

impl Target {
    fn new(device: &wgpu::Device, resolution: Resolution, label: &str) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: resolution.width,
                height: resolution.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: WORKING_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self { texture, view }
    }
}

/// Composites layers, bottom to top, over opaque black.
///
/// Two working textures at the composition size alternate: each layer pass
/// reads the result so far from one and writes the new result to the other.
pub struct Compositor {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    params: wgpu::Buffer,
    params_capacity: usize,
    params_stride: u64,
    targets: [Target; 2],
    output: usize,
    resolution: Resolution,
}

impl Compositor {
    /// Creates a compositor producing images of `resolution`.
    pub fn new(device: &wgpu::Device, resolution: Resolution) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("composite.wgsl"));
        let texture_entry = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("composite"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: NonZeroU64::new(PARAMS_SIZE),
                    },
                    count: None,
                },
                texture_entry(1),
                texture_entry(2),
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                texture_entry(4),
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("composite"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("composite"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: WORKING_FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("layer"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let alignment = u64::from(device.limits().min_uniform_buffer_offset_alignment);
        let params_stride = PARAMS_SIZE.div_ceil(alignment) * alignment;
        let params_capacity = 8;
        let params = Self::create_params_buffer(device, params_capacity, params_stride);

        Self {
            pipeline,
            bind_group_layout,
            sampler,
            params,
            params_capacity,
            params_stride,
            targets: Self::create_targets(device, resolution),
            output: 0,
            resolution,
        }
    }

    fn create_params_buffer(device: &wgpu::Device, capacity: usize, stride: u64) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("layer params"),
            size: capacity as u64 * stride,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    fn create_targets(device: &wgpu::Device, resolution: Resolution) -> [Target; 2] {
        [
            Target::new(device, resolution, "composite A"),
            Target::new(device, resolution, "composite B"),
        ]
    }

    /// Size of the produced images.
    pub fn resolution(&self) -> Resolution {
        self.resolution
    }

    /// Changes the size of the produced images.
    pub fn resize(&mut self, device: &wgpu::Device, resolution: Resolution) {
        if resolution != self.resolution {
            self.targets = Self::create_targets(device, resolution);
            self.output = 0;
            self.resolution = resolution;
        }
    }

    /// Composites `layers`, bottom first, over opaque black, and returns the
    /// texture holding the result. It stays valid until the next call.
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layers: &[LayerInput<'_>],
    ) -> &wgpu::Texture {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("composite"),
        });
        self.encode(device, queue, &mut encoder, layers);
        queue.submit([encoder.finish()]);
        self.output()
    }

    /// Records the compositing of `layers` into `encoder`. The result is in
    /// [`Compositor::output`] once the encoder is submitted. Call it at most
    /// once per submission: layer parameters are written when it is submitted.
    pub fn encode(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        layers: &[LayerInput<'_>],
    ) {
        if layers.len() > self.params_capacity {
            self.params_capacity = layers.len().next_power_of_two();
            self.params =
                Self::create_params_buffer(device, self.params_capacity, self.params_stride);
        }
        for (index, layer) in layers.iter().enumerate() {
            let params = LayerParams {
                opacity: unit(layer.opacity),
                blend_mode: blend_code(layer.blend_mode),
                progress: layer.transition.map_or(1.0, |t| unit(t.progress)),
                _pad: 0,
            };
            queue.write_buffer(
                &self.params,
                index as u64 * self.params_stride,
                bytemuck::bytes_of(&params),
            );
        }

        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("background"),
            color_attachments: &[Some(color_attachment(
                &self.targets[0].view,
                wgpu::Color::BLACK,
            ))],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });

        let mut current = 0;
        for (index, layer) in layers.iter().enumerate() {
            let next = 1 - current;
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("composite layer"),
                layout: &self.bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: &self.params,
                            offset: 0,
                            size: NonZeroU64::new(PARAMS_SIZE),
                        }),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&self.targets[current].view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(layer.texture),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: wgpu::BindingResource::TextureView(
                            layer.transition.map_or(layer.texture, |t| t.previous),
                        ),
                    },
                ],
            });
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("composite layer"),
                color_attachments: &[Some(color_attachment(
                    &self.targets[next].view,
                    wgpu::Color::TRANSPARENT,
                ))],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            let offset = u32::try_from(index as u64 * self.params_stride)
                .expect("layer parameters exceed the uniform offset range");
            pass.set_bind_group(0, &bind_group, &[offset]);
            pass.draw(0..3, 0..1);
            drop(pass);
            current = next;
        }
        self.output = current;
    }

    /// Result of the last [`Compositor::render`] or [`Compositor::encode`].
    pub fn output(&self) -> &wgpu::Texture {
        &self.targets[self.output].texture
    }

    /// View of the result of the last [`Compositor::render`] or [`Compositor::encode`].
    pub fn output_view(&self) -> &wgpu::TextureView {
        &self.targets[self.output].view
    }
}

fn color_attachment(
    view: &wgpu::TextureView,
    clear: wgpu::Color,
) -> wgpu::RenderPassColorAttachment<'_> {
    wgpu::RenderPassColorAttachment {
        view,
        depth_slice: None,
        resolve_target: None,
        ops: wgpu::Operations {
            load: wgpu::LoadOp::Clear(clear),
            store: wgpu::StoreOp::Store,
        },
    }
}
