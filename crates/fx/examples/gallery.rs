//! Draws every effect and generator, at its default settings, on a test
//! pattern, side by side in one PNG image: to judge them by eye.
//!
//! ```text
//! cargo run -p prism-fx --example gallery -- OUTPUT.png
//! ```
//!
//! Tiles follow the order of the catalog, row by row, then the generators,
//! then variants of the generators of the procedural bank (other modes and
//! palettes); the first tile is the pattern without effect.

use std::process::ExitCode;

use prism_core::Effect;
use prism_fx::{CATALOG, EffectChain, EffectRenderer, FrameTime, GENERATORS};
use prism_render::{Gpu, WORKING_FORMAT, pixels_texture, read_texture, wgpu};

const WIDTH: u32 = 240;
const HEIGHT: u32 = 135;
const COLUMNS: u32 = 5;

/// Color gradients, a checkerboard and a white ring on a dark background.
fn pattern() -> Vec<[f32; 4]> {
    let mut pixels = Vec::with_capacity((WIDTH * HEIGHT) as usize);
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let (u, v) = (x as f32 / WIDTH as f32, y as f32 / HEIGHT as f32);
            let checker = if (x / 15 + y / 15) % 2 == 0 {
                0.15
            } else {
                0.0
            };
            let (dx, dy) = (
                x as f32 - WIDTH as f32 / 2.0,
                y as f32 - HEIGHT as f32 / 2.0,
            );
            let ring = ((dx * dx + dy * dy).sqrt() - 40.0).abs() < 4.0;
            let pixel = if ring {
                [1.0, 1.0, 1.0, 1.0]
            } else {
                [u * 0.9 + checker, v * 0.8 + checker, (1.0 - u) * 0.9, 1.0]
            };
            pixels.push(pixel);
        }
    }
    pixels
}

/// Generators of the procedural bank in other modes and palettes.
fn bank_variants() -> Vec<Effect> {
    let variant = |kind: &str, settings: &[(&str, f32)]| {
        let mut effect = Effect::new(kind);
        for (id, value) in settings {
            effect.set_param(*id, *value);
        }
        effect
    };
    vec![
        variant("fractal_noise", &[("mode", 1.0), ("palette", 2.0)]),
        variant("fractal_noise", &[("mode", 2.0), ("palette", 3.0)]),
        variant(
            "fractal_noise",
            &[("base", 0.0), ("mode", 3.0), ("palette", 6.0)],
        ),
        variant(
            "fractal_noise",
            &[("base", 1.0), ("palette", 1.0), ("levels", 6.0)],
        ),
        variant("cells", &[("mode", 1.0), ("palette", 5.0)]),
        variant("cells", &[("mode", 2.0), ("palette", 1.0)]),
        variant("cells", &[("metric", 1.0), ("palette", 4.0)]),
        variant("cells", &[("mode", 4.0), ("metric", 2.0), ("palette", 2.0)]),
        variant("domain_warp", &[("palette", 6.0)]),
        variant(
            "domain_warp",
            &[("palette", 4.0), ("strength", 3.0), ("warp_levels", 3.0)],
        ),
        variant(
            "plane_wave",
            &[("shape", 2.0), ("direction", 30.0), ("palette", 6.0)],
        ),
        variant(
            "plane_wave",
            &[("shape", 1.0), ("duty", 0.8), ("palette", 2.0)],
        ),
        variant("ripples", &[("palette", 4.0)]),
        variant(
            "ripples",
            &[("shape", 2.0), ("damping", 0.0), ("palette", 1.0)],
        ),
        variant("interference", &[("palette", 3.0)]),
        variant(
            "interference",
            &[
                ("sources", 5.0),
                ("layout", 2.0),
                ("render", 1.0),
                ("palette", 6.0),
            ],
        ),
        variant("plasma", &[("palette", 1.0)]),
        variant(
            "plasma",
            &[("terms", 7.0), ("complexity", 2.5), ("palette", 5.0)],
        ),
        variant("moire", &[("palette", 2.0)]),
        variant(
            "moire",
            &[
                ("pattern", 1.0),
                ("shift", 0.1),
                ("sharpness", 0.8),
                ("palette", 4.0),
            ],
        ),
        variant("spiral", &[("palette", 1.0)]),
        variant(
            "spiral",
            &[
                ("curve", 1.0),
                ("shape", 2.0),
                ("arms", 6.0),
                ("palette", 2.0),
            ],
        ),
        variant("line_waves", &[("palette", 3.0)]),
        variant(
            "line_waves",
            &[
                ("perspective", 1.0),
                ("noise", 0.7),
                ("lines", 40.0),
                ("palette", 3.0),
            ],
        ),
    ]
}

