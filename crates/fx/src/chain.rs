//! Chains of effects on the GPU.

use std::collections::HashMap;
use std::num::NonZeroU64;
use std::sync::Mutex;

use prism_core::{BAND_COUNT, DASHBOARD_SIZE, Effect, Moment};
use prism_render::{WORKING_FORMAT, wgpu};

use crate::catalog::{CATALOG, EffectInfo, GENERATORS, MAX_PARAMS, info_for};
use crate::isf::ISF_KIND;
use crate::text::{Align, TEXT_KIND, TextCache};

const PRELUDE: &str = include_str!("shaders/prelude.wgsl");

/// Settings of one pass, laid out as `Params` in prelude.wgsl.
#[repr(C)]
#[derive(Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
struct PassParams {
    values: [f32; MAX_PARAMS],
    size: [f32; 2],
    time: f32,
    beat: f32,
    /// Line height of the text mask the pass reads, in pixels; `0.0` for
    /// passes without one.
    mask_line: f32,
    _padding: [f32; 3],
}

const PARAMS_SIZE: u64 = std::mem::size_of::<PassParams>() as u64;

/// What settings follow at a frame: the time, for effects and settings that
/// move on their own, and the dashboard of the clip.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FrameTime {
    /// Seconds since the engine started.
    pub seconds: f64,
    /// Beat position of the tempo clock.
    pub beat: f64,
    /// Beat position when the clip the chain belongs to was launched, for
    /// envelopes; the last launch of any clip for the composition.
    pub launch_beat: f64,
    /// Seconds since the engine started, when the clip was launched.
    pub launch_seconds: f64,
    /// Sliders of the dashboard of the clip the chain belongs to, from
    /// `0.0` to `1.0`; all at `0.0` for the composition.
    pub dashboard: [f32; DASHBOARD_SIZE],
    /// Levels of the sound coming in, from `0.0` to `1.0`, in the order of
    /// [`prism_core::Band::ALL`].
    pub audio: [f32; BAND_COUNT],
}

impl FrameTime {
    /// Where the clocks stand, for animations.
    pub fn moment(&self) -> Moment {
        Moment::new(self.beat, self.seconds).launched_at(self.launch_beat, self.launch_seconds)
    }
}

/// Pipelines of every effect of the catalog, shared by all chains.
pub struct EffectRenderer {
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    /// By effect of the catalog, then by pass.
    pipelines: Vec<Vec<wgpu::RenderPipeline>>,
    /// Effects read from files, made when first used, by address of their
    /// description.
    loaded: Mutex<HashMap<usize, wgpu::RenderPipeline>>,
    layout: wgpu::PipelineLayout,
    /// The prelude alone: vertex stage of effects read from files.
    vertex: wgpu::ShaderModule,
    params_stride: u64,
}

impl EffectRenderer {
    /// Compiles every effect of the catalog.
    pub fn new(device: &wgpu::Device) -> Self {
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("effect"),
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
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("effect"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });
        let pipelines = CATALOG
            .iter()
            .chain(GENERATORS)
            .map(|info| {
                let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some(info.kind),
                    source: wgpu::ShaderSource::Wgsl(format!("{PRELUDE}\n{}", info.source).into()),
                });
                info.passes
                    .iter()
                    .map(|entry| {
                        render_pipeline(device, &layout, info.kind, &module, &module, entry)
                    })
                    .collect()
            })
            .collect();
        let vertex = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("effect vertex"),
            source: wgpu::ShaderSource::Wgsl(PRELUDE.into()),
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("effect"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let alignment = u64::from(device.limits().min_uniform_buffer_offset_alignment);
        Self {
            bind_group_layout,
            sampler,
            pipelines,
            loaded: Mutex::default(),
            layout,
            vertex,
            params_stride: PARAMS_SIZE.div_ceil(alignment) * alignment,
        }
    }

    fn pipeline(
        &self,
        device: &wgpu::Device,
        info: &'static EffectInfo,
        pass: usize,
    ) -> Option<wgpu::RenderPipeline> {
        if info.kind == ISF_KIND {
            // One pass, compiled the first time it is drawn.
            let mut loaded = self
                .loaded
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let pipeline = loaded
                .entry(std::ptr::from_ref(info) as usize)
                .or_insert_with(|| {
                    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                        label: Some(info.label),
                        source: wgpu::ShaderSource::Wgsl(info.source.into()),
                    });
                    render_pipeline(
                        device,
                        &self.layout,
                        info.label,
                        &self.vertex,
                        &module,
                        crate::isf::ENTRY_POINT,
                    )
                });
            return Some(pipeline.clone());
        }
        let index = CATALOG
            .iter()
            .chain(GENERATORS)
            .position(|known| known.kind == info.kind)?;
        self.pipelines.get(index)?.get(pass).cloned()
    }
}

