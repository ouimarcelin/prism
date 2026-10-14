//! Measures the GPU time of one engine frame, without window: upload of
//! 1080p HAP frames, conversion into layer textures, mixing into the
//! program image.
//!
//! ```text
//! cargo run --release -p prism-render --example gpu-bench -- [--layers N] [--frames N]
//! ```
//!
//! Layers alternate HAP and HAP Q frames of random blocks, the heaviest
//! content. Frames go to the GPU back to back, two at a time: once the GPU
//! is the bottleneck, the time per frame is its time.

use std::process::ExitCode;
use std::time::{Duration, Instant};

use prism_core::{BlendMode, Resolution};
use prism_media::{BlockFormat, ColorLayout, Frame, HapFrame, Plane};
use prism_render::{
    Compositor, FrameUploader, Gpu, LayerInput, VideoTexture, WORKING_FORMAT, wgpu,
};

const WIDTH: u32 = 1920;
const HEIGHT: u32 = 1080;

/// Frames run before measuring.
const WARMUP: usize = 60;

/// Random bytes: every block of them is a valid BC block.
fn noise(len: usize, seed: u64) -> Vec<u8> {
    let mut state = seed | 1;
    (0..len)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state as u8
        })
        .collect()
}

/// HAP frame (BC1) for even layers, HAP Q (scaled YCoCg in BC3) for odd ones.
fn hap_frame(layer: usize) -> Frame {
    let (format, layout) = if layer.is_multiple_of(2) {
        (BlockFormat::Bc1, ColorLayout::Rgb)
    } else {
        (BlockFormat::Bc3, ColorLayout::ScaledYCoCg)
    };
    let data = noise(format.image_bytes(WIDTH, HEIGHT), layer as u64 + 1);
    Frame::Hap(HapFrame {
        layout,
        color: Plane { format, data },
        alpha: None,
    })
}

fn program_texture(device: &wgpu::Device) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("program"),
        size: wgpu::Extent3d {
            width: WIDTH,
            height: HEIGHT,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: WORKING_FORMAT,
        usage: wgpu::TextureUsages::COPY_DST
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    })
}

fn main() -> ExitCode {
    let mut layers = 4;
    let mut frames = 600;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let value = args.next().and_then(|value| value.parse().ok());
        match (arg.as_str(), value) {
            ("--layers", Some(value)) => layers = value,
            ("--frames", Some(value)) => frames = value,
            _ => {
                eprintln!("usage: gpu-bench [--layers N] [--frames N]");
                return ExitCode::FAILURE;
            }
        }
    }
    let gpu = match Gpu::headless() {
        Ok(gpu) => gpu,
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCode::FAILURE;
        }
    };
    let (device, queue) = (&gpu.device, &gpu.queue);
    let resolution = Resolution {
        width: WIDTH,
        height: HEIGHT,
    };
    let mut uploader = FrameUploader::new(device, queue);
    let mut compositor = Compositor::new(device, resolution);
    let program = program_texture(device);
    let hap: Vec<Frame> = (0..layers).map(hap_frame).collect();
    let mut textures: Vec<VideoTexture> = (0..layers)
        .map(|_| VideoTexture::new(device, WIDTH, HEIGHT))
        .collect();

    let mut previous = None;
    let mut cpu = Duration::ZERO;
    let mut start = Instant::now();
    for frame in 0..WARMUP + frames {
        if frame == WARMUP {
            start = Instant::now();
            cpu = Duration::ZERO;
        }
        let encoding = Instant::now();
        let mut encoder = device.create_command_encoder(&Default::default());
        for (texture, hap) in textures.iter_mut().zip(&hap) {
            if let Err(error) = uploader.upload(device, queue, &mut encoder, texture, hap) {
                eprintln!("error: {error}");
                return ExitCode::FAILURE;
            }
        }
        uploader.finish(&encoder);
        let inputs: Vec<_> = textures
            .iter()
            .map(|texture| LayerInput::new(texture.view(), 0.8, BlendMode::Screen))
            .collect();
        compositor.encode(device, queue, &mut encoder, &inputs);
        encoder.copy_texture_to_texture(
            compositor.output().as_image_copy(),
            program.as_image_copy(),
            program.size(),
        );
        let index = queue.submit([encoder.finish()]);
        cpu += encoding.elapsed();
        // Two frames in flight: wait for the one before.
        if let Some(previous) = previous.replace(index) {
            let _ = device.poll(wgpu::PollType::Wait {
                submission_index: Some(previous),
                timeout: None,
            });
        }
    }
    let _ = device.poll(wgpu::PollType::wait_indefinitely());
    let per_frame = start.elapsed().as_secs_f64() * 1000.0 / frames as f64;
    let cpu_per_frame = cpu.as_secs_f64() * 1000.0 / frames as f64;
    println!("gpu       {}", gpu.adapter.get_info().name);
    println!(
        "{layers} couches HAP 1080p, {frames} images : {per_frame:.2} ms par image ({:.0} images/s au plus), dont {cpu_per_frame:.2} ms de préparation sur le processeur",
        1000.0 / per_frame
    );
    ExitCode::SUCCESS
}
