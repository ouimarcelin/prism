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

/// Layers mixed in one pass. Must match the size of `Batch::layers` in
/// composite.wgsl.
const LAYERS_PER_PASS: usize = 4;

#[repr(C)]
#[derive(Clone, Copy, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
struct LayerParams {
    opacity: f32,
    blend_mode: u32,
    progress: f32,
    _pad: u32,
}

/// Settings of one pass, laid out as `Batch` in composite.wgsl.
#[repr(C)]
#[derive(Clone, Copy, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
struct PassParams {
    layers: [LayerParams; LAYERS_PER_PASS],
    count: u32,
    over_black: u32,
    _pad: [u32; 2],
}

fn unit(value: f32) -> f32 {
    if value.is_nan() {
        0.0
    } else {
        value.clamp(0.0, 1.0)
    }
}

const PARAMS_SIZE: u64 = std::mem::size_of::<PassParams>() as u64;

fn blend_code(mode: BlendMode) -> u32 {
    // Must match the constants of composite.wgsl.
    match mode {
        BlendMode::Normal => 0,
        BlendMode::Add => 1,
        BlendMode::Multiply => 2,
        BlendMode::Screen => 3,
        BlendMode::Lighten => 4,
        BlendMode::Darken => 5,
        BlendMode::Difference => 6,
        BlendMode::Subtract => 7,
        BlendMode::Overlay => 8,
        BlendMode::Exclusion => 9,
    }
}

/// Value of `over_black` for the first pass: start from opaque black, or
/// from transparent black. Must match composite.wgsl.
const OVER_BLACK: u32 = 1;
const OVER_TRANSPARENT: u32 = 2;

