//! Drawing of the screens of the stage: each slice of the composition
//! placed on its screen through its four corners, in perspective.

use std::collections::HashMap;

use prism_core::{Flip, Mask, Point, Screen, Slice, SliceId};

use crate::WORKING_FORMAT;

/// A corner of a slice as drawn: position on the screen (fractions from the
/// top left) and position in the composition, multiplied by `q` with `q`
/// last, so the GPU interpolates in perspective.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SliceVertex {
    /// Position on the screen.
    pub position: [f32; 2],
    /// Position in the composition times `q`, then `q`.
    pub source: [f32; 3],
    /// Position in the slice (fractions across and down) times `q`.
    pub local: [f32; 2],
}

/// Weights making a quad's texture follow its perspective: for each corner,
/// `(d + d') / d'`, where `d` is its distance to the crossing of the
/// diagonals and `d'` that of the opposite corner. A quad whose diagonals
/// do not cross inside it gets no perspective.
pub fn perspective_weights(corners: &[Point; 4]) -> [f32; 4] {
    let [p0, p1, p2, p3] = corners.map(|point| (f64::from(point.x), f64::from(point.y)));
    // p0 + t (p2 - p0) = p1 + s (p3 - p1)
    let (ax, ay) = (p2.0 - p0.0, p2.1 - p0.1);
    let (bx, by) = (p3.0 - p1.0, p3.1 - p1.1);
    let denominator = ax * by - ay * bx;
    if denominator.abs() < 1e-12 {
        return [1.0; 4];
    }
    let (cx, cy) = (p1.0 - p0.0, p1.1 - p0.1);
    let t = (cx * by - cy * bx) / denominator;
    let s = (cx * ay - cy * ax) / denominator;
    if !(0.0..=1.0).contains(&t) || !(0.0..=1.0).contains(&s) || t == 0.0 || t == 1.0 {
        return [1.0; 4];
    }
    if s == 0.0 || s == 1.0 {
        return [1.0; 4];
    }
    // Distances along each diagonal, as fractions of it.
    let (d0, d2) = (t, 1.0 - t);
    let (d1, d3) = (s, 1.0 - s);
    [
        ((d0 + d2) / d2) as f32,
        ((d1 + d3) / d3) as f32,
        ((d2 + d0) / d0) as f32,
        ((d3 + d1) / d1) as f32,
    ]
}

/// Steps across or down a warped slice per cell of its grid, so curves
/// look smooth.
const STEPS_PER_CELL: usize = 4;
/// Fewest and most steps across or down a warped slice.
const STEPS: (usize, usize) = (32, 256);

/// The triangles drawing `slice`: two for a quad in perspective, a fine
/// mesh following its warp grid when it has one.
/// `aspect` is the composition's width over its height.
pub fn slice_vertices(slice: &Slice, aspect: f32) -> Vec<SliceVertex> {
    match &slice.warp {
        Some(warp) if warp.is_valid() => warp_vertices(slice, warp, aspect),
        _ => quad_vertices(slice, aspect).to_vec(),
    }
}

fn warp_vertices(slice: &Slice, warp: &prism_core::Warp, aspect: f32) -> Vec<SliceVertex> {
    let (columns, rows) = warp.cells();
    let steps = |cells: u8| (usize::from(cells) * STEPS_PER_CELL).clamp(STEPS.0, STEPS.1);
    let (across, down) = (steps(columns), steps(rows));
    let grid: Vec<SliceVertex> = (0..=down)
        .flat_map(|row| {
            (0..=across).map(move |column| {
                let (u, v) = (column as f32 / across as f32, row as f32 / down as f32);
                let position = warp.evaluate(u, v);
                let source = slice.source(u, v, aspect);
                SliceVertex {
                    position: [position.x, position.y],
                    source: [source.x, source.y, 1.0],
                    local: [u, v],
                }
            })
        })
        .collect();
    let at = |column: usize, row: usize| grid[row * (across + 1) + column];
    let mut vertices = Vec::with_capacity(across * down * 6);
    for row in 0..down {
        for column in 0..across {
            let corners = [
                at(column, row),
                at(column + 1, row),
                at(column + 1, row + 1),
                at(column, row + 1),
            ];
            vertices.extend([0, 1, 2, 0, 2, 3].map(|index| corners[index]));
        }
    }
    vertices
}

