//! Plays media files on layers of a 1080p60 composition, without window,
//! and checks that the engine keeps up: the benchmark and endurance test of
//! phase 1.
//!
//! ```text
//! cargo run --release -p prism-engine --example engine-run -- \
//!     [--seconds N] [--every N] [--report FILE.md] FILE [FILE ...]
//! ```
//!
//! One layer per file, all playing at once. Prints the statistics every
//! `--every` seconds, then a summary: frame rate, late frames, stutters,
//! memory, and the verdict against 60 frames per second without stutter.

use std::fmt::Write as _;
use std::process::ExitCode;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use prism_core::{Clip, Composition, Slot};
use prism_engine::{ClipState, Engine, EngineStatus};
use prism_render::Gpu;

/// Resident memory of this process, in megabytes, as `ps` reports it.
fn memory_mb() -> Option<f64> {
    let output = std::process::Command::new("ps")
        .args(["-o", "rss=", "-p", &std::process::id().to_string()])
        .output()
        .ok()?;
    let kilobytes: f64 = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .ok()?;
    Some(kilobytes / 1024.0)
}

struct Options {
    seconds: u64,
    every: u64,
    report: Option<String>,
    files: Vec<String>,
}

fn parse() -> Option<Options> {
    let mut options = Options {
        seconds: 10,
        every: 0,
        report: None,
        files: Vec::new(),
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--seconds" => options.seconds = args.next()?.parse().ok()?,
            "--every" => options.every = args.next()?.parse().ok()?,
            "--report" => options.report = Some(args.next()?),
            _ => options.files.push(arg),
        }
    }
    if options.every == 0 {
        options.every = if options.seconds > 120 { 30 } else { 1 };
    }
    (!options.files.is_empty()).then_some(options)
}

fn layer_line(status: &EngineStatus) -> String {
    status
        .layers
        .iter()
        .filter(|layer| layer.slot.is_some())
        .map(|layer| match &layer.state {
            ClipState::Error(message) => format!("erreur : {message}"),
            state => format!("{state:?}, {} en retard", layer.late_frames),
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

fn main() -> ExitCode {
    let Some(options) = parse() else {
        eprintln!("usage: engine-run [--seconds N] [--every N] [--report FILE.md] FILE [FILE ...]");
        return ExitCode::FAILURE;
    };
    let gpu = match Gpu::headless() {
        Ok(gpu) => Arc::new(gpu),
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCode::FAILURE;
        }
    };
    let gpu_name = gpu.adapter.get_info().name;
    println!("gpu       {gpu_name}");

    let mut composition =
        Composition::default().with_grid(options.files.len().saturating_sub(4), 0);
    let column = composition.columns()[0].id();
    let layers: Vec<_> = composition
        .layers()
        .iter()
        .map(|layer| layer.id())
        .collect();
    for (layer, file) in layers.iter().zip(&options.files) {
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
    let mut memory_after_start = None;
    let mut elapsed = 0;
    while elapsed < options.seconds {
        let step = options.every.min(options.seconds - elapsed);
        elapsed += step;
        thread::sleep(
            (start + Duration::from_secs(elapsed)).saturating_duration_since(Instant::now()),
        );
        let status = engine.status();
        if elapsed > 2 {
            worst_fps = worst_fps.min(status.fps);
        }
        if memory_after_start.is_none() && elapsed >= 5 {
            memory_after_start = memory_mb();
        }
        println!(
            "{:>5} s  {:5.1} i/s  {:5.2} ms/image (pire {:5.1})  pire écart {:5.1} ms  saccades {}  | {}",
            elapsed,
            status.fps,
            status.frame_time_ms,
            status.worst_frame_time_ms,
            status.worst_interval_ms,
            status.hitches,
            layer_line(&status)
        );
    }

    let status = engine.status();
    let memory_end = memory_mb();
    let late_frames: u64 = status.layers.iter().map(|layer| layer.late_frames).sum();
    let errors = status
        .layers
        .iter()
        .any(|layer| matches!(layer.state, ClipState::Error(_)));
    let worst_fps = if worst_fps.is_finite() {
        worst_fps
    } else {
        status.fps
    };
    let smooth = worst_fps >= 59.0 && status.hitches == 0 && late_frames == 0 && !errors;
    let memory = match (memory_after_start, memory_end) {
        (Some(first), Some(last)) => format!("{first:.0} Mo après 5 s, {last:.0} Mo à la fin"),
        _ => "inconnue".to_owned(),
    };

    let mut summary = String::new();
    let _ = writeln!(summary, "- GPU : {gpu_name}");
    let _ = writeln!(
        summary,
        "- Couches : {}, durée : {} s, images rendues : {}",
        options.files.len(),
        options.seconds,
        status.frames
    );
    let _ = writeln!(
        summary,
        "- Images par seconde, pire seconde : {worst_fps:.1}"
    );
    let _ = writeln!(
        summary,
        "- Travail du thread de rendu : {:.2} ms par image en moyenne, {:.1} ms au pire",
        status.frame_time_ms, status.worst_frame_time_ms
    );
    let _ = writeln!(
        summary,
        "- Pire écart entre deux images : {:.1} ms ; saccades (écart > 25 ms) : {}",
        status.worst_interval_ms, status.hitches
    );
    let _ = writeln!(
        summary,
        "- Images en retard (image précédente gardée) : {late_frames} ; départs en retard du thread : {}",
        status.late_ticks
    );
    let _ = writeln!(summary, "- Mémoire : {memory}");
    let _ = writeln!(
        summary,
        "- Verdict : {}",
        if smooth { "FLUIDE" } else { "PAS FLUIDE" }
    );
    println!();
    print!("{summary}");

    if let Some(path) = options.report {
        let mut report = String::from("# Mesure du moteur\n\n");
        let _ = writeln!(report, "Fichiers :\n");
        for file in &options.files {
            let _ = writeln!(report, "- `{file}`");
        }
        let _ = writeln!(report, "\nRésultats :\n\n{summary}");
        if let Err(error) = std::fs::write(&path, report) {
            eprintln!("error: cannot write {path}: {error}");
            return ExitCode::FAILURE;
        }
        println!("rapport   {path}");
    }
    if smooth {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
