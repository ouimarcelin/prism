//! GPU compositing compared with the CPU reference.
//!
//! These tests need a GPU. Without one they print a notice and pass, so the
//! rest of the suite still runs on machines without graphics.

#![allow(clippy::unwrap_used)]

use prism_core::{
    BlendMode, Clip, Composition, CrossfaderCurve, CrossfaderSide, Resolution, Slot, Unit,
};
use prism_render::reference::{self, Rgba};
use prism_render::{
    Compositor, Gpu, LayerInput, WORKING_FORMAT, plan_layers, read_texture, solid_color_texture,
};

/// Half-float precision on values up to 2.0.
const TOLERANCE: f32 = 2e-3;

const SIZE: Resolution = Resolution {
    width: 48,
    height: 20,
};

fn gpu() -> Option<Gpu> {
    match Gpu::headless() {
        Ok(gpu) => Some(gpu),
        Err(error) => {
            eprintln!("skipping GPU test: {error}");
            None
        }
    }
}

struct Layer {
    view: wgpu::TextureView,
    opacity: f32,
    blend_mode: BlendMode,
}

fn layer(gpu: &Gpu, color: Rgba, opacity: f32, blend_mode: BlendMode) -> Layer {
    let texture = solid_color_texture(&gpu.device, &gpu.queue, color);
    Layer {
        view: texture.create_view(&Default::default()),
        opacity,
        blend_mode,
    }
}

/// Composites the layers and checks that every pixel equals `expected`.
fn assert_composite(gpu: &Gpu, layers: &[Layer], expected: Rgba) {
    let inputs: Vec<_> = layers
        .iter()
        .map(|layer| LayerInput {
            texture: &layer.view,
            opacity: layer.opacity,
            blend_mode: layer.blend_mode,
        })
        .collect();
    let mut compositor = Compositor::new(&gpu.device, SIZE);
    let output = compositor.render(&gpu.device, &gpu.queue, &inputs);
    assert_eq!(output.format(), WORKING_FORMAT);
    assert_eq!((output.width(), output.height()), (SIZE.width, SIZE.height));

    let pixels = read_texture(&gpu.device, &gpu.queue, output).unwrap();
    assert_eq!(pixels.len(), (SIZE.width * SIZE.height) as usize);
    for (index, pixel) in pixels.iter().enumerate() {
        for channel in 0..4 {
            assert!(
                (pixel[channel] - expected[channel]).abs() < TOLERANCE,
                "pixel {index}: expected {expected:?}, got {pixel:?}"
            );
        }
    }
}

/// Expected result of the CPU reference for the same layers.
fn reference(layers: &[(Rgba, f32, BlendMode)]) -> Rgba {
    layers
        .iter()
        .fold([0.0, 0.0, 0.0, 1.0], |below, &(color, opacity, mode)| {
            reference::composite(below, color, opacity, mode)
        })
}

const BASE: Rgba = [0.5, 0.25, 0.75, 1.0];
const GRAY: Rgba = [0.5, 0.5, 0.5, 1.0];

#[test]
fn no_layer_gives_opaque_black() {
    let Some(gpu) = gpu() else { return };
    assert_composite(&gpu, &[], [0.0, 0.0, 0.0, 1.0]);
}

#[test]
fn every_blend_mode_matches_the_reference() {
    let Some(gpu) = gpu() else { return };
    for mode in BlendMode::ALL {
        for opacity in [1.0, 0.5] {
            let expected = reference(&[(BASE, 1.0, BlendMode::Normal), (GRAY, opacity, mode)]);
            let layers = [
                layer(&gpu, BASE, 1.0, BlendMode::Normal),
                layer(&gpu, GRAY, opacity, mode),
            ];
            assert_composite(&gpu, &layers, expected);
        }
    }
}

#[test]
fn additive_blend_keeps_values_above_one() {
    let Some(gpu) = gpu() else { return };
    let layers = [
        layer(&gpu, BASE, 1.0, BlendMode::Normal),
        layer(&gpu, GRAY, 1.0, BlendMode::Add),
    ];
    assert_composite(&gpu, &layers, [1.0, 0.75, 1.25, 1.0]);
}