fn quad_vertices(slice: &Slice, aspect: f32) -> [SliceVertex; 6] {
    let corners = slice.output.0;
    let locals = prism_core::Area::FULL.corners();
    let weights = perspective_weights(&corners);
    let vertex = |index: usize| {
        let local = locals[index];
        let source = slice.source(local.x, local.y, aspect);
        let weight = weights[index];
        SliceVertex {
            position: [corners[index].x, corners[index].y],
            source: [source.x * weight, source.y * weight, weight],
            local: [local.x * weight, local.y * weight],
        }
    };
    [0, 1, 2, 0, 2, 3].map(vertex)
}

fn storage_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only: true },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn mask_texture_entry() -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding: 0,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: false },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

/// A screen to draw, and where.
pub struct ScreenTarget<'a> {
    /// Its slices.
    pub screen: &'a Screen,
    /// Texture of the screen's size, [`WORKING_FORMAT`].
    pub view: &'a wgpu::TextureView,
}

/// Vertices, shader index and mask of a slice to draw.
type SliceDraw = (SliceId, u32, Option<SliceId>);

/// Format of a mask's coverage: 0 hidden, 1 shown.
const MASK_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R8Unorm;

/// What a mask image was drawn from.
#[derive(Clone, PartialEq)]
struct MaskKey {
    mask: Mask,
    size: (u32, u32),
    flip: Flip,
}

/// The coverage of a slice's mask over its screen, drawn when the mask
/// changes.
struct MaskImage {
    key: MaskKey,
    bind_group: wgpu::BindGroup,
}

/// The triangles of a slice on the GPU, kept while it does not change.
struct SliceMesh {
    slice: Slice,
    aspect: f32,
    flip: Flip,
    buffer: wgpu::Buffer,
    count: u32,
}

impl SliceMesh {
    fn new(device: &wgpu::Device, slice: &Slice, aspect: f32, flip: Flip) -> Self {
        // A mirrored screen mirrors everything drawn on it.
        let bytes: Vec<u8> = slice_vertices(slice, aspect)
            .iter()
            .flat_map(|vertex| {
                let position = flip.apply(Point::new(vertex.position[0], vertex.position[1]));
                [position.x, position.y]
                    .into_iter()
                    .chain(vertex.source)
                    .chain(vertex.local)
                    .flat_map(f32::to_le_bytes)
            })
            .collect();
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("slice vertices"),
            size: (bytes.len() as u64).max(4),
            usage: wgpu::BufferUsages::VERTEX,
            mapped_at_creation: true,
        });
        if !bytes.is_empty()
            && let Ok(mut mapped) = buffer.slice(..).get_mapped_range_mut()
        {
            mapped.copy_from_slice(&bytes);
        }
        buffer.unmap();
        Self {
            slice: slice.clone(),
            aspect,
            flip,
            buffer,
            count: (bytes.len() / size_of::<SliceVertex>()) as u32,
        }
    }
}

/// Draws screens from the program image.
pub struct StageRenderer {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    mask_layout: wgpu::BindGroupLayout,
    mask_pipeline: wgpu::RenderPipeline,
    mask_points_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    /// What the shader knows of each slice, for a frame.
    slices: Option<wgpu::Buffer>,
    bind_group: Option<(wgpu::TextureView, wgpu::BindGroup)>,
    /// Bind group of a mask that shows everything.
    no_mask: wgpu::BindGroup,
    masks: HashMap<SliceId, MaskImage>,
    meshes: HashMap<SliceId, SliceMesh>,
}

