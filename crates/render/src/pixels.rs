//! Pixel mapping on the GPU: the colours of the program image at the
//! points of the fixtures, read back without waiting for the GPU.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use prism_core::Point;

/// Texels per row of the colour image.
const ROW: u32 = 256;
/// Read backs in flight.
const IN_FLIGHT: usize = 3;
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

struct Readback {
    buffer: wgpu::Buffer,
    count: usize,
    ready: Arc<AtomicBool>,
    /// Copied to, waiting to be read.
    pending: bool,
    /// Copied to by an encoder not yet submitted: to map after submitting.
    unmapped: bool,
}

/// Takes the colours of the program at points, a few frames late.
pub struct PixelSampler {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    /// Points, their buffer and the colour image, while their count holds.
    target: Option<(Vec<Point>, wgpu::Buffer, wgpu::Texture)>,
    readbacks: Vec<Readback>,
    next: usize,
}

fn rows(count: usize) -> u32 {
    (count as u32).div_ceil(ROW).max(1)
}

impl PixelSampler {
    /// Creates the sampler.
    pub fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("pixels.wgsl"));
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("pixels"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pixels"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("pixels"),
            layout: Some(&pipeline_layout),
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
                targets: &[Some(FORMAT.into())],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("pixels"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });
        Self {
            pipeline,
            layout,
            sampler,
            target: None,
            readbacks: Vec::new(),
            next: 0,
        }
    }

    /// Records the sampling of `program` at `points` (fractions of the
    /// image) and its copy for reading; nothing when every buffer is still
    /// in flight.
    pub fn sample(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        program: &wgpu::TextureView,
        points: &[Point],
    ) {
        if points.is_empty() {
            return;
        }
        let count = points.len();
        if self
            .target
            .as_ref()
            .is_none_or(|(known, _, _)| known.len() != count)
        {
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("pixel points"),
                size: (count * 8) as u64,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("pixel colours"),
                size: wgpu::Extent3d {
                    width: ROW,
                    height: rows(count),
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            self.target = Some((Vec::new(), buffer, texture));
            self.readbacks.clear();
        }
        let Some((known, buffer, texture)) = &mut self.target else {
            return;
        };
        if known.as_slice() != points {
            let bytes: Vec<u8> = points
                .iter()
                .flat_map(|point| [point.x, point.y])
                .flat_map(f32::to_le_bytes)
                .collect();
            queue.write_buffer(buffer, 0, &bytes);
            *known = points.to_vec();
        }
        // A free read back buffer, or this frame is not sampled.
        if self.readbacks.len() < IN_FLIGHT {
            self.readbacks.push(Readback {
                buffer: device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("pixel readback"),
                    size: u64::from(ROW * 4 * rows(count)),
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                    mapped_at_creation: false,
                }),
                count,
                ready: Arc::default(),
                pending: false,
                unmapped: false,
            });
        }
        let index = self.next % self.readbacks.len();
        if self.readbacks[index].pending {
            return;
        }
        self.next = self.next.wrapping_add(1);
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("pixels"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(program),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: buffer.as_entire_binding(),
                },
            ],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("pixels"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
        let readback = &mut self.readbacks[index];
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &readback.buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(ROW * 4),
                    rows_per_image: Some(rows(count)),
                },
            },
            texture.size(),
        );
        readback.count = count;
        readback.pending = true;
        readback.unmapped = true;
        readback.ready.store(false, Ordering::Release);
    }

    /// Asks for the buffers copied by the last submission to be mapped.
    /// Call after submitting the encoder given to [`PixelSampler::sample`].
    pub fn map_submitted(&mut self) {
        for readback in &mut self.readbacks {
            if readback.unmapped {
                readback.unmapped = false;
                let ready = Arc::clone(&readback.ready);
                readback
                    .buffer
                    .slice(..)
                    .map_async(wgpu::MapMode::Read, move |result| {
                        ready.store(result.is_ok(), Ordering::Release);
                    });
            }
        }
    }

    /// The newest colours read back since the last call, as RGB.
    pub fn read(&mut self, device: &wgpu::Device) -> Option<Vec<[u8; 3]>> {
        let _ = device.poll(wgpu::PollType::Poll);
        let mut newest = None;
        let count = self.readbacks.len();
        for offset in 0..count {
            let index = (self.next + offset) % count;
            let readback = &mut self.readbacks[index];
            if !readback.pending || !readback.ready.load(Ordering::Acquire) {
                continue;
            }
            if let Ok(mapped) = readback.buffer.slice(..).get_mapped_range() {
                newest = Some(
                    mapped
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .take(readback.count)
                        .map(|texel| [texel[0], texel[1], texel[2]])
                        .collect(),
                );
            }
            readback.buffer.unmap();
            readback.pending = false;
            readback.ready.store(false, Ordering::Release);
        }
        newest
    }
}
