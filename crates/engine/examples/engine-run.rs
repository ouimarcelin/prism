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
//! `--every` seconds and every incident as it happens (stutter with the
//! steps of the frame before it, late frames) with the paging activity of
//! the machine, then a summary: frame rate, late frames, stutters, memory,
//! and the verdict against 60 frames per second without stutter.

use std::fmt::Write as _;
use std::process::ExitCode;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use prism_core::{Clip, Composition, Slot};
use prism_engine::{ClipState, Engine, EngineStatus, FrameSteps, Hitch};
use prism_render::Gpu;

/// How often the status is read, to catch every stutter.
const POLL: Duration = Duration::from_millis(100);

/// Output of a command, or `None` when it cannot run.
fn command_output(program: &str, args: &[&str]) -> Option<String> {
    let output = std::process::Command::new(program)
        .args(args)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Memory of this process, in megabytes: its footprint as the `footprint`
/// tool of macOS reports it (memory of the GPU and compressed memory
/// included, as the Activity Monitor shows), else its resident memory as
/// `ps` reports it.
fn memory_mb() -> Option<f64> {
    let pid = std::process::id().to_string();
    footprint_mb(&pid).or_else(|| {
        let kilobytes: f64 = command_output("ps", &["-o", "rss=", "-p", &pid])?
            .trim()
            .parse()
            .ok()?;
        Some(kilobytes / 1024.0)
    })
}

fn footprint_mb(pid: &str) -> Option<f64> {
    // "engine-run [123]: 64-bit    Footprint: 259 MB (16384 bytes per page)"
    let report = command_output("footprint", &["-p", pid])?;
    let mut words = report.split("Footprint:").nth(1)?.split_whitespace();
    let value: f64 = words.next()?.parse().ok()?;
    let megabytes = match words.next()? {
        "B" => value / 1024.0 / 1024.0,
        "KB" => value / 1024.0,
        "MB" => value,
        "GB" => value * 1024.0,
        _ => return None,
    };
    Some(megabytes)
}

/// GPU buffers alive, when wgpu counts its objects.
fn gpu_buffers(gpu: &Gpu) -> isize {
    gpu.device.get_internal_counters().hal.buffers.read()
}

/// Swap space in use on the machine, in megabytes (macOS).
fn swap_used_mb() -> Option<f64> {
    // "total = 3072.00M  used = 2154.38M  free = 917.62M  (encrypted)"
    let usage = command_output("sysctl", &["-n", "vm.swapusage"])?;
    let used = usage.split("used =").nth(1)?.split_whitespace().next()?;
    used.trim_end_matches('M').parse().ok()
}

/// Paging of the machine since it started, in pages (macOS): reads of
/// files, decompressions of compressed memory, swap reads and writes.
#[derive(Clone, Copy, Debug, Default)]
struct PagingCounters {
    pageins: u64,
    decompressions: u64,
    swapins: u64,
    swapouts: u64,
}

impl PagingCounters {
    fn read() -> Option<Self> {
        let text = command_output("vm_stat", &[])?;
        let counter = |name: &str| -> Option<u64> {
            let line = text.lines().find(|line| line.starts_with(name))?;
            line.split(':')
                .nth(1)?
                .trim()
                .trim_end_matches('.')
                .parse()
                .ok()
        };
        Some(Self {
            pageins: counter("Pageins")?,
            decompressions: counter("Decompressions")?,
            swapins: counter("Swapins")?,
            swapouts: counter("Swapouts")?,
        })
    }

    fn since(self, earlier: Self) -> Self {
        Self {
            pageins: self.pageins.saturating_sub(earlier.pageins),
            decompressions: self.decompressions.saturating_sub(earlier.decompressions),
            swapins: self.swapins.saturating_sub(earlier.swapins),
            swapouts: self.swapouts.saturating_sub(earlier.swapouts),
        }
    }
}

impl std::fmt::Display for PagingCounters {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} pages lues sur disque, {} décompressées, swap {} lues et {} écrites",
            self.pageins, self.decompressions, self.swapins, self.swapouts
        )
    }
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

fn steps_line(steps: &FrameSteps) -> String {
    format!(
        "lecture {:.1}, envoi au GPU {:.1}, mixage {:.1}, sorties {:.1} ms",
        steps.playback_ms, steps.upload_ms, steps.mix_ms, steps.present_ms
    )
}

/// What went wrong between two reads of the status.
enum Trouble {
    /// Stutters, the last one described.
    Hitches { count: u64, last: Option<Hitch> },
    /// Frames shown late, on all layers together.
    LateFrames { count: u64 },
}

/// A problem seen during the run.
struct Incident {
    /// Seconds since the start of the run.
    at: f64,
    trouble: Trouble,
    /// Paging of the machine over the second before.
    paging: Option<PagingCounters>,
}