fn main() -> ExitCode {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: gallery OUTPUT.png");
        return ExitCode::FAILURE;
    };
    let gpu = match Gpu::headless() {
        Ok(gpu) => gpu,
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCode::FAILURE;
        }
    };
    let renderer = EffectRenderer::new(&gpu.device);
    let pattern = pattern();
    let input = pixels_texture(&gpu.device, &gpu.queue, WIDTH, HEIGHT, &pattern)
        .create_view(&wgpu::TextureViewDescriptor::default());
    let mut tiles: Vec<Vec<Effect>> = vec![Vec::new()];
    tiles.extend(
        CATALOG
            .iter()
            .chain(GENERATORS)
            .map(|info| vec![info.create()]),
    );
    tiles.extend(bank_variants().into_iter().map(|effect| vec![effect]));
    let count = tiles.len() as u32;
    let rows = count.div_ceil(COLUMNS);
    let (image_width, image_height) = (WIDTH * COLUMNS, HEIGHT * rows);
    let mut image = vec![0u8; (image_width * image_height * 4) as usize];

    for (tile, effects) in tiles.iter().enumerate() {
        let tile = tile as u32;
        let destination = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("tile"),
            size: wgpu::Extent3d {
                width: WIDTH,
                height: HEIGHT,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: WORKING_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let references: Vec<&Effect> = effects.iter().collect();
        let mut chain = EffectChain::default();
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        let time = FrameTime {
            seconds: 1.0,
            beat: 0.3,
            ..FrameTime::default()
        };
        let written = chain.apply_into(
            &renderer,
            &gpu.device,
            &gpu.queue,
            &mut encoder,
            &references,
            &input,
            (WIDTH, HEIGHT),
            time,
            &destination.create_view(&wgpu::TextureViewDescriptor::default()),
        );
        gpu.queue.submit([encoder.finish()]);
        let pixels = if written {
            match read_texture(&gpu.device, &gpu.queue, &destination) {
                Ok(pixels) => pixels,
                Err(error) => {
                    eprintln!("error: {error}");
                    return ExitCode::FAILURE;
                }
            }
        } else {
            pattern.clone()
        };
        let (left, top) = ((tile % COLUMNS) * WIDTH, (tile / COLUMNS) * HEIGHT);
        for (index, pixel) in pixels.iter().enumerate() {
            let (x, y) = (index as u32 % WIDTH + left, index as u32 / WIDTH + top);
            let at = ((y * image_width + x) * 4) as usize;
            // Premultiplied colors over black.
            for channel in 0..3 {
                image[at + channel] = (pixel[channel].clamp(0.0, 1.0) * 255.0).round() as u8;
            }
            image[at + 3] = 255;
        }
    }

    let file = match std::fs::File::create(&path) {
        Ok(file) => file,
        Err(error) => {
            eprintln!("error: cannot write {path}: {error}");
            return ExitCode::FAILURE;
        }
    };
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), image_width, image_height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    if let Err(error) = encoder
        .write_header()
        .and_then(|mut writer| writer.write_image_data(&image))
    {
        eprintln!("error: {error}");
        return ExitCode::FAILURE;
    }
    println!("image     {path}");
    ExitCode::SUCCESS
}
