//! Checks a HAP file: prints its properties, measures decoding and GPU upload
//! speed, and saves one frame as a PNG image.
//!
//! ```text
//! cargo run --release -p prism-render --example hap-check -- FILE.mov [FRAME] [OUTPUT.png]
//! ```

use std::fs::File;
use std::io::BufWriter;
use std::path::Path;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use prism_media::{Frame, HapDecoder};
use prism_render::{FrameUploader, Gpu, VideoTexture, read_texture};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(path) = args.first() else {
        eprintln!("usage: hap-check FILE.mov [FRAME] [OUTPUT.png]");
        return ExitCode::FAILURE;
    };
    match run(path, args.get(1), args.get(2)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            let mut source = error.source();
            while let Some(cause) = source {
                eprintln!("  caused by: {cause}");
                source = cause.source();
            }
            ExitCode::FAILURE
        }
    }
}

fn per_frame(total: Duration, frames: usize) -> String {
    let ms = total.as_secs_f64() * 1000.0 / frames.max(1) as f64;
    format!("{ms:.2} ms/frame ({:.0} frames/s)", 1000.0 / ms)
}

fn run(
    path: &str,
    frame_arg: Option<&String>,
    output: Option<&String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut decoder = HapDecoder::open(path)?;
    let track = decoder.track().clone();
    let frames = track.frame_count();
    println!("file      {path}");
    println!("codec     {}", track.codec_name());
    println!("size      {} x {}", track.width, track.height);
    println!(
        "frames    {frames} at {:.3} fps, {:.2} s",
        track.frame_rate(),
        track.duration_seconds()
    );

    // CPU: read and decompress every frame.
    let mut frame = Frame::default();
    let start = Instant::now();
    for index in 0..frames {
        decoder.decode_frame(index, frame.hap_mut())?;
    }
    let decode_time = start.elapsed();
    let hap = frame.hap_mut();
    println!(
        "layout    {:?}, {:?}{}",
        hap.layout,
        hap.color.format,
        if hap.alpha.is_some() {
            " + BC4 alpha"
        } else {
            ""
        }
    );
    println!("decode    {}", per_frame(decode_time, frames));

    // GPU: upload and convert every frame.
    let gpu = Gpu::headless()?;
    println!(
        "gpu       {} ({:?})",
        gpu.adapter.get_info().name,
        gpu.adapter.get_info().backend
    );
    let uploader = FrameUploader::new(&gpu.device, &gpu.queue);
    let mut video = VideoTexture::new(&gpu.device, track.width, track.height);
    let mut upload_time = Duration::ZERO;
    for index in 0..frames {
        decoder.decode_frame(index, frame.hap_mut())?;
        let start = Instant::now();
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        uploader.upload(&gpu.device, &gpu.queue, &mut encoder, &mut video, &frame)?;
        gpu.queue.submit([encoder.finish()]);
        gpu.device
            .poll(prism_render::wgpu::PollType::wait_indefinitely())?;
        upload_time += start.elapsed();
    }
    println!(
        "upload    {} (upload, conversion, wait for GPU)",
        per_frame(upload_time, frames)
    );
    let budget = 1000.0 / 60.0 / 4.0;
    let total = (decode_time + upload_time).as_secs_f64() * 1000.0 / frames.max(1) as f64;
    println!(
        "total     {total:.2} ms/frame; budget for 4 layers at 60 fps: {budget:.2} ms/frame per layer -> {}",
        if total <= budget { "OK" } else { "TOO SLOW" }
    );

    // Snapshot.
    let index = frame_arg
        .and_then(|a| a.parse().ok())
        .unwrap_or(0)
        .min(frames.saturating_sub(1));
    let output = output.map_or_else(
        || {
            format!(
                "{}-frame{index}.png",
                Path::new(path)
                    .file_stem()
                    .map_or("hap".into(), |s| s.to_string_lossy())
            )
        },
        Clone::clone,
    );
    decoder.decode_frame(index, frame.hap_mut())?;
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    uploader.upload(&gpu.device, &gpu.queue, &mut encoder, &mut video, &frame)?;
    gpu.queue.submit([encoder.finish()]);
    let pixels = read_texture(&gpu.device, &gpu.queue, video.texture())?;
    let bytes: Vec<u8> = pixels
        .iter()
        .flat_map(|&[r, g, b, a]| {
            // Back to straight alpha for the PNG file.
            let straight = |c: f32| if a > 0.0 { c / a } else { 0.0 };
            [straight(r), straight(g), straight(b), a]
                .map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8)
        })
        .collect();
    let mut png = png::Encoder::new(
        BufWriter::new(File::create(&output)?),
        track.width,
        track.height,
    );
    png.set_color(png::ColorType::Rgba);
    png.set_depth(png::BitDepth::Eight);
    png.write_header()?.write_image_data(&bytes)?;
    println!("snapshot  frame {index} written to {output}");
    Ok(())
}