impl StageRenderer {
    /// Creates the renderer.
    pub fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("stage.wgsl"));
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("stage"),
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
                storage_entry(2),
            ],
        });
        let mask_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("stage mask"),
            entries: &[mask_texture_entry()],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("stage"),
            bind_group_layouts: &[Some(&layout), Some(&mask_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("stage"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: size_of::<SliceVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float32x2,
                        1 => Float32x3,
                        2 => Float32x2
                    ],
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: WORKING_FORMAT,
                    // Masked slices are faded into what lies below.
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let mask_shader = device.create_shader_module(wgpu::include_wgsl!("mask.wgsl"));
        let mask_points_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("mask points"),
                entries: &[storage_entry(0)],
            });
        let mask_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mask"),
            bind_group_layouts: &[Some(&mask_points_layout)],
            immediate_size: 0,
        });
        let mask_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("mask"),
            layout: Some(&mask_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &mask_shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &mask_shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(MASK_FORMAT.into())],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("stage"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });
        // A texture too small for any pixel: read outside it, a texture
        // load gives 0, so the shader treats a mask of size 1 as none.
        let no_mask_view = device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("no mask"),
                size: wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: MASK_FORMAT,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&wgpu::TextureViewDescriptor::default());
        let no_mask = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("no mask"),
            layout: &mask_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&no_mask_view),
            }],
        });
        Self {
            pipeline,
            layout,
            mask_layout,
            mask_pipeline,
            mask_points_layout,
            sampler,
            slices: None,
            bind_group: None,
            no_mask,
            masks: HashMap::new(),
            meshes: HashMap::new(),
        }
    }

    /// Draws the coverage of `key`'s mask into a new image.
    fn draw_mask(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        key: MaskKey,
    ) -> MaskImage {
        let (width, height) = key.size;
        let mut bytes = Vec::with_capacity(16 + key.mask.points.len() * 8);
        bytes.extend((key.mask.points.len() as u32).to_le_bytes());
        bytes.extend(u32::from(key.mask.inverted).to_le_bytes());
        bytes.extend(key.mask.feather.max(0.0).to_le_bytes());
        bytes.extend(0u32.to_le_bytes());
        for point in &key.mask.points {
            let point = key.flip.apply(*point);
            bytes.extend((point.x * width as f32).to_le_bytes());
            bytes.extend((point.y * height as f32).to_le_bytes());
        }
        let points = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mask points"),
            size: bytes.len() as u64,
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: true,
        });
        if let Ok(mut mapped) = points.slice(..).get_mapped_range_mut() {
            mapped.copy_from_slice(&bytes);
        }
        points.unmap();
        let points_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mask points"),
            layout: &self.mask_points_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: points.as_entire_binding(),
            }],
        });
        let view = device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("mask"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: MASK_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&wgpu::TextureViewDescriptor::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("mask"),
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
            pass.set_pipeline(&self.mask_pipeline);
            pass.set_bind_group(0, &points_group, &[]);
            pass.draw(0..3, 0..1);
        }
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mask"),
            layout: &self.mask_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            }],
        });
        MaskImage { key, bind_group }
    }

    /// Records the drawing of `targets`, each from `program` (of
    /// `program_size` pixels): black, then its enabled slices, first to
    /// last, each inside its mask.
    pub fn encode(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        program: &wgpu::TextureView,
        program_size: (u32, u32),
        targets: &[ScreenTarget<'_>],
    ) {
        let aspect = program_size.0 as f32 / program_size.1.max(1) as f32;
        let mut infos: Vec<u8> = Vec::new();
        let mut seen = Vec::new();
        let mut draws: Vec<Vec<SliceDraw>> = Vec::with_capacity(targets.len());
        for target in targets {
            let flip = target.screen.flip;
            let mut screen_draws = Vec::new();
            for slice in target.screen.slices.iter().filter(|slice| slice.enabled) {
                seen.push(slice.id());
                // Warped slices make many vertices: kept on the GPU while
                // unchanged.
                if self.meshes.get(&slice.id()).is_none_or(|mesh| {
                    mesh.slice != *slice || mesh.aspect != aspect || mesh.flip != flip
                }) {
                    self.meshes
                        .insert(slice.id(), SliceMesh::new(device, slice, aspect, flip));
                }
                let index = (infos.len() / SLICE_INFO_BYTES) as u32;
                infos.extend(slice_info(slice, target.screen.test_pattern, index));
                let mask = slice
                    .mask
                    .as_ref()
                    .filter(|mask| mask.is_valid())
                    .map(|mask| {
                        let key = MaskKey {
                            mask: mask.clone(),
                            size: target.screen.size(),
                            flip,
                        };
                        if self
                            .masks
                            .get(&slice.id())
                            .is_none_or(|image| image.key != key)
                        {
                            let image = self.draw_mask(device, encoder, key);
                            self.masks.insert(slice.id(), image);
                        }
                        slice.id()
                    });
                screen_draws.push((slice.id(), index, mask));
            }
            draws.push(screen_draws);
        }
        // Slices gone or without mask: their caches go.
        self.meshes.retain(|id, _| seen.contains(id));
        let masked: Vec<SliceId> = draws
            .iter()
            .flatten()
            .filter_map(|(_, _, mask)| *mask)
            .collect();
        self.masks.retain(|id, _| masked.contains(id));

        let rebound = grow(
            device,
            &mut self.slices,
            infos.len(),
            wgpu::BufferUsages::STORAGE,
        );
        let Some(slice_buffer) = &self.slices else {
            return;
        };
        if !infos.is_empty() {
            queue.write_buffer(slice_buffer, 0, &infos);
        }
        // The program image changes only with the resolution.
        if rebound
            || self
                .bind_group
                .as_ref()
                .is_none_or(|(view, _)| view != program)
        {
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("stage"),
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
                        resource: slice_buffer.as_entire_binding(),
                    },
                ],
            });
            self.bind_group = Some((program.clone(), bind_group));
        }
        let Some((_, bind_group)) = &self.bind_group else {
            return;
        };
        for (target, screen_draws) in targets.iter().zip(draws) {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("stage"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target.view,
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
            if screen_draws.is_empty() {
                continue;
            }
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, bind_group, &[]);
            // The instance index names the slice for the shader.
            for (slice, index, mask) in screen_draws {
                let Some(mesh) = self.meshes.get(&slice) else {
                    continue;
                };
                pass.set_vertex_buffer(0, mesh.buffer.slice(..));
                let mask = mask
                    .and_then(|id| self.masks.get(&id))
                    .map_or(&self.no_mask, |image| &image.bind_group);
                pass.set_bind_group(1, mask, &[]);
                pass.draw(0..mesh.count, index..index + 1);
            }
        }
    }
}

