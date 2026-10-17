//! Screens drawn from the program image: slices cut, placed and put in
//! perspective. Needs a GPU; without one the tests print a notice and pass.

#![allow(clippy::unwrap_used)]

use prism_core::{Area, Point, Quad, ScreenId, Stage};
use prism_render::{
    Gpu, ScreenTarget, StageRenderer, WORKING_FORMAT, pixels_texture, read_texture,
};

const RED: [f32; 4] = [1.0, 0.0, 0.0, 1.0];
const GREEN: [f32; 4] = [0.0, 1.0, 0.0, 1.0];
const BLACK: [f32; 4] = [0.0, 0.0, 0.0, 0.0];

fn gpu() -> Option<Gpu> {
    match Gpu::headless() {
        Ok(gpu) => Some(gpu),
        Err(error) => {
            eprintln!("skipping GPU test: {error}");
            None
        }
    }
}

/// A program of 32 × 16 pixels: red on the left half and green on the
/// right, or with `rows`, red on the top half and green below.
fn program(gpu: &Gpu, rows: bool) -> wgpu::TextureView {
    let pixels: Vec<[f32; 4]> = (0..16)
        .flat_map(|y| {
            (0..32).map(move |x| {
                let first = if rows { y < 8 } else { x < 16 };
                if first { RED } else { GREEN }
            })
        })
        .collect();
    pixels_texture(&gpu.device, &gpu.queue, 32, 16, &pixels)
        .create_view(&wgpu::TextureViewDescriptor::default())
}

use prism_render::wgpu;

/// Draws screen `id` of `stage` and reads it back.
fn draw(gpu: &Gpu, stage: &Stage, id: ScreenId, rows: bool) -> (Vec<[f32; 4]>, u32) {
    let screen = stage.screen(id).unwrap();
    let (width, height) = screen.size();
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("screen"),
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
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let program = program(gpu, rows);
    let mut renderer = StageRenderer::new(&gpu.device);
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    renderer.encode(
        &gpu.device,
        &gpu.queue,
        &mut encoder,
        &program,
        &[ScreenTarget {
            screen,
            view: &view,
        }],
    );
    gpu.queue.submit([encoder.finish()]);
    (
        read_texture(&gpu.device, &gpu.queue, &texture).unwrap(),
        width,
    )
}

fn close(a: [f32; 4], b: [f32; 4]) -> bool {
    a.iter().zip(b).all(|(a, b)| (a - b).abs() < 0.02)
}

#[test]
fn a_slice_shows_its_area_where_it_is_placed() {
    let Some(gpu) = gpu() else { return };
    let mut stage = Stage::default();
    let id = stage.add_screen("Écran", 40, 20).unwrap();
    // The green half of the program, on the left half of the screen.
    let slice = &mut stage.screen_mut(id).unwrap().slices[0];
    slice.input = Area {
        x: 0.5,
        y: 0.0,
        width: 0.5,
        height: 1.0,
    };
    slice.output = Quad::from_area(Area {
        x: 0.0,
        y: 0.0,
        width: 0.5,
        height: 1.0,
    });
    let (pixels, width) = draw(&gpu, &stage, id, false);
    let at = |x: u32, y: u32| pixels[(y * width + x) as usize];
    assert!(close(at(2, 10), GREEN), "{:?}", at(2, 10));
    assert!(close(at(17, 3), GREEN), "{:?}", at(17, 3));
    // Nothing on the right half: black.
    assert!(close(at(30, 10), [0.0, 0.0, 0.0, 1.0]) || close(at(30, 10), BLACK));
    assert_eq!(at(30, 10)[..3], [0.0, 0.0, 0.0]);

    // Disabled, the slice is not drawn.
    stage.screen_mut(id).unwrap().slices[0].enabled = false;
    let (pixels, _) = draw(&gpu, &stage, id, false);
    assert_eq!(pixels[(10 * width + 2) as usize][..3], [0.0, 0.0, 0.0]);
}

