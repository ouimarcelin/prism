//! Upload of decoded video frames into layer textures.
//!
//! HAP frames stay block-compressed on the way to the GPU: the planes are
//! copied as BC textures, then one pass converts them into a
//! [`WORKING_FORMAT`] texture the compositor samples (premultiplied alpha,
//! YCoCg decoding, block padding removed).
//!
//! Frame data goes through a [`StagingBelt`]: upload buffers kept mapped and
//! reused from frame to frame, so playback allocates no GPU memory once
//! started. A fresh allocation per frame can stall the render thread when
//! the machine runs short of memory.

use std::num::NonZeroU64;

use wgpu::util::StagingBelt;

use prism_media::{
    BlockFormat, ChannelOrder, ColorLayout, Frame, HapFrame, Plane, RgbaImage, YuvImage, YuvMatrix,
};

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
    /// An upload buffer cannot be written.
    #[error("upload buffer unavailable: {0}")]
    Staging(#[from] wgpu::MapRangeError),
}

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, bytemuck::Pod, bytemuck::Zeroable)]
struct ConvertParams {
    layout: u32,
    has_alpha_plane: u32,
    yuv_flags: u32,
    _pad: u32,
}

/// Layout code of 4:2:0 video in convert.wgsl.
const LAYOUT_YUV_420: u32 = 3;

const PARAMS_SIZE: u64 = std::mem::size_of::<ConvertParams>() as u64;

/// Size of the upload buffers: the frames of four 1080p HAP Q layers fit in
/// one. A larger frame gets a buffer of its own size, kept for reuse too.
const STAGING_CHUNK: u64 = 8 << 20;

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

/// Pipeline and upload buffers shared by every [`VideoTexture`].
pub struct FrameUploader {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    no_alpha: wgpu::TextureView,
    chroma_sampler: wgpu::Sampler,
    staging: StagingBelt,
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
    rgba: Option<(wgpu::Texture, wgpu::TextureView)>,
    rgba_format: wgpu::TextureFormat,
    luma: Option<(wgpu::Texture, wgpu::TextureView)>,
    chroma: Option<(wgpu::Texture, wgpu::TextureView)>,
    params: wgpu::Buffer,
    params_written: Option<ConvertParams>,
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
            rgba: None,
            rgba_format: wgpu::TextureFormat::Rgba8Unorm,
            luma: None,
            chroma: None,
            params,
            params_written: None,
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
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
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

        let chroma_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("chroma"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        Self {
            pipeline,
            bind_group_layout,
            no_alpha,
            chroma_sampler,
            staging: StagingBelt::new(device.clone(), STAGING_CHUNK),
        }
    }

    /// Records, in `encoder`, the copy of `frame` to the GPU and the pass
    /// that converts it into `target`.
    ///
    /// Call [`FrameUploader::finish`] after the last upload into `encoder`,
    /// before submitting it. The conversion settings of `target` change
    /// when the submission starts, so upload a given `target` at most once
    /// per submission.
    pub fn upload(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &mut VideoTexture,
        frame: &Frame,
    ) -> Result<(), UploadError> {
        match frame {
            Frame::Hap(hap) => self.upload_hap(device, queue, encoder, target, hap),
            Frame::Rgba(image) => self.upload_rgba(device, queue, encoder, target, image),
            Frame::Yuv(image) => self.upload_yuv(device, queue, encoder, target, image),
        }
    }

    /// Closes the upload buffers written into `encoder`. They return to the
    /// uploader once the GPU has run `encoder`, which must be submitted.
    pub fn finish(&mut self, encoder: &wgpu::CommandEncoder) {
        self.staging.finish_and_recall_on_submit(encoder);
    }

