//! Moving previews of every generator, side by side in one image, drawn
//! while the interface shows its generator browser.

use std::sync::Arc;

use prism_core::{Effect, Resolution};
use prism_fx::{EffectChain, EffectRenderer, FrameTime, GENERATORS};
use prism_render::{Gpu, WORKING_FORMAT, wgpu};

use crate::engine::ProgramOutput;

/// Size of one preview, in pixels.
pub const PREVIEW_TILE: (u32, u32) = (192, 108);
/// Previews per row of the image; generators in the order of
/// [`GENERATORS`].
pub const PREVIEW_COLUMNS: u32 = 8;

/// Where the preview of the generator at `index` of [`GENERATORS`] sits in
/// the image, in texture coordinates (left, top, right, bottom).
pub fn preview_rect(index: usize) -> [f32; 4] {
    let rows = (GENERATORS.len() as u32).div_ceil(PREVIEW_COLUMNS);
    let column = index as u32 % PREVIEW_COLUMNS;
    let row = index as u32 / PREVIEW_COLUMNS;
    let (width, height) = (PREVIEW_COLUMNS as f32, rows as f32);
    [
        column as f32 / width,
        row as f32 / height,
        (column + 1) as f32 / width,
        (row + 1) as f32 / height,
    ]
}

/// The previews and what draws them: one chain per generator, so that
/// generators with memory keep theirs.
pub(crate) struct GeneratorPreviews {
    image: Arc<ProgramOutput>,
    tile: wgpu::Texture,
    tile_view: wgpu::TextureView,
    generators: Vec<Effect>,
    chains: Vec<EffectChain>,
    /// Engine time when the previews started, as a launch.
    launched: (f64, f64),
}

impl GeneratorPreviews {
    pub(crate) fn new(device: &wgpu::Device, launched: (f64, f64)) -> Self {
        let rows = (GENERATORS.len() as u32).div_ceil(PREVIEW_COLUMNS);
        let image = Arc::new(ProgramOutput::new(
            device,
            Resolution {
                width: PREVIEW_TILE.0 * PREVIEW_COLUMNS,
                height: PREVIEW_TILE.1 * rows,
            },
        ));
        let tile = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("generator preview"),
            size: wgpu::Extent3d {
                width: PREVIEW_TILE.0,
                height: PREVIEW_TILE.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: WORKING_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let tile_view = tile.create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            image,
            tile,
            tile_view,
            generators: GENERATORS.iter().map(|info| info.create()).collect(),
            chains: GENERATORS.iter().map(|_| EffectChain::default()).collect(),
            launched,
        }
    }

    /// The image of all the previews.
    pub(crate) fn image(&self) -> Arc<ProgramOutput> {
        Arc::clone(&self.image)
    }

    /// Records every preview at `time`, each generator with its default
    /// settings.
    pub(crate) fn encode(
        &mut self,
        renderer: &EffectRenderer,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        time: FrameTime,
        blank: &wgpu::TextureView,
    ) {
        let mut time = time;
        (time.launch_beat, time.launch_seconds) = self.launched;
        for (index, (generator, chain)) in self.generators.iter().zip(&mut self.chains).enumerate()
        {
            let written = chain.apply_into(
                renderer,
                &gpu.device,
                &gpu.queue,
                encoder,
                &[generator],
                blank,
                PREVIEW_TILE,
                time,
                &self.tile_view,
            );
            if !written {
                continue;
            }
            let column = index as u32 % PREVIEW_COLUMNS;
            let row = index as u32 / PREVIEW_COLUMNS;
            encoder.copy_texture_to_texture(
                self.tile.as_image_copy(),
                wgpu::TexelCopyTextureInfo {
                    texture: self.image.texture(),
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: column * PREVIEW_TILE.0,
                        y: row * PREVIEW_TILE.1,
                        z: 0,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::Extent3d {
                    width: PREVIEW_TILE.0,
                    height: PREVIEW_TILE.1,
                    depth_or_array_layers: 1,
                },
            );
        }
    }
}