/// Bytes of what the shader knows of a slice: its edge fades, then the
/// fade curve, test pattern and pattern color.
const SLICE_INFO_BYTES: usize = 32;

fn slice_info(slice: &Slice, pattern: bool, index: u32) -> Vec<u8> {
    let blend = slice.blend;
    let mut bytes = Vec::with_capacity(SLICE_INFO_BYTES);
    for value in [
        blend.left.max(0.0),
        blend.right.max(0.0),
        blend.top.max(0.0),
        blend.bottom.max(0.0),
        blend.power.max(0.1),
        blend.gamma.max(0.1),
        if pattern { 1.0 } else { 0.0 },
        index as f32,
    ] {
        bytes.extend(value.to_le_bytes());
    }
    bytes
}

/// Makes `slot` a buffer of at least `size` bytes; returns whether it was
/// replaced.
fn grow(
    device: &wgpu::Device,
    slot: &mut Option<wgpu::Buffer>,
    size: usize,
    usage: wgpu::BufferUsages,
) -> bool {
    let size = (size as u64).max(32);
    if slot.as_ref().is_some_and(|buffer| buffer.size() >= size) {
        return false;
    }
    *slot = Some(device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("stage"),
        size: size.next_power_of_two(),
        usage: usage | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    }));
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rectangles_have_no_perspective() {
        let square = prism_core::Quad::FULL.0;
        assert_eq!(perspective_weights(&square), [2.0; 4]);
        let flat = [Point::new(0.0, 0.0); 4];
        assert_eq!(perspective_weights(&flat), [1.0; 4]);
    }

    #[test]
    fn the_wide_side_of_a_trapezoid_weighs_more() {
        // Top edge half as wide as the bottom one: the diagonals cross a
        // third of the way down; the wide side is the near one.
        let trapezoid = [
            Point::new(0.25, 0.0),
            Point::new(0.75, 0.0),
            Point::new(1.0, 1.0),
            Point::new(0.0, 1.0),
        ];
        let [top_left, top_right, bottom_right, bottom_left] = perspective_weights(&trapezoid);
        assert!((top_left - 1.5).abs() < 1e-5, "{top_left}");
        assert!((top_right - 1.5).abs() < 1e-5);
        assert!((bottom_right - 3.0).abs() < 1e-5);
        assert!((bottom_left - 3.0).abs() < 1e-5);
    }
}
