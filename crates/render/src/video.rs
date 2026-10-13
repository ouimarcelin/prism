//! Upload of decoded video frames into layer textures.
//!
//! HAP frames stay block-compressed on the way to the GPU: the planes are
//! copied as BC textures, then one pass converts them into a
//! [`WORKING_FORMAT`] texture the compositor samples (premultiplied alpha,
//! YCoCg decoding, block padding removed).

use std::num::NonZeroU64;

use prism_media::{BlockFormat, ColorLayout, HapFrame, Plane};

use crate::WORKING_FORMAT;

/// Error raised while uploading a frame.
#[derive(Debug, thiserror::Error)]
pub enum UploadError {
    /// The GPU cannot sample BC textures.
    #[error("the GPU does not support BC texture compression")]
    BlockCompressionUnsupported,
    /// A plane does not match the texture size.
    #[error("frame plane has {actual} bytes, expected {expected}")]
    WrongSize {
        /// Bytes expected for the texture size.
        expected: usize,
        /// Bytes received.
        actual: usize,
    },
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct ConvertParams {
    layout: u32,
    has_alpha_plane: u32,
    _pad: [u32; 2],
}

const PARAMS_SIZE: u64 = std::mem::size_of::<ConvertParams>() as u64;

fn wgpu_format(format: BlockFormat) -> wgpu::TextureFormat {
    match format {
        BlockFormat::Bc1 => wgpu::TextureFormat::Bc1RgbaUnorm,
        BlockFormat::Bc3 => wgpu::TextureFormat::Bc3RgbaUnorm,
        BlockFormat::Bc4 => wgpu::TextureFormat::Bc4RUnorm,
        BlockFormat::Bc7 => wgpu::TextureFormat::Bc7RgbaUnorm,
    }
}

fn layout_code(layout: ColorLayout) -> u32 {
    // Must match the constants of convert.wgsl.
    match layout {
        ColorLayout::Rgb => 0,
        ColorLayout::Rgba => 1,
        ColorLayout::ScaledYCoCg => 2,
    }
}

/// Pipeline shared by every [`VideoTexture`].
pub struct FrameUploader {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    no_alpha: wgpu::TextureView,
}

/// A block-compressed plane on the GPU.
struct BlockTexture {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    format: BlockFormat,
}

/// The current frame of one clip, as a layer texture.
pub struct VideoTexture {
    width: u32,
    height: u32,
    color: Option<BlockTexture>,
    alpha: Option<BlockTexture>,
    params: wgpu::Buffer,
    output: wgpu::Texture,
    output_view: wgpu::TextureView,
}

impl VideoTexture {
    /// Creates a texture for frames of `width` × `height` pixels.
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let output = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("video frame"),
            size: wgpu::Extent3d {
                width,
                height,
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
        let output_view = output.create_view(&wgpu::TextureViewDescriptor::default());
        let params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("video convert params"),
            size: PARAMS_SIZE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            width,
            height,
            color: None,
            alpha: None,
            params,
            output,
            output_view,
        }
    }

    /// Frame width in pixels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Frame height in pixels.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Layer texture holding the last uploaded frame, for [`crate::LayerInput`].
    pub fn view(&self) -> &wgpu::TextureView {
        &self.output_view
    }

    /// Underlying layer texture.
    pub fn texture(&self) -> &wgpu::Texture {
        &self.output
    }
}

impl FrameUploader {
    /// Creates the conversion pipeline.
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("convert.wgsl"));
        let texture_entry = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: false },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("video convert"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: NonZeroU64::new(PARAMS_SIZE),
                    },
                    count: None,
                },
                texture_entry(1),
                texture_entry(2),
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("video convert"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("video convert"),
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

        // Bound in place of the alpha plane when the frame has none.
        let no_alpha = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("no alpha plane"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            no_alpha.as_image_copy(),
            &[255],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(1),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        let no_alpha = no_alpha.create_view(&wgpu::TextureViewDescriptor::default());

        Self {
            pipeline,
            bind_group_layout,
            no_alpha,
        }
    }

    /// Copies the planes of `frame` to the GPU and records, in `encoder`, the
    /// pass that converts them into `target`.
    ///
    /// The plane copies happen when the next command buffer is submitted, so
    /// upload a given `target` at most once per submission.
    pub fn upload(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &mut VideoTexture,
        frame: &HapFrame,
    ) -> Result<(), UploadError> {
        if !device
            .features()
            .contains(wgpu::Features::TEXTURE_COMPRESSION_BC)
        {
            return Err(UploadError::BlockCompressionUnsupported);
        }
        let (width, height) = (target.width, target.height);
        write_plane(
            device,
            queue,
            &mut target.color,
            &frame.color,
            width,
            height,
        )?;
        match &frame.alpha {
            Some(plane) => write_plane(device, queue, &mut target.alpha, plane, width, height)?,
            None => target.alpha = None,
        }

        let params = ConvertParams {
            layout: layout_code(frame.layout),
            has_alpha_plane: u32::from(frame.alpha.is_some()),
            _pad: [0; 2],
        };
        queue.write_buffer(&target.params, 0, bytemuck::bytes_of(&params));

        let Some(color) = &target.color else {
            return Ok(());
        };
        let alpha_view = target
            .alpha
            .as_ref()
            .map_or(&self.no_alpha, |alpha| &alpha.view);
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("video convert"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: target.params.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&color.view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(alpha_view),
                },
            ],
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("video convert"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &target.output_view,
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
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..3, 0..1);
        Ok(())
    }
}

/// Copies a plane into its block texture, creating the texture when the
/// format changes.
fn write_plane(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    slot: &mut Option<BlockTexture>,
    plane: &Plane,
    width: u32,
    height: u32,
) -> Result<(), UploadError> {
    let expected = plane.format.image_bytes(width, height);
    if plane.data.len() != expected {
        return Err(UploadError::WrongSize {
            expected,
            actual: plane.data.len(),
        });
    }
    // Block-compressed textures must span whole blocks.
    let size = wgpu::Extent3d {
        width: width.div_ceil(4) * 4,
        height: height.div_ceil(4) * 4,
        depth_or_array_layers: 1,
    };
    if slot
        .as_ref()
        .is_none_or(|block| block.format != plane.format)
    {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("video plane"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu_format(plane.format),
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        *slot = Some(BlockTexture {
            texture,
            view,
            format: plane.format,
        });
    }
    let Some(block) = slot else {
        return Ok(());
    };
    let row_bytes = width.div_ceil(4) * plane.format.block_bytes() as u32;
    queue.write_texture(
        block.texture.as_image_copy(),
        &plane.data,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(row_bytes),
            rows_per_image: Some(height.div_ceil(4)),
        },
        size,
    );
    Ok(())
}
