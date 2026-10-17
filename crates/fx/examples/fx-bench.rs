//! Measures the GPU time of every effect and generator on a 1080p image,
//! without window: to know what a chain costs at 60 frames per second.
//!
//! ```text
//! cargo run --release -p prism-fx --example fx-bench -- [--frames N]
//! ```
//!
//! Each effect runs alone, at its default settings, on the same image,
//! frames back to back with two in flight: once the GPU is the bottleneck,
//! the time per frame is its time. The first line is the copy alone (an
//! effect that changes nothing measurable), for reference.

use std::process::ExitCode;
use std::time::Instant;

use prism_core::Effect;
use prism_fx::{CATALOG, EffectChain, EffectRenderer, FrameTime, GENERATORS};
use prism_render::{Gpu, WORKING_FORMAT, pixels_texture, wgpu};

const WIDTH: u32 = 1920;
const HEIGHT: u32 = 1080;
const WARMUP: usize = 30;

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
    let renderer = EffectRenderer::new(&gpu.device);
    let pixels: Vec<[f32; 4]> = (0..WIDTH * HEIGHT)
        .map(|index| {
            let (x, y) = (index % WIDTH, index / WIDTH);
            [x as f32 / WIDTH as f32, y as f32 / HEIGHT as f32, 0.5, 1.0]
        })
        .collect();
    let input = pixels_texture(&gpu.device, &gpu.queue, WIDTH, HEIGHT, &pixels)
        .create_view(&wgpu::TextureViewDescriptor::default());
    let destination = gpu
        .device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("bench"),
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
        .create_view(&wgpu::TextureViewDescriptor::default());

    println!("| Effet | Passes | ms par image (GPU) |");
    println!("| --- | --- | --- |");
    let mut kinds: Vec<(&str, Effect, usize)> = vec![(
        "Copie (inversion à 0)",
        {
            let mut copy = Effect::new("invert");
            copy.set_param("amount", 0.0);
            copy
        },
        1,
    )];
    for info in CATALOG.iter().chain(GENERATORS) {
        let mut effect = info.create();
        if info.kind == prism_fx::TEXT_KIND {
            effect.text = "Prism 1080p".into();
        }
        kinds.push((info.label, effect, info.passes()));
    }
    for (label, effect, passes) in kinds {
        let mut chain = EffectChain::default();
        let effects = [&effect];
        let run = |chain: &mut EffectChain, frame: usize| {
            let mut encoder = gpu.device.create_command_encoder(&Default::default());
            let time = FrameTime {
                seconds: frame as f64 / 60.0,
                beat: frame as f64 / 30.0,
                ..FrameTime::default()
            };
            chain.apply_into(
                &renderer,
                &gpu.device,
                &gpu.queue,
                &mut encoder,
                &effects,
                &input,
                (WIDTH, HEIGHT),
                time,
                &destination,
            );
            gpu.queue.submit([encoder.finish()])
        };
        let mut in_flight = std::collections::VecDeque::new();
        let mut started = Instant::now();
        for frame in 0..WARMUP + frames {
            if frame == WARMUP {
                let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
                in_flight.clear();
                started = Instant::now();
            }
            in_flight.push_back(run(&mut chain, frame));
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
        println!("| {label} | {passes} | {ms:.2} |");
    }
    ExitCode::SUCCESS
}
