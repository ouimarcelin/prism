//! 3D objects of a screen: meshes seen from the screen's projector, the
//! program laid on them by their texture coordinates, with a depth buffer
//! so nearer faces hide farther ones.

use std::collections::HashMap;
use std::path::Path;

use bytemuck::{Pod, Zeroable};
use prism_core::{MeshSource, Object3d, Projector};

use crate::gpu::WORKING_FORMAT;

const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// A point of a mesh: position, then texture coordinates (v down, as in
/// textures).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct MeshVertex {
    /// Position, in metres.
    pub position: [f32; 3],
    /// Texture coordinates.
    pub uv: [f32; 2],
}

/// Triangles of a shape: vertices and indices.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MeshData {
    /// The vertices.
    pub vertices: Vec<MeshVertex>,
    /// Three indices per triangle.
    pub indices: Vec<u32>,
}

impl MeshData {
    /// The triangles of `source`; for a file, an error to show when it
    /// cannot be read.
    pub fn of(source: &MeshSource) -> Result<Self, String> {
        match source {
            MeshSource::Cube => Ok(cube()),
            MeshSource::Plane => Ok(plane()),
            MeshSource::File { path } => load_obj(path),
        }
    }
}

/// A square of one metre facing +z, the whole texture on it.
fn plane() -> MeshData {
    let vertex = |x: f32, y: f32, u: f32, v: f32| MeshVertex {
        position: [x, y, 0.0],
        uv: [u, v],
    };
    MeshData {
        vertices: vec![
            vertex(-0.5, -0.5, 0.0, 1.0),
            vertex(0.5, -0.5, 1.0, 1.0),
            vertex(0.5, 0.5, 1.0, 0.0),
            vertex(-0.5, 0.5, 0.0, 0.0),
        ],
        indices: vec![0, 1, 2, 0, 2, 3],
    }
}

