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
