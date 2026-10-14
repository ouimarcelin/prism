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
    Compositor, Gpu, LayerInput, Transition, WORKING_FORMAT, plan_layers, read_texture,
    solid_color_texture,
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
        .map(|layer| LayerInput::new(&layer.view, layer.opacity, layer.blend_mode))
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
    compositor.resize(bigger);
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
        .map(|p| {
            let texture = if p.slot.layer == ids[0] { &red } else { &blue };
            LayerInput::new(texture, p.opacity, p.blend_mode)
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

#[test]
fn transition_mixes_previous_and_new_clip_before_blending() {
    let Some(gpu) = gpu() else { return };
    let base = layer(&gpu, BASE, 1.0, BlendMode::Normal);
    let previous = layer(&gpu, [1.0, 0.0, 0.0, 1.0], 1.0, BlendMode::Normal);
    let current = layer(&gpu, [0.0, 0.0, 1.0, 1.0], 1.0, BlendMode::Normal);
    for (progress, mode, opacity) in [
        (0.25, BlendMode::Normal, 1.0),
        (0.5, BlendMode::Multiply, 0.8),
        (0.0, BlendMode::Screen, 1.0),
    ] {
        let inputs = [
            LayerInput::new(&base.view, 1.0, BlendMode::Normal),
            LayerInput {
                transition: Some(Transition {
                    previous: &previous.view,
                    progress,
                }),
                ..LayerInput::new(&current.view, opacity, mode)
            },
        ];
        let mut compositor = Compositor::new(&gpu.device, SIZE);
        let output = compositor.render(&gpu.device, &gpu.queue, &inputs);
        let pixel = read_texture(&gpu.device, &gpu.queue, output).unwrap()[0];

        let mixed = reference::transition([1.0, 0.0, 0.0, 1.0], [0.0, 0.0, 1.0, 1.0], progress);
        let expected = reference(&[(BASE, 1.0, BlendMode::Normal), (mixed, opacity, mode)]);
        for channel in 0..4 {
            assert!(
                (pixel[channel] - expected[channel]).abs() < TOLERANCE,
                "progress {progress}: expected {expected:?}, got {pixel:?}"
            );
        }
    }
}

#[test]
fn layer_settings_follow_from_frame_to_frame() {
    let Some(gpu) = gpu() else { return };
    let white = layer(&gpu, [1.0; 4], 1.0, BlendMode::Normal);
    let mut compositor = Compositor::new(&gpu.device, SIZE);
    // Settings change, stay, then change back; a layer comes and goes.
    let frames: [&[(f32, BlendMode)]; 5] = [
        &[(1.0, BlendMode::Normal)],
        &[(0.25, BlendMode::Normal)],
        &[(0.25, BlendMode::Normal)],
        &[(0.25, BlendMode::Normal), (1.0, BlendMode::Difference)],
        &[(1.0, BlendMode::Normal)],
    ];
    for settings in frames {
        let inputs: Vec<_> = settings
            .iter()
            .map(|&(opacity, mode)| LayerInput::new(&white.view, opacity, mode))
            .collect();
        let spec: Vec<_> = settings
            .iter()
            .map(|&(opacity, mode)| ([1.0; 4], opacity, mode))
            .collect();
        let expected = reference(&spec);
        let output = compositor.render(&gpu.device, &gpu.queue, &inputs);
        let pixel = read_texture(&gpu.device, &gpu.queue, output).unwrap()[0];
        for channel in 0..4 {
            assert!(
                (pixel[channel] - expected[channel]).abs() < TOLERANCE,
                "{settings:?}: expected {expected:?}, got {pixel:?}"
            );
        }
    }
}

#[test]
fn layers_beyond_one_pass_match_the_reference() {
    let Some(gpu) = gpu() else { return };
    // Six layers: the fifth starts a second pass, with a transition.
    let colors: [Rgba; 6] = [
        BASE,
        [0.9, 0.2, 0.1, 1.0],
        [0.2, 0.7, 0.4, 0.6],
        [0.3, 0.3, 0.8, 1.0],
        [0.6, 0.5, 0.1, 0.8],
        [0.1, 0.9, 0.9, 1.0],
    ];
    let modes = [
        BlendMode::Normal,
        BlendMode::Screen,
        BlendMode::Overlay,
        BlendMode::Difference,
        BlendMode::Multiply,
        BlendMode::Add,
    ];
    let opacities = [1.0, 0.7, 0.9, 0.5, 0.8, 0.3];
    let previous_color = [1.0, 1.0, 0.0, 1.0];
    let progress = 0.4;
    let layers: Vec<_> = colors
        .iter()
        .zip(modes)
        .zip(opacities)
        .map(|((color, mode), opacity)| layer(&gpu, *color, opacity, mode))
        .collect();
    let previous = layer(&gpu, previous_color, 1.0, BlendMode::Normal);
    let inputs: Vec<_> = layers
        .iter()
        .enumerate()
        .map(|(index, layer)| LayerInput {
            transition: (index == 4).then_some(Transition {
                previous: &previous.view,
                progress,
            }),
            ..LayerInput::new(&layer.view, layer.opacity, layer.blend_mode)
        })
        .collect();
    let mut compositor = Compositor::new(&gpu.device, SIZE);
    let output = compositor.render(&gpu.device, &gpu.queue, &inputs);
    let pixel = read_texture(&gpu.device, &gpu.queue, output).unwrap()[0];

    let spec: Vec<_> = colors
        .iter()
        .enumerate()
        .map(|(index, color)| {
            let color = if index == 4 {
                reference::transition(previous_color, *color, progress)
            } else {
                *color
            };
            (color, opacities[index], modes[index])
        })
        .collect();
    let expected = reference(&spec);
    for channel in 0..4 {
        assert!(
            (pixel[channel] - expected[channel]).abs() < TOLERANCE,
            "expected {expected:?}, got {pixel:?}"
        );
    }
}
