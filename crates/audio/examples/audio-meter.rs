//! Shows the levels of an audio input in the terminal: to check the input
//! and the bands by ear.
//!
//! ```text
//! cargo run -p prism-audio --example audio-meter -- --list
//! cargo run -p prism-audio --example audio-meter -- [DEVICE] [GAIN_DB]
//! ```
//!
//! macOS asks once for access to the microphone.

use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use prism_audio::{AudioInput, AudioLevels, input_devices};
use prism_core::Band;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|arg| arg == "--list") {
        for name in input_devices() {
            println!("{name}");
        }
        return ExitCode::SUCCESS;
    }
    let levels = Arc::new(AudioLevels::default());
    levels.set_gain_db(
        args.get(1)
            .and_then(|gain| gain.parse().ok())
            .unwrap_or(0.0),
    );
    let input = match AudioInput::start(args.first().map(String::as_str), Arc::clone(&levels)) {
        Ok(input) => input,
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCode::FAILURE;
        }
    };
    println!("input     {}", input.name());
    println!("bands     {:?}", Band::ALL);
    loop {
        std::thread::sleep(Duration::from_millis(50));
        let bars: Vec<String> = levels
            .get()
            .iter()
            .map(|level| {
                let filled = (level * 20.0).round() as usize;
                format!("{:<20}", "#".repeat(filled))
            })
            .collect();
        print!("\r{}", bars.join(" | "));
        use std::io::Write as _;
        let _ = std::io::stdout().flush();
    }
}
