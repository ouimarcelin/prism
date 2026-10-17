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

/// The two triangles drawing `slice`.
pub fn slice_vertices(slice: &Slice) -> [SliceVertex; 6] {
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
            bind_group: None,
        }
    }

    /// Records the drawing of `targets`, each from `program`: black, then
    /// its enabled slices, first to last.
    pub fn encode(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        program: &wgpu::TextureView,
        targets: &[ScreenTarget<'_>],
    ) {
        let mut vertices = Vec::new();
        let mut ranges = Vec::with_capacity(targets.len());
        for target in targets {
            let start = vertices.len() as u32;
            for slice in target.screen.slices.iter().filter(|slice| slice.enabled) {
                vertices.extend(slice_vertices(slice));
            }
            ranges.push(start..vertices.len() as u32);
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
        let size = (bytes.len() as u64).max(4);
        if self
            .vertices
            .as_ref()
            .is_none_or(|buffer| buffer.size() < size)
        {
            self.vertices = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("stage vertices"),
                size: size.next_power_of_two(),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }
        let Some(buffer) = &self.vertices else {
            return;
        };
        if !bytes.is_empty() {
            queue.write_buffer(buffer, 0, &bytes);
        }
        // The program image changes only with the resolution.
        if self
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
                ],
            });
            self.bind_group = Some((program.clone(), bind_group));
        }
        let Some((_, bind_group)) = &self.bind_group else {
            return;
        };
        for (target, range) in targets.iter().zip(ranges) {
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
            if range.is_empty() {
                continue;
            }
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, bind_group, &[]);
            pass.set_vertex_buffer(0, buffer.slice(..));
            pass.draw(range, 0..1);
        }
    }
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
