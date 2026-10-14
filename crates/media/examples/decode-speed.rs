//! Measures how fast media files decode, alone and all at once.
//!
//! ```text
//! cargo run --release -p prism-media --example decode-speed -- FILE [FILE ...]
//! ```

use std::process::ExitCode;
use std::time::Instant;

use prism_media::{Frame, open_file};

fn decode_all(path: &str) -> Result<(usize, f64), String> {
    let mut source = open_file(path.as_ref()).map_err(|error| error.to_string())?;
    let frames = source.info().frame_count();
    let mut frame = Frame::default();
    let start = Instant::now();
    for index in 0..frames {
        source
            .decode(index, &mut frame)
            .map_err(|error| error.to_string())?;
    }
    Ok((frames, start.elapsed().as_secs_f64()))
}

fn main() -> ExitCode {
    let files: Vec<String> = std::env::args().skip(1).collect();
    if files.is_empty() {
        eprintln!("usage: decode-speed FILE [FILE ...]");
        return ExitCode::FAILURE;
    }
    for file in &files {
        match decode_all(file) {
            Ok((frames, seconds)) => {
                println!("alone     {file}: {:.0} frames/s", frames as f64 / seconds)
            }
            Err(error) => {
                eprintln!("{file}: {error}");
                return ExitCode::FAILURE;
            }
        }
    }
    let start = Instant::now();
    let threads: Vec<_> = files
        .iter()
        .cloned()
        .map(|file| std::thread::spawn(move || decode_all(&file)))
        .collect();
    let mut total = 0;
    for thread in threads {
        match thread.join() {
            Ok(Ok((frames, _))) => total += frames,
            _ => return ExitCode::FAILURE,
        }
    }
    let seconds = start.elapsed().as_secs_f64();
    println!(
        "together  {} files: {:.0} frames/s in total, {:.0} per file",
        files.len(),
        total as f64 / seconds,
        total as f64 / seconds / files.len() as f64
    );
    ExitCode::SUCCESS
}
