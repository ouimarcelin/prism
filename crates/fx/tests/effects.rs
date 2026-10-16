//! Effects on the GPU, checked pixel by pixel.
//!
//! These tests need a GPU. Without one they print a notice and pass.

#![allow(clippy::unwrap_used)]

use prism_core::Effect;
use prism_fx::{EffectChain, EffectRenderer, FrameTime, find, find_generator, is_active};
use prism_render::{Gpu, WORKING_FORMAT, pixels_texture, read_texture, wgpu};

/// Half-float precision on values up to 1.
const TOLERANCE: f32 = 3e-3;

fn gpu() -> Option<Gpu> {
    match Gpu::headless() {
        Ok(gpu) => Some(gpu),
        Err(error) => {
            eprintln!("skipping GPU test: {error}");
            None
        }
    }
}

fn effect(kind: &str, params: &[(&str, f32)]) -> Effect {
    let mut effect = find(kind).unwrap().create();
    for (name, value) in params {
        effect.set_param(*name, *value);
    }
    effect
}

/// Runs `effects` on an image of `width` × `height` pixels and returns the
/// result, row by row; `None` when no effect applies.
fn run(
    gpu: &Gpu,
    effects: &[Effect],
    width: u32,
    height: u32,
    pixels: &[[f32; 4]],
) -> Option<Vec<[f32; 4]>> {
    run_at(gpu, effects, width, height, pixels, FrameTime::default())
}

