//! Plays HAP files on layers of a 1080p60 composition, without window, and
//! prints the engine statistics every second.
//!
//! ```text
//! cargo run --release -p prism-engine --example engine-run -- [--seconds N] FILE.mov [FILE.mov ...]
//! ```
//!
//! One layer per file, all layers playing at once. Ends with a summary:
//! frame rate, late frames and the verdict against 60 frames per second.

use std::process::ExitCode;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use prism_core::{Clip, Composition, Slot};
use prism_engine::{ClipState, Engine};
use prism_render::Gpu;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1).peekable();
    let mut seconds = 10u64;
    let mut files = Vec::new();
    while let Some(arg) = args.next() {
        if arg == "--seconds" {
            seconds = args
                .next()
                .and_then(|value| value.parse().ok())
                .unwrap_or(seconds);
        } else {
            files.push(arg);
        }
    }
    if files.is_empty() {
        eprintln!("usage: engine-run [--seconds N] FILE.mov [FILE.mov ...]");
        return ExitCode::FAILURE;
    }

    let gpu = match Gpu::headless() {
        Ok(gpu) => Arc::new(gpu),
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCode::FAILURE;
        }
    };
    println!("gpu       {}", gpu.adapter.get_info().name);

    let mut composition = Composition::default().with_grid(files.len().saturating_sub(4), 0);
    let column = composition.columns()[0].id();
    let layers: Vec<_> = composition
        .layers()
        .iter()
        .map(|layer| layer.id())
        .collect();
    for (layer, file) in layers.iter().zip(&files) {
        if let Err(error) =
            composition.set_clip(Slot::new(*layer, column), Some(Clip::from_file(file)))
        {
            eprintln!("error: {error}");
            return ExitCode::FAILURE;
        }
    }
    if let Err(error) = composition.trigger_column(column) {
        eprintln!("error: {error}");
        return ExitCode::FAILURE;
    }

    let engine = match Engine::start(gpu, composition) {
        Ok(engine) => engine,
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCode::FAILURE;
        }
    };
    let start = Instant::now();
    let mut worst_fps = f64::INFINITY;
    for second in 1..=seconds {
        thread::sleep(
            (start + Duration::from_secs(second)).saturating_duration_since(Instant::now()),
        );
        let status = engine.status();
        if second > 1 {
            worst_fps = worst_fps.min(status.fps);
        }
        let layers: Vec<String> = status
            .layers
            .iter()
            .filter(|layer| layer.slot.is_some())
            .map(|layer| match &layer.state {
                ClipState::Error(message) => format!("error: {message}"),
                state => format!(
                    "{state:?} frame {}/{} late {}",
                    layer.frame.map_or(0, |frame| frame + 1),
                    layer.frame_count,
                    layer.late_frames
                ),
            })
            .collect();
        println!(
            "{second:>3} s  {:5.1} fps  {:5.2} ms/frame  late ticks {}  | {}",
            status.fps,
            status.frame_time_ms,
            status.late_ticks,
            layers.join(" | ")
        );
    }

    let status = engine.status();
    let late_frames: u64 = status.layers.iter().map(|layer| layer.late_frames).sum();
    let errors = status
        .layers
        .iter()
        .any(|layer| matches!(layer.state, ClipState::Error(_)));
    let smooth = worst_fps >= 59.0 && status.late_ticks == 0 && late_frames == 0 && !errors;
    println!();
    println!(
        "summary   {} layers, {} frames, worst fps {:.1}, late ticks {}, late frames {}",
        files.len(),
        status.frames,
        if worst_fps.is_finite() {
            worst_fps
        } else {
            status.fps
        },
        status.late_ticks,
        late_frames
    );
    println!("verdict   {}", if smooth { "SMOOTH" } else { "NOT SMOOTH" });
    if smooth {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
