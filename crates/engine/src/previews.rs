//! Moving previews of every generator, then of every effect (on a moving
//! picture), side by side in one image, drawn while the interface shows
//! them.

use std::sync::Arc;

use prism_core::{Effect, Resolution};
use prism_fx::{CATALOG, EffectChain, EffectRenderer, FrameTime, GENERATORS, find_generator};
use prism_render::{Gpu, WORKING_FORMAT, wgpu};

use crate::engine::ProgramOutput;

/// Size of one preview, in pixels.
pub const PREVIEW_TILE: (u32, u32) = (192, 108);
/// Previews per row of the image; generators in the order of
/// [`GENERATORS`], then effects in the order of [`CATALOG`].
pub const PREVIEW_COLUMNS: u32 = 8;

fn preview_count() -> u32 {
    (GENERATORS.len() + CATALOG.len()) as u32
}

/// Where the preview of the effect at `index` of [`CATALOG`] sits in the
/// image, in texture coordinates (left, top, right, bottom).
pub fn effect_preview_rect(index: usize) -> [f32; 4] {
    preview_rect(GENERATORS.len() + index)
}

/// Where the preview of the generator at `index` of [`GENERATORS`] sits in
/// the image, in texture coordinates (left, top, right, bottom).
pub fn preview_rect(index: usize) -> [f32; 4] {
    let rows = preview_count().div_ceil(PREVIEW_COLUMNS);
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
    /// Generators, then effects.
    generators: Vec<Effect>,
    chains: Vec<EffectChain>,
    /// The moving picture effects are shown on, and what draws it.
    picture: Effect,
    picture_chain: EffectChain,
    picture_view: wgpu::TextureView,
    /// Engine time when the previews started, as a launch.
    launched: (f64, f64),
}

impl GeneratorPreviews {
    pub(crate) fn new(device: &wgpu::Device, launched: (f64, f64)) -> Self {
        let rows = preview_count().div_ceil(PREVIEW_COLUMNS);
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
        let picture_view = device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("effect preview picture"),
                size: wgpu::Extent3d {
                    width: PREVIEW_TILE.0,
                    height: PREVIEW_TILE.1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: WORKING_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&wgpu::TextureViewDescriptor::default());
        // A slow, colourful picture: a plasma through the rainbow palette.
        let mut picture =
            find_generator("plasma").map_or_else(|| Effect::new("plasma"), |info| info.create());
        picture.set_param("palette", 1.0);
        picture.set_param("speed", 0.3);
        let effects = GENERATORS.iter().chain(CATALOG);
        Self {
            image,
            tile,
            tile_view,
            generators: effects.clone().map(|info| info.create()).collect(),
            chains: effects.map(|_| EffectChain::default()).collect(),
            picture,
            picture_chain: EffectChain::default(),
            picture_view,
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
        self.picture_chain.apply_into(
            renderer,
            &gpu.device,
            &gpu.queue,
            encoder,
            &[&self.picture],
            blank,
            PREVIEW_TILE,
            time,
            &self.picture_view,
        );
        for (index, (generator, chain)) in self.generators.iter().zip(&mut self.chains).enumerate()
        {
            // Generators draw on nothing, effects on the picture.
            let input = if index < GENERATORS.len() {
                blank
            } else {
                &self.picture_view
            };
            let written = chain.apply_into(
                renderer,
                &gpu.device,
                &gpu.queue,
                encoder,
                &[generator],
                input,
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