/// A cube of one metre, each face showing the whole texture, upright.
fn cube() -> MeshData {
    // Each face: its normal axis and sign, and the two axes along it.
    let faces: [([f32; 3], [f32; 3], [f32; 3]); 6] = [
        ([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        ([0.0, 0.0, -1.0], [-1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        ([1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]),
        ([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0]),
        ([0.0, 1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, -1.0]),
        ([0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
    ];
    let mut data = MeshData::default();
    for (normal, right, up) in faces {
        let base = data.vertices.len() as u32;
        for (a, b, u, v) in [
            (-0.5, -0.5, 0.0, 1.0),
            (0.5, -0.5, 1.0, 1.0),
            (0.5, 0.5, 1.0, 0.0),
            (-0.5, 0.5, 0.0, 0.0),
        ] {
            let position =
                std::array::from_fn(|axis| normal[axis] * 0.5 + right[axis] * a + up[axis] * b);
            data.vertices.push(MeshVertex {
                position,
                uv: [u, v],
            });
        }
        data.indices
            .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    data
}

/// The triangles of an OBJ file, all its meshes together. Without texture
/// coordinates, the front view (x, y) places the texture.
fn load_obj(path: &Path) -> Result<MeshData, String> {
    let options = tobj::LoadOptions {
        single_index: true,
        triangulate: true,
        ..Default::default()
    };
    let (models, _) =
        tobj::load_obj(path, &options).map_err(|error| format!("{} : {error}", path.display()))?;
    let mut data = MeshData::default();
    for model in models {
        let mesh = model.mesh;
        let base = data.vertices.len() as u32;
        let count = mesh.positions.len() / 3;
        for index in 0..count {
            let position = [
                mesh.positions[3 * index],
                mesh.positions[3 * index + 1],
                mesh.positions[3 * index + 2],
            ];
            let uv = if mesh.texcoords.len() >= 2 * count {
                [
                    mesh.texcoords[2 * index],
                    1.0 - mesh.texcoords[2 * index + 1],
                ]
            } else {
                [position[0] + 0.5, 0.5 - position[1]]
            };
            data.vertices.push(MeshVertex { position, uv });
        }
        data.indices
            .extend(mesh.indices.iter().map(|index| base + index));
    }
    if data.indices.is_empty() {
        return Err(format!("{} : aucun triangle", path.display()));
    }
    Ok(data)
}

/// A 4 × 4 matrix, column after column.
pub type Matrix = [[f32; 4]; 4];

fn multiply(a: &Matrix, b: &Matrix) -> Matrix {
    let mut out = [[0.0; 4]; 4];
    for (column, out_column) in out.iter_mut().enumerate() {
        for (row, value) in out_column.iter_mut().enumerate() {
            *value = (0..4).map(|k| a[k][row] * b[column][k]).sum();
        }
    }
    out
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn normalize(a: [f32; 3]) -> [f32; 3] {
    let length = dot(a, a).sqrt().max(1e-9);
    [a[0] / length, a[1] / length, a[2] / length]
}

/// What the projector sees, for an image of `aspect` (width over height):
/// view then projection, depth from 0 (near) to 1 (far).
pub fn projector_matrix(projector: &Projector, aspect: f32) -> Matrix {
    let eye = projector.position;
    let forward = normalize(sub(projector.target, eye));
    // Looking straight up or down: another up.
    let up = if forward[1].abs() > 0.999 {
        [0.0, 0.0, -1.0]
    } else {
        [0.0, 1.0, 0.0]
    };
    let side = normalize(cross(forward, up));
    let true_up = cross(side, forward);
    let view = [
        [side[0], true_up[0], -forward[0], 0.0],
        [side[1], true_up[1], -forward[1], 0.0],
        [side[2], true_up[2], -forward[2], 0.0],
        [-dot(side, eye), -dot(true_up, eye), dot(forward, eye), 1.0],
    ];
    let (near, far) = (0.05, 1000.0);
    let focal = 1.0 / (projector.fov.clamp(1.0, 170.0).to_radians() / 2.0).tan();
    let depth = far / (near - far);
    let mut projection = [
        [focal / aspect.max(1e-6), 0.0, 0.0, 0.0],
        [0.0, focal, 0.0, 0.0],
        [0.0, 0.0, depth, -1.0],
        [0.0, 0.0, near * depth, 0.0],
    ];
    // Lens shift: the image moved by fractions of its size.
    projection[2][0] = -2.0 * projector.shift[0];
    projection[2][1] = -2.0 * projector.shift[1];
    multiply(&projection, &view)
}

/// Where the object stands: scale, then turns around x, y, z, then moves.
pub fn object_matrix(object: &Object3d) -> Matrix {
    let [x, y, z] = object.rotation.map(f32::to_radians);
    let rotate_x = [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, x.cos(), x.sin(), 0.0],
        [0.0, -x.sin(), x.cos(), 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    let rotate_y = [
        [y.cos(), 0.0, -y.sin(), 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [y.sin(), 0.0, y.cos(), 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    let rotate_z = [
        [z.cos(), z.sin(), 0.0, 0.0],
        [-z.sin(), z.cos(), 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    let scale = object.scale;
    let scaled = [
        [scale, 0.0, 0.0, 0.0],
        [0.0, scale, 0.0, 0.0],
        [0.0, 0.0, scale, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    let [px, py, pz] = object.position;
    let moved = [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [px, py, pz, 1.0],
    ];
    let turned = multiply(&rotate_z, &multiply(&rotate_y, &rotate_x));
    multiply(&moved, &multiply(&turned, &scaled))
}

/// Where a point of the object falls on the screen, from -1 to 1 (y up),
/// if in front of the projector.
pub fn project(matrix: &Matrix, point: [f32; 3]) -> Option<[f32; 2]> {
    let clip: [f32; 4] = std::array::from_fn(|row| {
        matrix[0][row] * point[0]
            + matrix[1][row] * point[1]
            + matrix[2][row] * point[2]
            + matrix[3][row]
    });
    (clip[3] > 1e-6).then(|| [clip[0] / clip[3], clip[1] / clip[3]])
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ObjectUniform {
    transform: Matrix,
    area: [f32; 4],
    opacity: f32,
    _padding: [f32; 3],
}

/// A shape on the GPU.
struct GpuMesh {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    count: u32,
}

/// Draws the 3D objects of screens.
pub(crate) struct ObjectRenderer {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    meshes: HashMap<MeshSource, Result<GpuMesh, String>>,
    depth: HashMap<(u32, u32), wgpu::TextureView>,
}

impl ObjectRenderer {
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("objects.wgsl"));
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("objects"),
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
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("objects"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("objects"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<MeshVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2],
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: WORKING_FORMAT,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("objects"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        Self {
            pipeline,
            layout,
            sampler,
            meshes: HashMap::new(),
            depth: HashMap::new(),
        }
    }

    /// Records the objects of a screen over what `target` holds.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn encode(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        program: &wgpu::TextureView,
        size: (u32, u32),
        projector: &Projector,
        objects: &[Object3d],
        target: &wgpu::TextureView,
    ) {
        let shown: Vec<&Object3d> = objects.iter().filter(|object| object.enabled).collect();
        if shown.is_empty() {
            return;
        }
        for object in &shown {
            if !self.meshes.contains_key(&object.mesh) {
                let mesh = MeshData::of(&object.mesh).map(|data| upload(device, queue, &data));
                self.meshes.insert(object.mesh.clone(), mesh);
            }
        }
        let depth = self
            .depth
            .entry(size)
            .or_insert_with(|| {
                device
                    .create_texture(&wgpu::TextureDescriptor {
                        label: Some("objects depth"),
                        size: wgpu::Extent3d {
                            width: size.0.max(1),
                            height: size.1.max(1),
                            depth_or_array_layers: 1,
                        },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: DEPTH_FORMAT,
                        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                        view_formats: &[],
                    })
                    .create_view(&wgpu::TextureViewDescriptor::default())
            })
            .clone();
        let camera = projector_matrix(projector, size.0 as f32 / size.1.max(1) as f32);
        let mut draws = Vec::new();
        for object in shown {
            let Some(Ok(mesh)) = self.meshes.get(&object.mesh) else {
                continue;
            };
            let uniform = ObjectUniform {
                transform: multiply(&camera, &object_matrix(object)),
                area: object.area,
                opacity: object.opacity.clamp(0.0, 1.0),
                _padding: [0.0; 3],
            };
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("object"),
                size: std::mem::size_of::<ObjectUniform>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            queue.write_buffer(&buffer, 0, bytemuck::bytes_of(&uniform));
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("object"),
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
            draws.push((mesh, bind_group));
        }
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("objects"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipeline);
        for (mesh, bind_group) in &draws {
            pass.set_bind_group(0, bind_group, &[]);
            pass.set_vertex_buffer(0, mesh.vertices.slice(..));
            pass.set_index_buffer(mesh.indices.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..mesh.count, 0, 0..1);
        }
    }

    /// Why the shape of `mesh` could not be read, if it could not.
    pub(crate) fn error(&self, mesh: &MeshSource) -> Option<&str> {
        self.meshes
            .get(mesh)
            .and_then(|mesh| mesh.as_ref().err())
            .map(String::as_str)
    }
}

fn upload(device: &wgpu::Device, queue: &wgpu::Queue, data: &MeshData) -> GpuMesh {
    let buffer = |label, bytes: &[u8], usage| {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: (bytes.len() as u64).max(4).next_multiple_of(4),
            usage: usage | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&buffer, 0, bytes);
        buffer
    };
    GpuMesh {
        vertices: buffer(
            "mesh vertices",
            bytemuck::cast_slice(&data.vertices),
            wgpu::BufferUsages::VERTEX,
        ),
        indices: buffer(
            "mesh indices",
            bytemuck::cast_slice(&data.indices),
            wgpu::BufferUsages::INDEX,
        ),
        count: data.indices.len() as u32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_projector_sees_what_it_aims_at() {
        let projector = Projector::default();
        let matrix = projector_matrix(&projector, 16.0 / 9.0);
        let centre = project(&matrix, [0.0, 0.0, 0.0]).unwrap();
        assert!(
            centre[0].abs() < 1e-5 && centre[1].abs() < 1e-5,
            "{centre:?}"
        );
        // Up is up, right is right.
        let up = project(&matrix, [0.0, 0.5, 0.0]).unwrap();
        let right = project(&matrix, [0.5, 0.0, 0.0]).unwrap();
        assert!(up[1] > 0.0 && right[0] > 0.0);
        // Behind the projector: not on the image.
        assert!(project(&matrix, [0.0, 0.0, 10.0]).is_none());
        // A shifted lens moves the image up.
        let shifted = Projector {
            shift: [0.0, 0.5],
            ..projector
        };
        let moved = project(&projector_matrix(&shifted, 16.0 / 9.0), [0.0, 0.0, 0.0]).unwrap();
        assert!((moved[1] - 1.0).abs() < 1e-4, "{moved:?}");
    }

    #[test]
    fn objects_turn_and_move() {
        let mut object = Object3d::new("cube", MeshSource::Cube);
        object.position = [1.0, 0.0, 0.0];
        object.rotation = [0.0, 90.0, 0.0];
        object.scale = 2.0;
        let matrix = object_matrix(&object);
        // A point on +x turns to -z (around y), doubled, moved by 1 on x.
        let point = project(&matrix, [0.5, 0.0, 0.0]);
        assert!(point.is_some());
        let world: [f32; 3] = std::array::from_fn(|row| matrix[0][row] * 0.5 + matrix[3][row]);
        assert!(
            (world[0] - 1.0).abs() < 1e-5 && (world[2] + 1.0).abs() < 1e-5,
            "{world:?}"
        );
        // Built-in shapes have triangles and texture coordinates in range.
        for shape in [MeshSource::Cube, MeshSource::Plane] {
            let data = MeshData::of(&shape).unwrap();
            assert_eq!(data.indices.len() % 3, 0);
            assert!(
                data.vertices
                    .iter()
                    .all(|vertex| vertex.uv.iter().all(|uv| (0.0..=1.0).contains(uv)))
            );
        }
    }
}