/// Like `run`, at a given time of the engine.
fn run_at(
    gpu: &Gpu,
    effects: &[Effect],
    width: u32,
    height: u32,
    pixels: &[[f32; 4]],
    time: FrameTime,
) -> Option<Vec<[f32; 4]>> {
    let renderer = EffectRenderer::new(&gpu.device);
    let input = pixels_texture(&gpu.device, &gpu.queue, width, height, pixels)
        .create_view(&wgpu::TextureViewDescriptor::default());
    let destination = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("result"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: WORKING_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let effects: Vec<&Effect> = effects.iter().collect();
    let mut chain = EffectChain::default();
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    let written = chain.apply_into(
        &renderer,
        &gpu.device,
        &gpu.queue,
        &mut encoder,
        &effects,
        &input,
        (width, height),
        time,
        &destination.create_view(&wgpu::TextureViewDescriptor::default()),
    );
    gpu.queue.submit([encoder.finish()]);
    written.then(|| read_texture(&gpu.device, &gpu.queue, &destination).unwrap())
}

fn assert_close(actual: [f32; 4], expected: [f32; 4], tolerance: f32, what: &str) {
    for channel in 0..4 {
        assert!(
            (actual[channel] - expected[channel]).abs() < tolerance,
            "{what}: expected {expected:?}, got {actual:?}"
        );
    }
}

/// An image with its left half `left` and its right half `right`.
fn halves(width: u32, height: u32, left: [f32; 4], right: [f32; 4]) -> Vec<[f32; 4]> {
    (0..width * height)
        .map(|index| {
            if index % width < width / 2 {
                left
            } else {
                right
            }
        })
        .collect()
}

#[test]
fn invert_works_on_straight_colors() {
    let Some(gpu) = gpu() else { return };
    let pixels = [[0.2, 0.4, 0.6, 1.0], [0.1, 0.2, 0.3, 0.5]];
    let result = run(&gpu, &[effect("invert", &[])], 2, 1, &pixels).unwrap();
    assert_close(result[0], [0.8, 0.6, 0.4, 1.0], TOLERANCE, "opaque");
    assert_close(result[1], [0.4, 0.3, 0.2, 0.5], TOLERANCE, "translucent");
}

#[test]
fn chains_skip_bypassed_and_unknown_effects() {
    let Some(gpu) = gpu() else { return };
    let pixel = [0.2, 0.4, 0.6, 1.0];
    let mut bypassed = effect("invert", &[]);
    bypassed.bypass = true;
    let unknown = Effect::new("from a later version");
    assert!(!is_active(&[&bypassed, &unknown]));
    assert_eq!(
        run(&gpu, &[bypassed.clone(), unknown.clone()], 1, 1, &[pixel]),
        None
    );

    // Two inversions around the skipped ones: back to the start.
    let chain = [
        effect("invert", &[]),
        bypassed,
        unknown,
        effect("invert", &[]),
    ];
    let result = run(&gpu, &chain, 1, 1, &[pixel]).unwrap();
    assert_close(result[0], pixel, TOLERANCE, "twice inverted");
}

#[test]
fn color_adjusts_brightness_saturation_and_hue() {
    let Some(gpu) = gpu() else { return };
    let gray = [0.5, 0.5, 0.5, 1.0];
    let red = [1.0, 0.0, 0.0, 1.0];
    let brighter = run(
        &gpu,
        &[effect("color", &[("brightness", 0.1)])],
        1,
        1,
        &[gray],
    );
    assert_close(
        brighter.unwrap()[0],
        [0.6, 0.6, 0.6, 1.0],
        TOLERANCE,
        "brightness",
    );
    let flat = run(&gpu, &[effect("color", &[("contrast", 0.0)])], 1, 1, &[red]);
    assert_close(flat.unwrap()[0], gray, TOLERANCE, "no contrast");
    let gray_red = run(
        &gpu,
        &[effect("color", &[("saturation", 0.0)])],
        1,
        1,
        &[red],
    );
    let luma = 0.2126;
    assert_close(
        gray_red.unwrap()[0],
        [luma, luma, luma, 1.0],
        TOLERANCE,
        "no saturation",
    );
    let green = run(&gpu, &[effect("color", &[("hue", 120.0)])], 1, 1, &[red]);
    assert_close(green.unwrap()[0], [0.0, 1.0, 0.0, 1.0], TOLERANCE, "hue");
}

#[test]
fn transform_scales_and_moves_around_the_center() {
    let Some(gpu) = gpu() else { return };
    let color = [0.3, 0.6, 0.9, 1.0];
    let pixels = vec![color; 64];
    let at = |result: &[[f32; 4]], x: usize, y: usize| result[y * 8 + x];

    let smaller = run(
        &gpu,
        &[effect("transform", &[("scale", 0.5)])],
        8,
        8,
        &pixels,
    )
    .unwrap();
    assert_close(at(&smaller, 0, 0), [0.0; 4], TOLERANCE, "corner");
    assert_close(at(&smaller, 3, 4), color, TOLERANCE, "center");

    // Half a width to the right: the left half is empty.
    let moved = run(&gpu, &[effect("transform", &[("x", 0.5)])], 8, 8, &pixels).unwrap();
    assert_close(at(&moved, 1, 4), [0.0; 4], TOLERANCE, "left");
    assert_close(at(&moved, 6, 4), color, TOLERANCE, "right");
}

#[test]
fn mirror_reflects_one_half() {
    let Some(gpu) = gpu() else { return };
    let red = [1.0, 0.0, 0.0, 1.0];
    let blue = [0.0, 0.0, 1.0, 1.0];
    let pixels = halves(8, 2, red, blue);
    let left = run(&gpu, &[effect("mirror", &[])], 8, 2, &pixels).unwrap();
    assert!(left.iter().all(|pixel| (pixel[0] - 1.0).abs() < TOLERANCE));
    let right = run(&gpu, &[effect("mirror", &[("mode", 1.0)])], 8, 2, &pixels).unwrap();
    assert!(right.iter().all(|pixel| (pixel[2] - 1.0).abs() < TOLERANCE));
}

#[test]
fn kaleidoscope_and_blur_keep_a_plain_image() {
    let Some(gpu) = gpu() else { return };
    let color = [0.25, 0.5, 0.75, 1.0];
    let pixels = vec![color; 16 * 8];
    for chain in [
        vec![effect(
            "kaleidoscope",
            &[("segments", 5.0), ("rotation", 30.0)],
        )],
        vec![effect("blur", &[("radius", 6.0)])],
    ] {
        let result = run(&gpu, &chain, 16, 8, &pixels).unwrap();
        for pixel in result {
            assert_close(pixel, color, TOLERANCE, &chain[0].kind);
        }
    }
}

#[test]
fn blur_softens_an_edge() {
    let Some(gpu) = gpu() else { return };
    let black = [0.0, 0.0, 0.0, 1.0];
    let white = [1.0, 1.0, 1.0, 1.0];
    let pixels = halves(32, 4, black, white);
    let result = run(&gpu, &[effect("blur", &[("radius", 4.0)])], 32, 4, &pixels).unwrap();
    let row: Vec<f32> = result[..32].iter().map(|pixel| pixel[0]).collect();
    // Far from the edge, unchanged; at the edge, in between; increasing.
    assert!(row[0] < 0.01 && row[31] > 0.99, "{row:?}");
    assert!(row[15] > 0.1 && row[15] < 0.5, "{row:?}");
    assert!(row[16] > 0.5 && row[16] < 0.9, "{row:?}");
    assert!(
        row.windows(2).all(|pair| pair[0] <= pair[1] + 1e-3),
        "{row:?}"
    );
}

/// A varied opaque image: a gradient across, a different one down.
fn gradient(width: u32, height: u32) -> Vec<[f32; 4]> {
    (0..height)
        .flat_map(|y| {
            (0..width).map(move |x| {
                let across = (x as f32 + 0.5) / width as f32;
                let down = (y as f32 + 0.5) / height as f32;
                [across, down, 1.0 - across, 1.0]
            })
        })
        .collect()
}

#[test]
fn neutral_settings_leave_the_image_unchanged() {
    let Some(gpu) = gpu() else { return };
    let pixels = gradient(16, 8);
    for neutral in [
        effect("pixelate", &[("size", 1.0)]),
        effect("rgb_split", &[("distance", 0.0)]),
        effect("glitch", &[("amount", 0.0)]),
        effect("wave", &[("amplitude", 0.0)]),
        effect("swirl", &[("angle", 0.0)]),
        effect("tile", &[("columns", 1.0), ("rows", 1.0)]),
        effect("vignette", &[("amount", 0.0)]),
        effect("strobe", &[("duty", 1.0)]),
        effect("colorize", &[("amount", 0.0)]),
        effect("edges", &[("mix", 0.0)]),
    ] {
        let result = run(&gpu, std::slice::from_ref(&neutral), 16, 8, &pixels).unwrap();
        for (actual, expected) in result.iter().zip(&pixels) {
            assert_close(*actual, *expected, TOLERANCE, &neutral.kind);
        }
    }
}

#[test]
fn levels_posterize_and_threshold() {
    let Some(gpu) = gpu() else { return };
    let pixels = [[0.4, 0.4, 0.4, 1.0], [0.6, 0.6, 0.6, 1.0]];
    let two = run(
        &gpu,
        &[effect("posterize", &[("levels", 2.0)])],
        2,
        1,
        &pixels,
    )
    .unwrap();
    assert_close(two[0], [0.0, 0.0, 0.0, 1.0], TOLERANCE, "posterize dark");
    assert_close(two[1], [1.0, 1.0, 1.0, 1.0], TOLERANCE, "posterize light");
    let cut = run(
        &gpu,
        &[effect("threshold", &[("softness", 0.0)])],
        2,
        1,
        &pixels,
    )
    .unwrap();
    assert_close(cut[0], [0.0, 0.0, 0.0, 1.0], TOLERANCE, "threshold dark");
    assert_close(cut[1], [1.0, 1.0, 1.0, 1.0], TOLERANCE, "threshold light");
}

#[test]
fn pixelate_tile_and_edges_follow_the_layout() {
    let Some(gpu) = gpu() else { return };
    let red = [1.0, 0.0, 0.0, 1.0];
    let blue = [0.0, 0.0, 1.0, 1.0];
    let pixels = halves(8, 2, red, blue);

    // One cell as wide as the image: one color everywhere.
    let big = run(&gpu, &[effect("pixelate", &[("size", 8.0)])], 8, 2, &pixels).unwrap();
    assert!(
        big.windows(2)
            .all(|pair| (pair[0][0] - pair[1][0]).abs() < TOLERANCE)
    );

    // Two tiles across: red, blue, red, blue by quarters.
    let tiles = run(&gpu, &[effect("tile", &[("rows", 1.0)])], 8, 2, &pixels).unwrap();
    let reds: Vec<bool> = tiles[..8].iter().map(|pixel| pixel[0] > 0.5).collect();
    assert_eq!(reds, [true, true, false, false, true, true, false, false]);

    // Edges: dark on the plain halves, light at the border.
    let outline = run(&gpu, &[effect("edges", &[])], 8, 2, &pixels).unwrap();
    assert!(outline[0][1] < 0.01 && outline[7][1] < 0.01, "{outline:?}");
    assert!(outline[3][1] > 0.2 && outline[4][1] > 0.2, "{outline:?}");
}

#[test]
fn strobe_follows_the_beat() {
    let Some(gpu) = gpu() else { return };
    let color = [0.3, 0.6, 0.9, 1.0];
    let strobe = [effect("strobe", &[("rate", 1.0), ("duty", 0.5)])];
    let at = |beat: f64| {
        let time = FrameTime { seconds: 0.0, beat };
        run_at(&gpu, &strobe, 1, 1, &[color], time).unwrap()[0]
    };
    assert_close(at(10.25), color, TOLERANCE, "first half of the beat");
    assert_close(at(10.75), [0.0, 0.0, 0.0, 1.0], TOLERANCE, "second half");
}

#[test]
fn colorize_and_vignette_change_colors() {
    let Some(gpu) = gpu() else { return };
    let red = [1.0, 0.0, 0.0, 1.0];
    let gray = run(
        &gpu,
        &[effect("colorize", &[("saturation", 0.0)])],
        1,
        1,
        &[red],
    )
    .unwrap();
    let luma = 0.2126;
    assert_close(gray[0], [luma, luma, luma, 1.0], TOLERANCE, "colorize");

    let white = vec![[1.0; 4]; 81];
    let shaded = run(
        &gpu,
        &[effect("vignette", &[("amount", 1.0)])],
        9,
        9,
        &white,
    )
    .unwrap();
    assert!(shaded[0][0] < 0.5, "corner {:?}", shaded[0]);
    assert_close(shaded[40], [1.0; 4], TOLERANCE, "center");
}

#[test]
fn a_beat_animation_is_still_on_the_beat_after_ten_minutes() {
    use prism_clock::BeatClock;
    use prism_core::{Animation, Period, Shape};
    use std::time::{Duration, Instant};

    // The criterion of phase 2: an effect on the beat stays in time after
    // 10 minutes. Here every frame of the tenth minute at 60 per second,
    // 128 BPM: the effect is on for the first half of each beat.
    let invert = find("invert").unwrap();
    let mut effect = invert.create();
    effect.animations.insert(
        "amount".into(),
        Animation {
            shape: Shape::Square,
            period: Period::Beats(1.0),
            ..Animation::new(0.0, 1.0)
        },
    );
    let start = Instant::now();
    let clock = BeatClock::new(128.0, start);
    for frame in 9 * 3600..10 * 3600 {
        let seconds = f64::from(frame) / 60.0;
        let time = FrameTime {
            seconds,
            beat: clock.beat_at(start + Duration::from_secs_f64(seconds)),
        };
        let beats = seconds * 128.0 / 60.0;
        let within = beats - beats.floor();
        if (within - 0.5).abs() < 1e-6 || within < 1e-6 {
            continue;
        }
        let expected = if within < 0.5 { 1.0 } else { 0.0 };
        assert_eq!(
            invert.values_at(&effect, time)[0],
            expected,
            "frame {frame}"
        );
    }
}

fn generator(kind: &str, params: &[(&str, f32)]) -> Effect {
    let mut generator = find_generator(kind).unwrap().create();
    for (name, value) in params {
        generator.set_param(*name, *value);
    }
    generator
}

#[test]
fn generators_draw_without_input() {
    let Some(gpu) = gpu() else { return };
    // The input is ignored: a black image.
    let black = vec![[0.0, 0.0, 0.0, 1.0]; 8];

    let solid = generator("solid", &[("saturation", 1.0), ("opacity", 0.5)]);
    let red = run(&gpu, &[solid], 4, 2, &black).unwrap();
    assert_close(red[5], [0.5, 0.0, 0.0, 0.5], TOLERANCE, "solid");

    let checker = generator("checker", &[("count", 2.0), ("rate", 0.0)]);
    let squares = run(&gpu, &[checker], 4, 2, &black).unwrap();
    assert_close(squares[0], [0.0, 0.0, 0.0, 1.0], TOLERANCE, "dark square");
    assert_close(squares[2], [1.0, 1.0, 1.0, 1.0], TOLERANCE, "light square");

    let gradient = generator(
        "gradient",
        &[("hue_from", 0.0), ("hue_to", 240.0), ("saturation", 1.0)],
    );
    let across = run(&gpu, &[gradient], 32, 1, &[[0.0; 4]; 32]).unwrap();
    assert!(across[0][0] > 0.9 && across[0][2] < 0.1, "{:?}", across[0]);
    assert!(
        across[31][2] > 0.9 && across[31][0] < 0.1,
        "{:?}",
        across[31]
    );

    let noise = run(&gpu, &[generator("noise", &[])], 32, 16, &[[0.0; 4]; 512]).unwrap();
    let levels: Vec<f32> = noise.iter().map(|pixel| pixel[0]).collect();
    assert!(levels.iter().all(|level| (0.0..=1.0).contains(level)));
    let spread = levels.iter().copied().fold(f32::MIN, f32::max)
        - levels.iter().copied().fold(f32::MAX, f32::min);
    assert!(spread > 0.2, "noise too flat: {spread}");

    // Generators then effects: inverted lines.
    let chain = [generator("lines", &[("count", 4.0)]), effect("invert", &[])];
    let lines = run(&gpu, &chain, 128, 2, &[[0.0; 4]; 256]).unwrap();
    assert!(lines.iter().any(|pixel| pixel[0] > 0.9));
    assert!(lines.iter().any(|pixel| pixel[0] < 0.1));
}