#[test]
fn layer_alpha_is_respected() {
    let Some(gpu) = gpu() else { return };
    let red = [1.0, 0.0, 0.0, 1.0];
    let half_white = [0.5, 0.5, 0.5, 0.5];
    let layers = [
        layer(&gpu, red, 1.0, BlendMode::Normal),
        layer(&gpu, half_white, 1.0, BlendMode::Normal),
    ];
    assert_composite(&gpu, &layers, [1.0, 0.5, 0.5, 1.0]);
}

#[test]
fn layer_order_matters() {
    let Some(gpu) = gpu() else { return };
    let red = [1.0, 0.0, 0.0, 1.0];
    let blue = [0.0, 0.0, 1.0, 1.0];
    let layers = [
        layer(&gpu, red, 1.0, BlendMode::Normal),
        layer(&gpu, blue, 1.0, BlendMode::Normal),
    ];
    assert_composite(&gpu, &layers, blue);
    let layers = [
        layer(&gpu, blue, 1.0, BlendMode::Normal),
        layer(&gpu, red, 1.0, BlendMode::Normal),
    ];
    assert_composite(&gpu, &layers, red);
}

#[test]
fn many_layers_grow_the_parameter_buffer() {
    let Some(gpu) = gpu() else { return };
    let step = [0.05, 0.05, 0.05, 1.0];
    let mut layers = vec![layer(&gpu, [0.0, 0.0, 0.0, 1.0], 1.0, BlendMode::Normal)];
    let mut spec = vec![([0.0, 0.0, 0.0, 1.0], 1.0, BlendMode::Normal)];
    for _ in 0..12 {
        layers.push(layer(&gpu, step, 1.0, BlendMode::Add));
        spec.push((step, 1.0, BlendMode::Add));
    }
    assert_composite(&gpu, &layers, reference(&spec));
}

#[test]
fn resize_changes_the_output_size() {
    let Some(gpu) = gpu() else { return };
    let mut compositor = Compositor::new(&gpu.device, SIZE);
    let bigger = Resolution {
        width: 64,
        height: 36,
    };
    compositor.resize(&gpu.device, bigger);
    let output = compositor.render(&gpu.device, &gpu.queue, &[]);
    assert_eq!((output.width(), output.height()), (64, 36));
    assert_eq!(compositor.resolution(), bigger);
}

#[test]
fn composition_state_drives_the_mix() {
    let Some(gpu) = gpu() else { return };
    // Bottom layer red on side A, top layer blue on side B, linear crossfader.
    let mut comp = Composition::default();
    let column = comp.columns()[0].id();
    let ids: Vec<_> = comp.layers().iter().map(|layer| layer.id()).collect();
    for (&id, side) in ids.iter().zip([CrossfaderSide::A, CrossfaderSide::B]) {
        comp.set_clip(Slot::new(id, column), Some(Clip::from_file("/x.mov")))
            .unwrap();
        comp.layer_mut(id).unwrap().crossfader_side = Some(side);
    }
    comp.trigger_column(column).unwrap();
    comp.crossfader.curve = CrossfaderCurve::Linear;
    comp.crossfader.position = Unit::new(0.25);

    let red = solid_color_texture(&gpu.device, &gpu.queue, [1.0, 0.0, 0.0, 1.0]);
    let blue = solid_color_texture(&gpu.device, &gpu.queue, [0.0, 0.0, 1.0, 1.0]);
    let red = red.create_view(&Default::default());
    let blue = blue.create_view(&Default::default());

    let plan = plan_layers(&comp);
    let inputs: Vec<_> = plan
        .iter()
        .map(|p| LayerInput {
            texture: if p.slot.layer == ids[0] { &red } else { &blue },
            opacity: p.opacity,
            blend_mode: p.blend_mode,
        })
        .collect();
    let mut compositor = Compositor::new(&gpu.device, SIZE);
    let output = compositor.render(&gpu.device, &gpu.queue, &inputs);
    let pixel = read_texture(&gpu.device, &gpu.queue, output).unwrap()[0];

    // Red at 0.75 over black, then blue at 0.25 over it.
    let expected = [0.5625, 0.0, 0.25, 1.0];
    for channel in 0..4 {
        assert!(
            (pixel[channel] - expected[channel]).abs() < TOLERANCE,
            "expected {expected:?}, got {pixel:?}"
        );
    }
}
