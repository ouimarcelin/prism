//! Measures the GPU time of drawing screens of the stage from a 1080p
//! program, without window: what the advanced output costs per frame.
//!
//! ```text
//! cargo run --release -p prism-render --example stage-bench -- [--frames N]
//! ```
//!
//! Each case draws two 1080p screens, frames back to back with two in
//! flight, so the time per frame is the GPU's time.

use std::process::ExitCode;
use std::time::Instant;

use prism_core::{Mask, Point, Quad, Stage, Warp, WarpCurve};
use prism_render::{Gpu, ScreenTarget, StageRenderer, WORKING_FORMAT, pixels_texture, wgpu};

const WIDTH: u32 = 1920;
const HEIGHT: u32 = 1080;
const WARMUP: usize = 30;

/// Two screens, each with one slice of half the program, set up by `setup`.
fn stage(setup: impl Fn(&mut prism_core::Slice)) -> Stage {
    let mut stage = Stage::default();
    for x in [0.0, 0.5] {
        let Ok(id) = stage.add_screen("Écran", WIDTH, HEIGHT) else {
            continue;
        };
        if let Some(screen) = stage.screen_mut(id) {
            let slice = &mut screen.slices[0];
            slice.input.x = x;
            slice.input.width = 0.5;
            setup(slice);
        }
    }
    stage
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let frames = args
        .iter()
        .position(|arg| arg == "--frames")
        .and_then(|index| args.get(index + 1))
        .and_then(|value| value.parse().ok())
        .unwrap_or(300usize);
    let gpu = match Gpu::headless() {
        Ok(gpu) => gpu,
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCode::FAILURE;
        }
    };
    let pixels: Vec<[f32; 4]> = (0..WIDTH * HEIGHT)
        .map(|index| {
            let (x, y) = (index % WIDTH, index / WIDTH);
            [x as f32 / WIDTH as f32, y as f32 / HEIGHT as f32, 0.5, 1.0]
        })
        .collect();
    let program = pixels_texture(&gpu.device, &gpu.queue, WIDTH, HEIGHT, &pixels)
        .create_view(&wgpu::TextureViewDescriptor::default());
    let screen = || {
        gpu.device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("screen"),
                size: wgpu::Extent3d {
                    width: WIDTH,
                    height: HEIGHT,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: WORKING_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            })
            .create_view(&wgpu::TextureViewDescriptor::default())
    };
    let views = [screen(), screen()];
    let trapezoid = Quad([
        Point::new(0.1, 0.0),
        Point::new(0.9, 0.05),
        Point::new(1.0, 1.0),
        Point::new(0.0, 0.95),
    ]);
    let cases: Vec<(&str, Stage)> = vec![
        ("Découpes droites", stage(|_| {})),
        ("Perspective", stage(|slice| slice.output = trapezoid)),
        (
            "Grille lissée 8 × 8",
            stage(|slice| {
                let mut warp = Warp::from_quad(&trapezoid, 8, 8);
                warp.curve = WarpCurve::Smooth;
                slice.warp = Some(warp);
            }),
        ),
        (
            "Grille lissée 32 × 32",
            stage(|slice| {
                let mut warp = Warp::from_quad(&trapezoid, 32, 32);
                warp.curve = WarpCurve::Smooth;
                slice.warp = Some(warp);
            }),
        ),
        (
            "Masque de 64 points, bord de 40 px",
            stage(|slice| {
                let mut mask = Mask::inside(&Quad::FULL);
                mask.points = (0..64)
                    .map(|index| {
                        let angle = index as f32 / 64.0 * std::f32::consts::TAU;
                        Point::new(0.5 + 0.45 * angle.cos(), 0.5 + 0.45 * angle.sin())
                    })
                    .collect();
                mask.feather = 40.0;
                slice.mask = Some(mask);
            }),
        ),
        (
            "Fondu des bords 20 %",
            stage(|slice| {
                slice.blend.left = 0.2;
                slice.blend.right = 0.2;
            }),
        ),
    ];

    println!("| Deux écrans 1080p | ms par image (GPU) |");
    println!("| --- | --- |");
    for (label, stage) in cases {
        let mut renderer = StageRenderer::new(&gpu.device);
        let targets: Vec<ScreenTarget<'_>> = stage
            .screens()
            .iter()
            .zip(&views)
            .map(|(screen, view)| ScreenTarget { screen, view })
            .collect();
        let mut in_flight = std::collections::VecDeque::new();
        let mut started = Instant::now();
        for frame in 0..WARMUP + frames {
            if frame == WARMUP {
                let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
                in_flight.clear();
                started = Instant::now();
            }
            let mut encoder = gpu.device.create_command_encoder(&Default::default());
            renderer.encode(
                &gpu.device,
                &gpu.queue,
                &mut encoder,
                &program,
                (WIDTH, HEIGHT),
                &targets,
            );
            in_flight.push_back(gpu.queue.submit([encoder.finish()]));
            if in_flight.len() > 2
                && let Some(index) = in_flight.pop_front()
            {
                let _ = gpu.device.poll(wgpu::PollType::Wait {
                    submission_index: Some(index),
                    timeout: None,
                });
            }
        }
        let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
        let ms = started.elapsed().as_secs_f64() * 1000.0 / frames as f64;
        println!("| {label} | {ms:.2} |");
    }
    ExitCode::SUCCESS
}