    fn upload_hap(
        &mut self,
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
            &mut self.staging,
            encoder,
            &mut target.color,
            &frame.color,
            width,
            height,
        )?;
        match &frame.alpha {
            Some(plane) => write_plane(
                device,
                &mut self.staging,
                encoder,
                &mut target.alpha,
                plane,
                width,
                height,
            )?,
            None => target.alpha = None,
        }
        let params = ConvertParams {
            layout: layout_code(frame.layout),
            has_alpha_plane: u32::from(frame.alpha.is_some()),
            yuv_flags: 0,
            _pad: 0,
        };
        target.write_params(queue, params);
        let Some(color) = &target.color else {
            return Ok(());
        };
        let alpha = target
            .alpha
            .as_ref()
            .map_or(&self.no_alpha, |alpha| &alpha.view);
        self.convert(device, encoder, target, &color.view, alpha, &self.no_alpha);
        Ok(())
    }

    fn upload_rgba(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &mut VideoTexture,
        image: &RgbaImage,
    ) -> Result<(), UploadError> {
        let expected = target.width as usize * target.height as usize * 4;
        if (image.width, image.height) != (target.width, target.height)
            || image.pixels.len() != expected
        {
            return Err(UploadError::WrongSize {
                expected,
                actual: image.pixels.len(),
            });
        }
        let size = wgpu::Extent3d {
            width: target.width,
            height: target.height,
            depth_or_array_layers: 1,
        };
        // The GPU reorders blue-first pixels itself.
        let format = match image.order {
            ChannelOrder::Rgba => wgpu::TextureFormat::Rgba8Unorm,
            ChannelOrder::Bgra => wgpu::TextureFormat::Bgra8Unorm,
        };
        if target.rgba_format != format {
            target.rgba = None;
            target.rgba_format = format;
        }
        let (texture, view) = target.rgba.get_or_insert_with(|| {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("video pixels"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            (texture, view)
        });
        stage_copy(
            &mut self.staging,
            encoder,
            texture,
            &image.pixels,
            target.width * 4,
            target.height,
            size,
        )?;
        let view = view.clone();
        target.write_params(
            queue,
            ConvertParams {
                layout: layout_code(ColorLayout::Rgba),
                has_alpha_plane: 0,
                yuv_flags: 0,
                _pad: 0,
            },
        );
        self.convert(
            device,
            encoder,
            target,
            &view,
            &self.no_alpha,
            &self.no_alpha,
        );
        Ok(())
    }

    fn upload_yuv(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &mut VideoTexture,
        image: &YuvImage,
    ) -> Result<(), UploadError> {
        let (chroma_width, chroma_height) = image.chroma_size();
        let luma_bytes = target.width as usize * target.height as usize;
        let chroma_bytes = chroma_width as usize * chroma_height as usize * 2;
        if (image.width, image.height) != (target.width, target.height)
            || image.luma.len() != luma_bytes
            || image.chroma.len() != chroma_bytes
        {
            return Err(UploadError::WrongSize {
                expected: luma_bytes + chroma_bytes,
                actual: image.luma.len() + image.chroma.len(),
            });
        }
        let planes = [
            (
                &mut target.luma,
                wgpu::TextureFormat::R8Unorm,
                (target.width, target.height),
                1,
                &image.luma,
            ),
            (
                &mut target.chroma,
                wgpu::TextureFormat::Rg8Unorm,
                (chroma_width, chroma_height),
                2,
                &image.chroma,
            ),
        ];
        for (slot, format, (width, height), bytes, data) in planes {
            let size = wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            };
            let (texture, _) = slot.get_or_insert_with(|| {
                let texture = device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("video plane"),
                    size,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                });
                let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
                (texture, view)
            });
            stage_copy(
                &mut self.staging,
                encoder,
                texture,
                data,
                width * bytes,
                height,
                size,
            )?;
        }
        target.write_params(
            queue,
            ConvertParams {
                layout: LAYOUT_YUV_420,
                has_alpha_plane: 0,
                yuv_flags: u32::from(image.full_range)
                    | (u32::from(image.matrix == YuvMatrix::Bt601) << 1),
                _pad: 0,
            },
        );
        let (Some((_, luma)), Some((_, chroma))) = (&target.luma, &target.chroma) else {
            return Ok(());
        };
        let (luma, chroma) = (luma.clone(), chroma.clone());
        self.convert(device, encoder, target, &luma, &self.no_alpha, &chroma);
        Ok(())
    }

    /// Records the conversion pass from `color` (and `alpha`, `chroma`) into
    /// `target`.
    fn convert(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        target: &VideoTexture,
        color: &wgpu::TextureView,
        alpha: &wgpu::TextureView,
        chroma: &wgpu::TextureView,
    ) {
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
                    resource: wgpu::BindingResource::TextureView(color),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(alpha),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(chroma),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(&self.chroma_sampler),
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
    }
}

impl VideoTexture {
    /// Sets the conversion settings, writing them only when they change.
    fn write_params(&mut self, queue: &wgpu::Queue, params: ConvertParams) {
        if self.params_written != Some(params) {
            queue.write_buffer(&self.params, 0, bytemuck::bytes_of(&params));
            self.params_written = Some(params);
        }
    }
}

/// Records the copy of `rows` rows of `row_bytes` bytes from `data` into
/// `texture`, through an upload buffer of `staging`.
fn stage_copy(
    staging: &mut StagingBelt,
    encoder: &mut wgpu::CommandEncoder,
    texture: &wgpu::Texture,
    data: &[u8],
    row_bytes: u32,
    rows: u32,
    size: wgpu::Extent3d,
) -> Result<(), UploadError> {
    // Rows of a buffer-to-texture copy start every 256 bytes.
    let stride = row_bytes.next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
    let Some(staged) = wgpu::BufferSize::new(u64::from(stride) * u64::from(rows)) else {
        return Ok(());
    };
    let alignment = wgpu::BufferSize::new(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT.into())
        .expect("the copy alignment is not zero");
    let slice = staging.allocate(staged, alignment);
    {
        let mut view = slice.get_mapped_range_mut()?;
        let (stride, row_bytes) = (stride as usize, row_bytes as usize);
        if stride == row_bytes {
            view.copy_from_slice(&data[..stride * rows as usize]);
        } else {
            for (row, line) in data.chunks_exact(row_bytes).take(rows as usize).enumerate() {
                view.slice(row * stride..row * stride + row_bytes)
                    .copy_from_slice(line);
            }
        }
    }
    encoder.copy_buffer_to_texture(
        wgpu::TexelCopyBufferInfo {
            buffer: slice.buffer(),
            layout: wgpu::TexelCopyBufferLayout {
                offset: slice.offset(),
                bytes_per_row: Some(stride),
                rows_per_image: Some(rows),
            },
        },
        texture.as_image_copy(),
        size,
    );
    Ok(())
}

/// Records the copy of a plane into its block texture, creating the texture
/// when the format changes.
fn write_plane(
    device: &wgpu::Device,
    staging: &mut StagingBelt,
    encoder: &mut wgpu::CommandEncoder,
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
    stage_copy(
        staging,
        encoder,
        &block.texture,
        &plane.data,
        row_bytes,
        height.div_ceil(4),
        size,
    )
}
