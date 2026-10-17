//! Syphon output driven by the engine. Needs a GPU; without one the test
//! prints a notice and passes.

#![cfg(target_os = "macos")]
#![allow(clippy::unwrap_used)]

use std::sync::Arc;
use std::time::{Duration, Instant};

use prism_core::Composition;
use prism_engine::{Command, EngineState};
use prism_io_video::SyphonOutput;
use prism_render::Gpu;

#[test]
fn the_engine_publishes_frames_over_syphon() {
    let gpu = match Gpu::headless() {
        Ok(gpu) => Arc::new(gpu),
        Err(error) => {
            eprintln!("skipping GPU test: {error}");
            return;
        }
    };
    let mut state = EngineState::new(Arc::clone(&gpu), Composition::default());
    state.apply(Command::AddOutput {
        id: 1,
        output: Box::new(SyphonOutput::new("Prism test")),
    });
    let mut now = Instant::now();
    for _ in 0..10 {
        state.tick(now);
        now += Duration::from_millis(16);
        let _ = gpu
            .device
            .poll(prism_render::wgpu::PollType::wait_indefinitely());
    }
    state.apply(Command::RemoveOutput { id: 1 });

    // Presented directly: the server starts on the device of wgpu.
    let mut output = SyphonOutput::new("Prism test direct");
    use prism_engine::Output as _;
    output.present(&gpu, &state.program());
    assert!(output.is_publishing());
    assert!(!output.has_clients());
}