/// A full-screen pass of an effect: `vs_main` of `vertex`, `entry` of
/// `fragment`.
fn render_pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    label: &str,
    vertex: &wgpu::ShaderModule,
    fragment: &wgpu::ShaderModule,
    entry: &str,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: vertex,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: fragment,
            entry_point: Some(entry),
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
    })
}

/// Whether any of `effects` changes the image: not bypassed, and of a kind
/// this build knows.
pub fn is_active(effects: &[&Effect]) -> bool {
    effects
        .iter()
        .any(|effect| !effect.bypass && info_for(effect).is_some())
}

struct Target {
    view: wgpu::TextureView,
    size: (u32, u32),
}

impl Target {
    fn new(device: &wgpu::Device, size: (u32, u32)) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("effect"),
            size: wgpu::Extent3d {
                width: size.0,
                height: size.1,
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
        Self {
            view: texture.create_view(&wgpu::TextureViewDescriptor::default()),
            size,
        }
    }
}

/// Intermediate images and settings of one chain of effects: one per
/// clip playing, one for the composition.
#[derive(Default)]
pub struct EffectChain {
    targets: Vec<Target>,
    params: Option<wgpu::Buffer>,
    params_capacity: usize,
    written: Vec<PassParams>,
    output: Option<usize>,
    text: TextCache,
}

/// One pass to run.
struct Pass<'a> {
    info: &'static EffectInfo,
    index: usize,
    values: [f32; MAX_PARAMS],
    effect: &'a Effect,
    /// For the text generator: its mask and line height in pixels.
    mask: Option<(wgpu::TextureView, f32)>,
}

impl EffectChain {
    /// Records, in `encoder`, `effects` applied to `input`, an image of
    /// `size` pixels. Returns the result, or `None` when no effect changes
    /// the image: `input` stays the result.
    ///
    /// Settings are written when the encoder is submitted: use a chain at
    /// most once per submission.
    #[allow(clippy::too_many_arguments)]
    pub fn apply(
        &mut self,
        renderer: &EffectRenderer,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        effects: &[&Effect],
        input: &wgpu::TextureView,
        size: (u32, u32),
        time: FrameTime,
    ) -> Option<&wgpu::TextureView> {
        self.run(
            renderer, device, queue, encoder, effects, input, size, time, None,
        );
        self.output()
    }

    /// Result of the last [`EffectChain::apply`], if an effect changed the
    /// image.
    pub fn output(&self) -> Option<&wgpu::TextureView> {
        self.output
            .and_then(|index| self.targets.get(index))
            .map(|target| &target.view)
    }