impl Incident {
    fn line(&self) -> String {
        let mut line = format!("{:7.1} s  ", self.at);
        match &self.trouble {
            Trouble::Hitches { count, last } => {
                if *count > 1 {
                    let _ = write!(line, "{count} saccades, la dernière : ");
                }
                match last {
                    Some(hitch) => {
                        let _ = write!(
                            line,
                            "saccade de {:.1} ms entre deux images ; image d'avant : {}",
                            hitch.interval_ms,
                            steps_line(&hitch.previous)
                        );
                    }
                    None => line.push_str("saccade"),
                }
            }
            Trouble::LateFrames { count } => {
                let _ = write!(line, "{count} images en retard (décodage pas prêt)");
            }
        }
        if let Some(paging) = self.paging {
            let _ = write!(line, " ; mémoire de la machine (1 s) : {paging}");
        }
        line
    }
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

    let swap_at_start = swap_used_mb();
    let counters_at_start = PagingCounters::read();
    let engine = match Engine::start(Arc::clone(&gpu), composition) {
        Ok(engine) => engine,
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCode::FAILURE;
        }
    };
    let start = Instant::now();
    let duration = Duration::from_secs(options.seconds);
    let mut worst_fps = f64::INFINITY;
    // Upload buffers and caches settle during the first seconds: memory is
    // compared from this point on.
    let memory_reference = (options.seconds / 2).clamp(1, 60);
    let mut memory_after_start = None;
    let mut buffers_after_start = 0;
    let mut next_line = options.every;
    let mut next_second = 1;
    let mut counters = counters_at_start;
    let mut last_second_paging = None;
    let mut seen_hitches = 0;
    let mut seen_late = 0;
    let mut incidents = Vec::new();
    let mut polls = 1;
    loop {
        let wake = start + POLL * polls;
        polls += 1;
        thread::sleep(wake.saturating_duration_since(Instant::now()));
        let elapsed = start.elapsed();
        let status = engine.status();

        let seconds = elapsed.as_secs();
        if seconds >= next_second {
            next_second = seconds + 1;
            if seconds > 2 {
                worst_fps = worst_fps.min(status.fps);
            }
            let now = PagingCounters::read();
            if let (Some(now), Some(before)) = (now, counters) {
                last_second_paging = Some(now.since(before));
            }
            counters = now;
            if memory_after_start.is_none() && seconds >= memory_reference {
                memory_after_start = memory_mb();
                buffers_after_start = gpu_buffers(&gpu);
            }
        }
        let late: u64 = status.layers.iter().map(|layer| layer.late_frames).sum();
        let mut troubles = Vec::new();
        if status.hitches > seen_hitches {
            troubles.push(Trouble::Hitches {
                count: status.hitches - seen_hitches,
                last: status.last_hitch,
            });
            seen_hitches = status.hitches;
        }
        if late > seen_late {
            troubles.push(Trouble::LateFrames {
                count: late - seen_late,
            });
            seen_late = late;
        }
        for trouble in troubles {
            let incident = Incident {
                at: elapsed.as_secs_f64(),
                trouble,
                paging: last_second_paging,
            };
            println!("incident {}", incident.line());
            incidents.push(incident);
        }
        if seconds >= next_line || elapsed >= duration {
            next_line = seconds + options.every;
            println!(
                "{:>5} s  {:5.1} i/s  {:5.2} ms/image (pire {:5.1})  pire écart {:5.1} ms  saccades {}  {:>4.0} Mo, {} tampons GPU  | {}",
                seconds,
                status.fps,
                status.frame_time_ms,
                status.worst_frame_time_ms,
                status.worst_interval_ms,
                status.hitches,
                memory_mb().unwrap_or(f64::NAN),
                gpu_buffers(&gpu),
                layer_line(&status)
            );
        }
        if elapsed >= duration {
            break;
        }
    }

    let status = engine.status();
    let memory_end = memory_mb();
    let swap_at_end = swap_used_mb();
    let paging_during = match (PagingCounters::read(), counters_at_start) {
        (Some(end), Some(begin)) => Some(end.since(begin)),
        _ => None,
    };
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
        (Some(first), Some(last)) => format!(
            "{first:.0} Mo après {memory_reference} s, {last:.0} Mo à la fin ; tampons GPU : {buffers_after_start} après {memory_reference} s, {} à la fin",
            gpu_buffers(&gpu)
        ),
        _ => "inconnue".to_owned(),
    };

    let mut summary = String::new();
    let _ = writeln!(summary, "- GPU : {gpu_name}");
    let _ = writeln!(
        summary,
        "- Thread de rendu : {}",
        if status.real_time {
            "temps réel"
        } else {
            "priorité normale"
        }
    );
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
        "- Pire durée de chaque étape : {}",
        steps_line(&status.worst_steps)
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
    if let (Some(begin), Some(end)) = (swap_at_start, swap_at_end) {
        let _ = write!(
            summary,
            "- Swap de la machine : {begin:.0} Mo au début, {end:.0} Mo à la fin"
        );
        match paging_during {
            Some(during) => {
                let _ = writeln!(summary, " ; pendant la mesure : {during}");
            }
            None => summary.push('\n'),
        }
    }
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
        if !incidents.is_empty() {
            let _ = writeln!(report, "Incidents :\n");
            for incident in &incidents {
                let _ = writeln!(report, "- {}", incident.line().trim_start());
            }
        }
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
