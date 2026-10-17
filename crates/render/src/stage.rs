//! Drawing of the screens of the stage: each slice of the composition
//! placed on its screen through its four corners, in perspective.

use prism_core::{Point, Screen, Slice};

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
pub fn slice_vertices(slice: &Slice) -> Vec<SliceVertex> {
    match &slice.warp {
        Some(warp) if warp.is_valid() => warp_vertices(slice, warp),
        _ => quad_vertices(slice).to_vec(),
    }
}

fn warp_vertices(slice: &Slice, warp: &prism_core::Warp) -> Vec<SliceVertex> {
    let (columns, rows) = warp.cells();
    let steps = |cells: u8| (usize::from(cells) * STEPS_PER_CELL).clamp(STEPS.0, STEPS.1);
    let (across, down) = (steps(columns), steps(rows));
    let input = slice.input;
    let grid: Vec<SliceVertex> = (0..=down)
        .flat_map(|row| {
            (0..=across).map(move |column| {
                let (u, v) = (column as f32 / across as f32, row as f32 / down as f32);
                let position = warp.evaluate(u, v);
                SliceVertex {
                    position: [position.x, position.y],
                    source: [input.x + u * input.width, input.y + v * input.height, 1.0],
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

fn quad_vertices(slice: &Slice) -> [SliceVertex; 6] {
    let corners = slice.output.0;
    let sources = slice.input.corners();
    let weights = perspective_weights(&corners);
    let vertex = |index: usize| SliceVertex {
        position: [corners[index].x, corners[index].y],
        source: [
            sources[index].x * weights[index],
            sources[index].y * weights[index],
            weights[index],
        ],
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

/// A screen to draw, and where.
pub struct ScreenTarget<'a> {
    /// Its slices.
    pub screen: &'a Screen,
    /// Texture of the screen's size, [`WORKING_FORMAT`].
    pub view: &'a wgpu::TextureView,
}

/// Draws screens from the program image.
pub struct StageRenderer {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    /// Vertices of every screen of a frame, written once per frame.
    vertices: Option<wgpu::Buffer>,
    /// Mask headers and mask points of a frame.
    masks: Option<wgpu::Buffer>,
    points: Option<wgpu::Buffer>,
    bind_group: Option<(wgpu::TextureView, wgpu::BindGroup)>,
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
                storage_entry(3),
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("stage"),
            bind_group_layouts: &[Some(&layout)],
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
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x3],
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
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("stage"),
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
            vertices: None,
            masks: None,
            points: None,
            bind_group: None,
        }
    }

    /// Records the drawing of `targets`, each from `program`: black, then
    /// its enabled slices, first to last, each inside its mask.
    pub fn encode(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        program: &wgpu::TextureView,
        targets: &[ScreenTarget<'_>],
    ) {
        let mut vertices = Vec::new();
        // Mask 0: none.
        let mut masks: Vec<u8> = mask_header(0, 0, false, 0.0).to_vec();
        let mut points: Vec<u8> = vec![0; 8];
        let mut draws: Vec<Vec<(std::ops::Range<u32>, u32)>> = Vec::with_capacity(targets.len());
        for target in targets {
            let (width, height) = target.screen.size();
            let mut screen_draws = Vec::new();
            for slice in target.screen.slices.iter().filter(|slice| slice.enabled) {
                let start = vertices.len() as u32;
                vertices.extend(slice_vertices(slice));
                let mask = match &slice.mask {
                    Some(mask) if mask.is_valid() => {
                        let index = (masks.len() / MASK_HEADER_BYTES) as u32;
                        let first = (points.len() / 8) as u32;
                        for point in &mask.points {
                            points.extend((point.x * width as f32).to_le_bytes());
                            points.extend((point.y * height as f32).to_le_bytes());
                        }
                        masks.extend(mask_header(
                            first,
                            mask.points.len() as u32,
                            mask.inverted,
                            mask.feather,
                        ));
                        index
                    }
                    _ => 0,
                };
                screen_draws.push((start..vertices.len() as u32, mask));
            }
            draws.push(screen_draws);
        }
        let bytes: Vec<u8> = vertices
            .iter()
            .flat_map(|vertex| {
                vertex
                    .position
                    .iter()
                    .chain(&vertex.source)
                    .flat_map(|value| value.to_le_bytes())
            })
            .collect();
        let mut rebound = grow(
            device,
            &mut self.vertices,
            bytes.len(),
            wgpu::BufferUsages::VERTEX,
        );
        rebound |= grow(
            device,
            &mut self.masks,
            masks.len(),
            wgpu::BufferUsages::STORAGE,
        );
        rebound |= grow(
            device,
            &mut self.points,
            points.len(),
            wgpu::BufferUsages::STORAGE,
        );
        let (Some(buffer), Some(mask_buffer), Some(point_buffer)) =
            (&self.vertices, &self.masks, &self.points)
        else {
            return;
        };
        if !bytes.is_empty() {
            queue.write_buffer(buffer, 0, &bytes);
        }
        queue.write_buffer(mask_buffer, 0, &masks);
        queue.write_buffer(point_buffer, 0, &points);
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
                        resource: mask_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: point_buffer.as_entire_binding(),
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
            pass.set_vertex_buffer(0, buffer.slice(..));
            // The instance index carries the mask of the slice.
            for (range, mask) in screen_draws {
                pass.draw(range, mask..mask + 1);
            }
        }
    }
}

/// Bytes of a mask header in the shader: first point, point count,
/// inverted, soft edge width in pixels.
const MASK_HEADER_BYTES: usize = 16;

fn mask_header(first: u32, count: u32, inverted: bool, feather: f32) -> [u8; MASK_HEADER_BYTES] {
    let mut bytes = [0; MASK_HEADER_BYTES];
    bytes[0..4].copy_from_slice(&first.to_le_bytes());
    bytes[4..8].copy_from_slice(&count.to_le_bytes());
    bytes[8..12].copy_from_slice(&u32::from(inverted).to_le_bytes());
    bytes[12..16].copy_from_slice(&feather.max(0.0).to_le_bytes());
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
    let size = (size as u64).max(16);
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