    /// Like [`EffectChain::apply`], with the result written to
    /// `destination`, of `size` pixels and usable as a render attachment.
    /// Returns whether it was written: without an active effect, nothing is
    /// recorded.
    #[allow(clippy::too_many_arguments)]
    pub fn apply_into(
        &mut self,
        renderer: &EffectRenderer,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        effects: &[&Effect],
        input: &wgpu::TextureView,
        size: (u32, u32),
        time: FrameTime,
        destination: &wgpu::TextureView,
    ) -> bool {
        self.run(
            renderer,
            device,
            queue,
            encoder,
            effects,
            input,
            size,
            time,
            Some(destination),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn run(
        &mut self,
        renderer: &EffectRenderer,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        effects: &[&Effect],
        input: &wgpu::TextureView,
        size: (u32, u32),
        time: FrameTime,
        destination: Option<&wgpu::TextureView>,
    ) -> bool {
        self.output = None;
        let passes: Vec<Pass> = effects
            .iter()
            .filter(|effect| !effect.bypass)
            .filter_map(|effect| {
                let info = info_for(effect)?;
                Some((info, info.values_at(effect, time), *effect))
            })
            .flat_map(|(info, values, effect)| {
                (0..info.passes.len()).map(move |index| Pass {
                    info,
                    index,
                    values,
                    effect,
                    mask: None,
                })
            })
            .collect();
        if passes.is_empty() || size.0 == 0 || size.1 == 0 {
            return false;
        }
        // The text generator reads its text, rasterized, instead of the
        // image so far.
        let mut passes = passes;
        for pass in &mut passes {
            if pass.info.kind == TEXT_KIND {
                let [line, font, align] = [0, 6, 7].map(|index| pass.values[index]);
                let align = match align as u32 {
                    0 => Align::Center,
                    1 => Align::Left,
                    _ => Align::Right,
                };
                pass.mask = self
                    .text
                    .view(
                        device,
                        queue,
                        pass.effect,
                        line,
                        size.1,
                        font as usize,
                        align,
                    )
                    .map(|(view, line)| (view.clone(), line as f32));
            }
        }
        self.write_params(renderer, device, queue, &passes, size, time);
        // Two images in turn hold the result between passes.
        let needed = if destination.is_some() {
            (passes.len() - 1).min(2)
        } else {
            passes.len().min(2)
        };
        if self.targets.iter().any(|target| target.size != size) {
            self.targets.clear();
        }
        while self.targets.len() < needed {
            self.targets.push(Target::new(device, size));
        }
        let Some(params) = &self.params else {
            return false;
        };

        let last = passes.len() - 1;
        for (index, pass) in passes.iter().enumerate() {
            let Some(pipeline) = renderer.pipeline(device, pass.info, pass.index) else {
                continue;
            };
            let source = match (&pass.mask, index) {
                (Some((mask, _)), _) => mask,
                (None, 0) => input,
                (None, _) => &self.targets[(index - 1) % 2].view,
            };
            let target = match destination {
                Some(destination) if index == last => destination,
                _ => &self.targets[index % 2].view,
            };
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(pass.info.kind),
                layout: &renderer.bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: params,
                            offset: 0,
                            size: NonZeroU64::new(PARAMS_SIZE),
                        }),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(source),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&renderer.sampler),
                    },
                ],
            });
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(pass.info.kind),
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
            render_pass.set_pipeline(&pipeline);
            let offset = u32::try_from(index as u64 * renderer.params_stride)
                .expect("effect settings exceed the uniform offset range");
            render_pass.set_bind_group(0, &bind_group, &[offset]);
            render_pass.draw(0..3, 0..1);
        }
        if destination.is_none() {
            self.output = Some(last % 2);
        }
        true
    }

    /// Writes the settings of each pass that changed since the last frame.
    fn write_params(
        &mut self,
        renderer: &EffectRenderer,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        passes: &[Pass],
        size: (u32, u32),
        time: FrameTime,
    ) {
        if self.params.is_none() || passes.len() > self.params_capacity {
            self.params_capacity = passes.len().next_power_of_two().max(4);
            self.params = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("effect params"),
                size: self.params_capacity as u64 * renderer.params_stride,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
            self.written.clear();
        }
        let Some(buffer) = &self.params else {
            return;
        };
        for (index, pass) in passes.iter().enumerate() {
            let params = PassParams {
                values: pass.values,
                size: [size.0 as f32, size.1 as f32],
                time: time.seconds as f32,
                beat: time.beat as f32,
                mask_line: pass.mask.as_ref().map_or(0.0, |(_, line)| *line),
                _padding: [0.0; 3],
            };
            if self.written.get(index) == Some(&params) {
                continue;
            }
            queue.write_buffer(
                buffer,
                index as u64 * renderer.params_stride,
                bytemuck::bytes_of(&params),
            );
            match self.written.get_mut(index) {
                Some(written) => *written = params,
                None => self.written.push(params),
            }
        }
    }
}