fn pass_params(layers: &[LayerInput<'_>], first: Option<u32>) -> PassParams {
    let mut params = PassParams {
        count: layers.len() as u32,
        over_black: first.unwrap_or(0),
        ..PassParams::default()
    };
    for (slot, layer) in params.layers.iter_mut().zip(layers) {
        *slot = LayerParams {
            opacity: unit(layer.opacity),
            blend_mode: blend_code(layer.blend_mode),
            progress: layer.transition.map_or(1.0, |t| unit(t.progress)),
            _pad: 0,
        };
    }
    params
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

/// Composites layers, bottom to top, over opaque black, or over
/// transparent black for [`Compositor::over_transparent`].
///
/// One pass mixes up to four layers, keeping the result in 32-bit registers
/// from one layer to the next, and the last pass writes straight into the
/// destination. With more layers, two working textures at the composition
/// size carry the result from one pass to the next.
pub struct Compositor {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    params: wgpu::Buffer,
    params_capacity: usize,
    params_stride: u64,
    /// Settings in the buffer, by pass: only changes are written.
    params_written: Vec<PassParams>,
    /// Bound in place of the textures a pass does not read.
    blank: wgpu::TextureView,
    /// Results between passes, created when more than one pass is needed.
    targets: Option<[Target; 2]>,
    /// Destination of [`Compositor::render`], created on first use.
    result: Option<Target>,
    resolution: Resolution,
    /// `over_black` of the first pass.
    base: u32,
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
        let mut entries = vec![
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
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ];
        // Current frame and previous clip of each layer of a pass.
        entries.extend((0..2 * LAYERS_PER_PASS as u32).map(|index| texture_entry(3 + index)));
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("composite"),
            entries: &entries,
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
        let blank = device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("composite blank"),
                size: wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: WORKING_FORMAT,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&wgpu::TextureViewDescriptor::default());

        let alignment = u64::from(device.limits().min_uniform_buffer_offset_alignment);
        let params_stride = PARAMS_SIZE.div_ceil(alignment) * alignment;
        let params_capacity = 4;
        let params = Self::create_params_buffer(device, params_capacity, params_stride);

        Self {
            pipeline,
            bind_group_layout,
            sampler,
            params,
            params_capacity,
            params_stride,
            params_written: Vec::new(),
            blank,
            targets: None,
            result: None,
            resolution,
            base: OVER_BLACK,
        }
    }

    /// The same compositor, starting from transparent black: the result
    /// keeps the transparency of the layers, to be mixed in turn.
    pub fn over_transparent(mut self) -> Self {
        self.base = OVER_TRANSPARENT;
        self
    }

    fn create_params_buffer(device: &wgpu::Device, capacity: usize, stride: u64) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("composite params"),
            size: capacity as u64 * stride,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    /// Size of the produced images.
    pub fn resolution(&self) -> Resolution {
        self.resolution
    }

    /// Changes the size of the produced images.
    pub fn resize(&mut self, resolution: Resolution) {
        if resolution != self.resolution {
            self.targets = None;
            self.result = None;
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
        let resolution = self.resolution;
        let destination = self
            .result
            .get_or_insert_with(|| Target::new(device, resolution, "composite result"))
            .view
            .clone();
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("composite"),
        });
        self.encode(device, queue, &mut encoder, layers, &destination);
        queue.submit([encoder.finish()]);
        &self
            .result
            .as_ref()
            .expect("created at the start of the call")
            .texture
    }

    /// Records, in `encoder`, the compositing of `layers` into
    /// `destination`: a [`WORKING_FORMAT`] view of the composition size,
    /// usable as a render attachment, read by none of the layers.
    ///
    /// The settings of the layers are written when the encoder is
    /// submitted: call it at most once per submission.
    pub fn encode(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        layers: &[LayerInput<'_>],
        destination: &wgpu::TextureView,
    ) {
        let passes = layers.len().div_ceil(LAYERS_PER_PASS).max(1);
        self.write_params(device, queue, layers, passes);
        if passes > 1 && self.targets.is_none() {
            self.targets = Some([
                Target::new(device, self.resolution, "composite A"),
                Target::new(device, self.resolution, "composite B"),
            ]);
        }

        for pass_index in 0..passes {
            let batch = layers
                .chunks(LAYERS_PER_PASS)
                .nth(pass_index)
                .unwrap_or_default();
            let between = self.targets.as_ref();
            // The first pass starts from black; the others from the result
            // of the pass before. The last one writes to the destination.
            let below = match (pass_index, between) {
                (0, _) | (_, None) => &self.blank,
                (index, Some(targets)) => &targets[(index - 1) % 2].view,
            };
            let target = match between {
                Some(targets) if pass_index + 1 < passes => &targets[pass_index % 2].view,
                _ => destination,
            };
            let mut entries = vec![
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
                    resource: wgpu::BindingResource::TextureView(below),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ];
            for slot in 0..LAYERS_PER_PASS {
                let (current, previous) =
                    batch.get(slot).map_or((&self.blank, &self.blank), |layer| {
                        (
                            layer.texture,
                            layer.transition.map_or(layer.texture, |t| t.previous),
                        )
                    });
                let binding = 3 + 2 * slot as u32;
                entries.push(wgpu::BindGroupEntry {
                    binding,
                    resource: wgpu::BindingResource::TextureView(current),
                });
                entries.push(wgpu::BindGroupEntry {
                    binding: binding + 1,
                    resource: wgpu::BindingResource::TextureView(previous),
                });
            }
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("composite pass"),
                layout: &self.bind_group_layout,
                entries: &entries,
            });
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("composite pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
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
            pass.set_pipeline(&self.pipeline);
            let offset = u32::try_from(pass_index as u64 * self.params_stride)
                .expect("pass settings exceed the uniform offset range");
            pass.set_bind_group(0, &bind_group, &[offset]);
            pass.draw(0..3, 0..1);
        }
    }

    /// Writes the settings of each pass that changed since the last frame.
    fn write_params(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layers: &[LayerInput<'_>],
        passes: usize,
    ) {
        if passes > self.params_capacity {
            self.params_capacity = passes.next_power_of_two();
            self.params =
                Self::create_params_buffer(device, self.params_capacity, self.params_stride);
            self.params_written.clear();
        }
        for pass_index in 0..passes {
            let batch = layers
                .chunks(LAYERS_PER_PASS)
                .nth(pass_index)
                .unwrap_or_default();
            let params = pass_params(batch, (pass_index == 0).then_some(self.base));
            if self.params_written.get(pass_index) == Some(&params) {
                continue;
            }
            queue.write_buffer(
                &self.params,
                pass_index as u64 * self.params_stride,
                bytemuck::bytes_of(&params),
            );
            match self.params_written.get_mut(pass_index) {
                Some(written) => *written = params,
                None => self.params_written.push(params),
            }
        }
    }
}
