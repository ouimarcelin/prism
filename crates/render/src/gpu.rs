//! GPU device setup and texture helpers.

use std::sync::mpsc;

/// Texture format of every intermediate image of the compositor.
///
/// 16-bit float keeps precision through many layers and lets additive blends
/// exceed 1.0 until the final output.
pub const WORKING_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

const WORKING_FORMAT_BYTES: u32 = 8;

/// Error raised while opening the GPU.
#[derive(Debug, thiserror::Error)]
pub enum GpuError {
    /// No adapter matches the request.
    #[error("no compatible GPU adapter found")]
    NoAdapter(#[source] wgpu::RequestAdapterError),
    /// The adapter refused to open a device.
    #[error("cannot open the GPU device")]
    Device(#[from] wgpu::RequestDeviceError),
}

/// Error raised while reading a texture back to the CPU.
#[derive(Debug, thiserror::Error)]
pub enum ReadbackError {
    /// Only [`WORKING_FORMAT`] textures can be read back.
    #[error("cannot read back a texture in format {0:?}")]
    UnsupportedFormat(wgpu::TextureFormat),
    /// Waiting for the GPU failed.
    #[error("waiting for the GPU failed")]
    Poll(#[from] wgpu::PollError),
    /// Mapping the staging buffer failed.
    #[error("cannot map the readback buffer")]
    Map(#[from] wgpu::BufferAsyncError),
    /// The mapped range could not be accessed.
    #[error("cannot access the mapped readback buffer")]
    MapRange(#[from] wgpu::MapRangeError),
}

/// Instance, adapter, device and queue shared by the render engine, the
/// outputs and the interface.
///
/// `wgpu` objects are thread-safe: the render thread and the interface can
/// both hold references to the same device.
#[derive(Debug)]
pub struct Gpu {
    /// Entry point, needed to create window surfaces.
    pub instance: wgpu::Instance,
    /// Physical GPU.
    pub adapter: wgpu::Adapter,
    /// Logical device.
    pub device: wgpu::Device,
    /// Command queue.
    pub queue: wgpu::Queue,
}

impl Gpu {
    /// Opens the high-performance GPU without any window surface. Used by
    /// tests, benchmarks and offline rendering.
    pub fn headless() -> Result<Self, GpuError> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        pollster::block_on(Self::from_instance(&instance, None))
    }

    /// Opens a GPU able to present to `compatible_surface`, if given.
    pub async fn from_instance(
        instance: &wgpu::Instance,
        compatible_surface: Option<&wgpu::Surface<'_>>,
    ) -> Result<Self, GpuError> {
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface,
                ..Default::default()
            })
            .await
            .map_err(GpuError::NoAdapter)?;
        // HAP frames are uploaded block-compressed when the GPU allows it.
        let required_features = adapter.features() & wgpu::Features::TEXTURE_COMPRESSION_BC;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("prism"),
                required_features,
                // Large compositions need the adapter's real texture limits.
                required_limits: adapter.limits(),
                ..Default::default()
            })
            .await?;
        Ok(Self {
            instance: instance.clone(),
            adapter,
            device,
            queue,
        })
    }
}

impl Gpu {
    /// Whether BC1/BC3/BC4/BC7 textures can be uploaded as is.
    pub fn supports_block_compression(&self) -> bool {
        self.device
            .features()
            .contains(wgpu::Features::TEXTURE_COMPRESSION_BC)
    }
}

/// Creates a 1 × 1 texture of a premultiplied color, usable as a layer.
pub fn solid_color_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    color: [f32; 4],
) -> wgpu::Texture {
    pixels_texture(device, queue, 1, 1, &[color])
}

/// Creates a [`WORKING_FORMAT`] texture from premultiplied pixels, row by
/// row from the top, usable as a layer or as the source of an effect.
///
/// # Panics
///
/// When `pixels` does not hold `width × height` pixels.
pub fn pixels_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    width: u32,
    height: u32,
    pixels: &[[f32; 4]],
) -> wgpu::Texture {
    assert_eq!(
        pixels.len(),
        width as usize * height as usize,
        "pixel count does not match the size"
    );
    let size = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("pixels"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: WORKING_FORMAT,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let texels: Vec<u8> = pixels
        .iter()
        .flatten()
        .flat_map(|&channel| half::f16::from_f32(channel).to_le_bytes())
        .collect();
    queue.write_texture(
        texture.as_image_copy(),
        &texels,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(WORKING_FORMAT_BYTES * width),
            rows_per_image: Some(height),
        },
        size,
    );
    texture
}

/// Reads a [`WORKING_FORMAT`] texture back as premultiplied RGBA, row by row
/// from the top. Blocks until the GPU is done. Meant for tests, thumbnails and
/// screenshots, not for the real-time path.
pub fn read_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
) -> Result<Vec<[f32; 4]>, ReadbackError> {
    if texture.format() != WORKING_FORMAT {
        return Err(ReadbackError::UnsupportedFormat(texture.format()));
    }
    let size = texture.size();
    let row_bytes = size.width * WORKING_FORMAT_BYTES;
    let padded_row_bytes =
        row_bytes.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;

    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: u64::from(padded_row_bytes) * u64::from(size.height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("readback"),
    });
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_row_bytes),
                rows_per_image: Some(size.height),
            },
        },
        size,
    );
    queue.submit([encoder.finish()]);

    let slice = buffer.slice(..);
    let (sender, receiver) = mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = sender.send(result);
    });
    device.poll(wgpu::PollType::wait_indefinitely())?;
    receiver
        .recv()
        .map_err(|_| ReadbackError::Map(wgpu::BufferAsyncError))??;

    let data = slice.get_mapped_range()?;
    let mut pixels = Vec::with_capacity((size.width * size.height) as usize);
    for row in data.chunks_exact(padded_row_bytes as usize) {
        // Four little-endian half floats per texel.
        let (texels, _) = row[..row_bytes as usize].as_chunks::<8>();
        for texel in texels {
            let mut pixel = [0.0; 4];
            for (channel, bytes) in pixel.iter_mut().zip(texel.as_chunks::<2>().0) {
                *channel = half::f16::from_le_bytes(*bytes).to_f32();
            }
            pixels.push(pixel);
        }
    }
    drop(data);
    buffer.unmap();
    Ok(pixels)
}