#[test]
fn a_slice_in_perspective_keeps_the_middle_of_its_image_where_the_diagonals_cross() {
    let Some(gpu) = gpu() else { return };
    let mut stage = Stage::default();
    let id = stage.add_screen("Écran", 64, 64).unwrap();
    // Top edge half as wide as the bottom one: the diagonals cross at a
    // third of the height, which is where the middle of the program lands.
    stage.screen_mut(id).unwrap().slices[0].output = Quad([
        Point::new(0.25, 0.0),
        Point::new(0.75, 0.0),
        Point::new(1.0, 1.0),
        Point::new(0.0, 1.0),
    ]);
    let (pixels, width) = draw(&gpu, &stage, id, false);
    let at = |x: u32, y: u32| pixels[(y * width + x) as usize];
    // Middle column of the program (red | green boundary) at x = 32 on
    // every row; left of it red, right of it green.
    for y in [4, 21, 40, 60] {
        assert!(close(at(30, y), RED), "row {y}: {:?}", at(30, y));
        assert!(close(at(33, y), GREEN), "row {y}: {:?}", at(33, y));
    }
    // The middle row of the program lands a third of the way down, not
    // half way: perspective, not a plain stretch.
    let (pixels, _) = draw(&gpu, &stage, id, true);
    let at = |x: u32, y: u32| pixels[(y * width + x) as usize];
    assert!(close(at(32, 19), RED), "{:?}", at(32, 19));
    assert!(close(at(32, 24), GREEN), "{:?}", at(32, 24));
    // Outside the trapezoid, at the top corners: black.
    assert_eq!(at(2, 2)[..3], [0.0, 0.0, 0.0]);
}

#[test]
fn a_warp_grid_follows_its_quad_and_moves_the_image_with_its_points() {
    let Some(gpu) = gpu() else { return };
    let mut stage = Stage::default();
    let id = stage.add_screen("Écran", 64, 64).unwrap();
    let trapezoid = Quad([
        Point::new(0.25, 0.0),
        Point::new(0.75, 0.0),
        Point::new(1.0, 1.0),
        Point::new(0.0, 1.0),
    ]);
    let slice = &mut stage.screen_mut(id).unwrap().slices[0];
    slice.output = trapezoid;
    slice.warp = Some(prism_core::Warp::from_quad(&trapezoid, 4, 4));
    // As without the warp: the middle row a third of the way down.
    let (pixels, width) = draw(&gpu, &stage, id, true);
    let at = |pixels: &[[f32; 4]], x: u32, y: u32| pixels[(y * width + x) as usize];
    assert!(close(at(&pixels, 32, 19), RED), "{:?}", at(&pixels, 32, 19));
    assert!(
        close(at(&pixels, 32, 24), GREEN),
        "{:?}",
        at(&pixels, 32, 24)
    );

    // The whole grid moved down by a quarter: so is the image.
    let warp = stage.screen_mut(id).unwrap().slices[0]
        .warp
        .as_mut()
        .unwrap();
    for point in warp.points_mut() {
        point.y += 0.25;
    }
    let (pixels, _) = draw(&gpu, &stage, id, true);
    assert!(close(at(&pixels, 32, 35), RED), "{:?}", at(&pixels, 32, 35));
    assert!(
        close(at(&pixels, 32, 40), GREEN),
        "{:?}",
        at(&pixels, 32, 40)
    );
    assert_eq!(at(&pixels, 32, 8)[..3], [0.0, 0.0, 0.0]);
}

#[test]
fn a_mask_shows_the_slice_inside_it_or_outside_when_inverted() {
    let Some(gpu) = gpu() else { return };
    let mut stage = Stage::default();
    let id = stage.add_screen("Écran", 40, 20).unwrap();
    // The left half of the screen; the program is red there.
    let mut mask = prism_core::Mask::inside(&Quad::FULL);
    mask.points = vec![
        Point::new(0.0, 0.0),
        Point::new(0.5, 0.0),
        Point::new(0.5, 1.0),
        Point::new(0.0, 1.0),
    ];
    stage.screen_mut(id).unwrap().slices[0].mask = Some(mask);
    let (pixels, width) = draw(&gpu, &stage, id, false);
    let at = |pixels: &[[f32; 4]], x: u32, y: u32| pixels[(y * width + x) as usize];
    assert!(close(at(&pixels, 5, 10), RED), "{:?}", at(&pixels, 5, 10));
    // Right half: green in the program, hidden by the mask.
    assert_eq!(at(&pixels, 30, 10)[..3], [0.0, 0.0, 0.0]);

    let slice = &mut stage.screen_mut(id).unwrap().slices[0];
    slice.mask.as_mut().unwrap().inverted = true;
    let (pixels, _) = draw(&gpu, &stage, id, false);
    assert_eq!(at(&pixels, 5, 10)[..3], [0.0, 0.0, 0.0]);
    assert!(
        close(at(&pixels, 30, 10), GREEN),
        "{:?}",
        at(&pixels, 30, 10)
    );

    // A soft edge of 10 pixels: half way at the outline, fading out.
    let slice = &mut stage.screen_mut(id).unwrap().slices[0];
    let mask = slice.mask.as_mut().unwrap();
    mask.inverted = false;
    mask.feather = 10.0;
    let (pixels, _) = draw(&gpu, &stage, id, false);
    let edge = at(&pixels, 19, 10)[0];
    assert!(edge > 0.4 && edge < 0.75, "{edge}");
    assert!(at(&pixels, 12, 10)[0] > 0.95);
}

#[test]
fn edges_fade_along_the_blend_curve_and_the_test_pattern_replaces_the_image() {
    let Some(gpu) = gpu() else { return };
    let mut stage = Stage::default();
    let id = stage.add_screen("Écran", 40, 20).unwrap();
    let blend = prism_core::EdgeBlend {
        left: 0.5,
        ..prism_core::EdgeBlend::default()
    };
    stage.screen_mut(id).unwrap().slices[0].blend = blend;
    let (pixels, width) = draw(&gpu, &stage, id, false);
    let at = |pixels: &[[f32; 4]], x: u32, y: u32| pixels[(y * width + x) as usize];
    // Pixel 5 is centered at 5.5 / 40 of the slice, 0.275 of the fade.
    let expected = blend.factor(5.5 / 40.0 / 0.5);
    let red = at(&pixels, 5, 10)[0];
    assert!((red - expected).abs() < 0.01, "{red} vs {expected}");
    // Past the fade, the image is untouched.
    assert!(close(at(&pixels, 30, 10), GREEN));

    stage.screen_mut(id).unwrap().test_pattern = true;
    let (pixels, _) = draw(&gpu, &stage, id, false);
    // No green from the program; a bright border at the edge of the slice.
    assert!(at(&pixels, 30, 9)[1] < 0.9 || at(&pixels, 30, 9)[0] > 0.5);
    let border = at(&pixels, 39, 10);
    assert!(
        border[0] > 0.9 && border[1] > 0.9 && border[2] > 0.9,
        "{border:?}"
    );
}

#[test]
fn simulated_projectors_add_their_light_where_they_overlap() {
    let Some(gpu) = gpu() else { return };
    // Two grey screens of 20 × 4 overlapping on 10 pixels.
    let grey = 0.5_f32;
    let screen = || {
        pixels_texture(
            &gpu.device,
            &gpu.queue,
            20,
            4,
            &[[grey, grey, grey, 1.0]; 80],
        )
        .create_view(&wgpu::TextureViewDescriptor::default())
    };
    let (a, b) = (screen(), screen());
    let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("simulation"),
        size: wgpu::Extent3d {
            width: 30,
            height: 4,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: WORKING_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&wgpu::TextureViewDescriptor::default());
    let mut renderer = prism_render::SimulationRenderer::new(&gpu.device);
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    let screens = [
        prism_render::SimulatedScreen {
            view: &a,
            left: 0,
            size: (20, 4),
        },
        prism_render::SimulatedScreen {
            view: &b,
            left: 10,
            size: (20, 4),
        },
    ];
    renderer.encode(
        &gpu.device,
        &gpu.queue,
        &mut encoder,
        &screens,
        2.2,
        &view,
        (30, 4),
    );
    gpu.queue.submit([encoder.finish()]);
    let pixels = read_texture(&gpu.device, &gpu.queue, &target).unwrap();
    let alone = pixels[2 * 30 + 3][0];
    let both = pixels[2 * 30 + 15][0];
    assert!((alone - grey).abs() < 0.01, "{alone}");
    // Twice the light: 0.5^2.2 × 2, back to pixel values.
    let expected = (2.0 * grey.powf(2.2)).powf(1.0 / 2.2);
    assert!((both - expected).abs() < 0.01, "{both} vs {expected}");
}
