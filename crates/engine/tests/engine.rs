//! The engine driven frame by frame on real HAP files.
//!
//! Needs a GPU with BC texture support; otherwise the tests print a notice
//! and pass.

#![allow(clippy::unwrap_used)]

use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use prism_core::{Clip, Composition, Direction, FrameRate, LoopMode, Resolution, Slot};
use prism_engine::{ClipState, Command, Engine, EngineState, LayerStatus, Output, ProgramOutput};
use prism_media::BlockFormat;
use prism_media::test_support::{
    Compression, Image, MovOptions, encode_image, solid_plane, write_mov,
};
use prism_render::{Gpu, read_texture};

const FPS: f64 = 60.0;
const SIZE: (u16, u16) = (32, 16);

fn gpu() -> Option<Arc<Gpu>> {
    match Gpu::headless() {
        Ok(gpu) if gpu.supports_block_compression() => Some(Arc::new(gpu)),
        Ok(_) => {
            eprintln!("skipping engine test: no BC texture support");
            None
        }
        Err(error) => {
            eprintln!("skipping engine test: {error}");
            None
        }
    }
}

/// A one-second HAP clip at 60 fps where frame `i` has green level `4 i`,
/// which RGB565 stores exactly: the green channel names the frame.
fn numbered_clip(dir: &Path) -> PathBuf {
    let frames: Vec<_> = (0..60u8)
        .map(|i| {
            let texture = solid_plane(
                BlockFormat::Bc1,
                SIZE.0.into(),
                SIZE.1.into(),
                [0, i * 4, 0, 255],
            );
            encode_image(Image::Rgb, &texture, Compression::Snappy)
        })
        .collect();
    let path = dir.join("numbered.mov");
    write_mov(
        &mut File::create(&path).unwrap(),
        &MovOptions::hap(*b"Hap1", SIZE.0, SIZE.1, 60),
        &frames,
    )
    .unwrap();
    path
}

/// A one-second HAP clip of a single color.
fn solid_clip(dir: &Path, name: &str, rgb: [u8; 3]) -> PathBuf {
    let texture = solid_plane(
        BlockFormat::Bc1,
        SIZE.0.into(),
        SIZE.1.into(),
        [rgb[0], rgb[1], rgb[2], 255],
    );
    let frame = encode_image(Image::Rgb, &texture, Compression::None);
    let path = dir.join(name);
    write_mov(
        &mut File::create(&path).unwrap(),
        &MovOptions::hap(*b"Hap1", SIZE.0, SIZE.1, 60),
        &vec![frame; 60],
    )
    .unwrap();
    path
}

/// One layer, one column, `clip` in the slot, at the test frame rate.
fn composition_with(clip: Clip) -> (Composition, Slot) {
    let mut comp = Composition::new(
        "test",
        Resolution {
            width: 32,
            height: 16,
        },
        FrameRate {
            numerator: 60,
            denominator: 1,
        },
    )
    .unwrap()
    .with_grid(1, 2);
    let slot = Slot::new(comp.layers()[0].id(), comp.columns()[0].id());
    comp.set_clip(slot, Some(clip)).unwrap();
    (comp, slot)
}

/// Drives the engine with simulated time, one frame period per tick.
struct Driver {
    state: EngineState,
    gpu: Arc<Gpu>,
    now: Instant,
}

impl Driver {
    fn new(gpu: Arc<Gpu>, composition: Composition) -> Self {
        Self {
            state: EngineState::new(Arc::clone(&gpu), composition),
            gpu,
            now: Instant::now(),
        }
    }

    fn tick(&mut self) {
        self.state.tick(self.now);
        self.now += Duration::from_secs_f64(1.0 / FPS);
    }

    /// Ticks without advancing time until `done`, giving decoding threads
    /// real time to work. Fails after two seconds.
    fn settle(&mut self, done: impl Fn(&LayerStatus) -> bool) -> LayerStatus {
        let limit = Instant::now() + Duration::from_secs(2);
        loop {
            self.state.tick(self.now);
            let status = self.layer();
            if done(&status) {
                return status;
            }
            assert!(Instant::now() < limit, "engine did not settle: {status:?}");
            thread::sleep(Duration::from_millis(2));
        }
    }

    /// Ticks until the wanted frame is on screen.
    fn wait_for_frame(&mut self) -> LayerStatus {
        self.settle(|status| {
            status.state == ClipState::Playing
                && status.frame.is_some()
                && status.frame == expected_frame(status.position, status.frame_count)
        })
    }

    fn layer(&self) -> LayerStatus {
        self.state.status().layers[0].clone()
    }

    /// Green level of the program image, as a frame number of the numbered clip.
    fn program_frame(&self) -> usize {
        let program = self.state.program();
        let pixel = read_texture(&self.gpu.device, &self.gpu.queue, program.texture()).unwrap()[0];
        (pixel[1] * 63.0).round() as usize
    }
}

fn expected_frame(position: f64, frame_count: usize) -> Option<usize> {
    Some(((position * FPS) as usize).min(frame_count.saturating_sub(1)))
}

#[test]
fn plays_frames_in_real_time() {
    let Some(gpu) = gpu() else { return };
    let dir = tempfile::tempdir().unwrap();
    let (mut comp, slot) = composition_with(Clip::from_file(numbered_clip(dir.path())));
    comp.trigger_clip(slot).unwrap();
    let mut driver = Driver::new(gpu, comp);

    let status = driver.wait_for_frame();
    assert_eq!(status.slot, Some(slot));
    assert_eq!(status.frame_count, 60);
    assert!((status.duration - 1.0).abs() < 1e-9);
    assert_eq!(driver.program_frame(), status.frame.unwrap());
    let start = status.position;

    for _ in 0..30 {
        driver.tick();
    }
    let status = driver.wait_for_frame();
    assert!(
        (status.position - start - 0.5).abs() < 0.02,
        "position {}",
        status.position
    );
    assert_eq!(driver.program_frame(), status.frame.unwrap());
}

#[test]
fn paused_clip_holds_and_backward_clip_starts_at_the_end() {
    let Some(gpu) = gpu() else { return };
    let dir = tempfile::tempdir().unwrap();
    let mut clip = Clip::from_file(numbered_clip(dir.path()));
    clip.playback.direction = Direction::Paused;
    let (mut comp, slot) = composition_with(clip);
    comp.trigger_clip(slot).unwrap();
    let mut driver = Driver::new(Arc::clone(&gpu), comp.clone());
    driver.wait_for_frame();
    for _ in 0..20 {
        driver.tick();
    }
    assert_eq!(driver.wait_for_frame().frame, Some(0));

    comp.clip_mut(slot).unwrap().playback.direction = Direction::Backward;
    comp.clip_mut(slot).unwrap().playback.loop_mode = LoopMode::Once;
    comp.trigger_clip(slot).unwrap();
    driver.state.set_composition(comp);
    let status = driver.wait_for_frame();
    assert_eq!(status.frame, Some(59));
    assert_eq!(driver.program_frame(), 59);
}

#[test]
fn seek_holds_the_playhead_until_released() {
    let Some(gpu) = gpu() else { return };
    let dir = tempfile::tempdir().unwrap();
    let (mut comp, slot) = composition_with(Clip::from_file(numbered_clip(dir.path())));
    comp.trigger_clip(slot).unwrap();
    let mut driver = Driver::new(gpu, comp);
    driver.wait_for_frame();

    driver.state.apply(Command::Seek {
        layer: slot.layer,
        position: 0.75,
        hold: true,
    });
    for _ in 0..10 {
        driver.tick();
    }
    let status = driver.wait_for_frame();
    assert!(status.held);
    assert_eq!(status.frame, Some(45));
    assert_eq!(driver.program_frame(), 45);

    driver.state.apply(Command::Release { layer: slot.layer });
    for _ in 0..6 {
        driver.tick();
    }
    let status = driver.layer();
    assert!(!status.held);
    assert!(status.position > 0.8, "position {}", status.position);
}

#[test]
fn cue_points_and_retrigger_move_the_playhead() {
    let Some(gpu) = gpu() else { return };
    let dir = tempfile::tempdir().unwrap();
    let mut clip = Clip::from_file(numbered_clip(dir.path()));
    clip.playback.cue_points[3] = Some(0.5);
    let (mut comp, slot) = composition_with(clip);
    comp.trigger_clip(slot).unwrap();
    let mut driver = Driver::new(gpu, comp.clone());
    driver.wait_for_frame();

    driver.state.apply(Command::JumpToCue {
        layer: slot.layer,
        index: 3,
    });
    driver.tick();
    let status = driver.wait_for_frame();
    assert!(
        (status.position - 0.5).abs() < 0.02,
        "position {}",
        status.position
    );

    // Triggering the playing clip again restarts it.
    comp.trigger_clip(slot).unwrap();
    driver.state.set_composition(comp);
    let status = driver.wait_for_frame();
    assert!(status.position < 0.02, "position {}", status.position);
}

#[test]
fn missing_media_is_reported_and_leaves_the_layer_black() {
    let Some(gpu) = gpu() else { return };
    let (mut comp, slot) = composition_with(Clip::from_file("/does/not/exist.mov"));
    comp.trigger_clip(slot).unwrap();
    let mut driver = Driver::new(gpu, comp);
    let status = driver.settle(|status| matches!(status.state, ClipState::Error(_)));
    let ClipState::Error(message) = status.state else {
        unreachable!()
    };
    assert!(message.contains("cannot open"), "{message}");
    assert_eq!(driver.program_frame(), 0);
}

#[test]
fn transition_fades_from_one_clip_to_the_next() {
    let Some(gpu) = gpu() else { return };
    let dir = tempfile::tempdir().unwrap();
    let red = Clip::from_file(solid_clip(dir.path(), "red.mov", [255, 0, 0]));
    let blue = Clip::from_file(solid_clip(dir.path(), "blue.mov", [0, 0, 255]));
    let (mut comp, first) = composition_with(red);
    let second = Slot::new(first.layer, comp.columns()[1].id());
    comp.set_clip(second, Some(blue)).unwrap();
    comp.layer_mut(first.layer).unwrap().transition_ms = 500;
    comp.trigger_clip(first).unwrap();
    let mut driver = Driver::new(Arc::clone(&gpu), comp.clone());
    driver.settle(|status| status.state == ClipState::Playing);

    comp.trigger_clip(second).unwrap();
    driver.state.set_composition(comp);
    // The fade starts once the new clip shows its first frame.
    driver.settle(|status| status.state == ClipState::Playing && status.slot == Some(second));
    for _ in 0..15 {
        driver.tick();
    }
    let status = driver.layer();
    let progress = status.transition.unwrap();
    assert!((0.4..0.7).contains(&progress), "progress {progress}");
    let program = driver.state.program();
    let pixel = read_texture(&gpu.device, &gpu.queue, program.texture()).unwrap()[0];
    assert!(pixel[0] > 0.2 && pixel[2] > 0.2, "mid-fade pixel {pixel:?}");
    assert!(
        (pixel[0] + pixel[2] - 1.0).abs() < 0.03,
        "mid-fade pixel {pixel:?}"
    );

    for _ in 0..20 {
        driver.tick();
    }
    assert_eq!(driver.layer().transition, None);
    let pixel = read_texture(&gpu.device, &gpu.queue, program.texture()).unwrap()[0];
    assert!(pixel[0] < 0.01 && pixel[2] > 0.99, "after fade {pixel:?}");
}

#[test]
fn stopping_a_layer_without_transition_clears_it() {
    let Some(gpu) = gpu() else { return };
    let dir = tempfile::tempdir().unwrap();
    let (mut comp, slot) = composition_with(Clip::from_file(solid_clip(
        dir.path(),
        "red.mov",
        [255, 0, 0],
    )));
    comp.trigger_clip(slot).unwrap();
    let mut driver = Driver::new(Arc::clone(&gpu), comp.clone());
    driver.settle(|status| status.state == ClipState::Playing);

    comp.stop_layer(slot.layer).unwrap();
    driver.state.set_composition(comp);
    driver.tick();
    assert_eq!(driver.layer().state, ClipState::Idle);
    let program = driver.state.program();
    let pixel = read_texture(&gpu.device, &gpu.queue, program.texture()).unwrap()[0];
    assert_eq!(pixel, [0.0, 0.0, 0.0, 1.0]);
}

#[test]
fn render_thread_runs_at_the_composition_rate() {
    let Some(gpu) = gpu() else { return };
    let dir = tempfile::tempdir().unwrap();
    let (mut comp, slot) = composition_with(Clip::from_file(numbered_clip(dir.path())));
    let engine = Engine::start(gpu, comp.clone()).unwrap();
    comp.trigger_clip(slot).unwrap();
    engine.set_composition(comp);

    // Shared CI machines can be slow: wait for frames rather than assume a
    // minimum speed.
    let limit = Instant::now() + Duration::from_secs(10);
    while engine.status().frames < 30 || engine.status().layers[0].state != ClipState::Playing {
        assert!(
            Instant::now() < limit,
            "engine stalled: {:?}",
            engine.status()
        );
        thread::sleep(Duration::from_millis(20));
    }

    // The thread renders continuously, never faster than the composition.
    let start = (Instant::now(), engine.status().frames);
    thread::sleep(Duration::from_millis(1000));
    let rendered = engine.status().frames - start.1;
    let rate = rendered as f64 / start.0.elapsed().as_secs_f64();
    assert!(rendered > 0, "no frame rendered in a second");
    assert!(
        rate < 66.0,
        "{rate} frames per second for a 60 fps composition"
    );
    assert_eq!(engine.program().resolution().width, 32);
    drop(engine);
}

/// Counts the frames presented to it.
struct CountingOutput(Arc<AtomicUsize>);

impl Output for CountingOutput {
    fn present(&mut self, _gpu: &Gpu, program: &ProgramOutput) {
        assert_eq!(program.resolution().width, 32);
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn outputs_receive_every_frame_until_removed() {
    let Some(gpu) = gpu() else { return };
    let (comp, _) = composition_with(Clip::from_file("/unused.mov"));
    let mut driver = Driver::new(gpu, comp);
    let first = Arc::new(AtomicUsize::new(0));
    let second = Arc::new(AtomicUsize::new(0));
    driver.state.apply(Command::AddOutput {
        id: 1,
        output: Box::new(CountingOutput(Arc::clone(&first))),
    });
    driver.state.apply(Command::AddOutput {
        id: 2,
        output: Box::new(CountingOutput(Arc::clone(&second))),
    });
    for _ in 0..3 {
        driver.tick();
    }
    driver.state.apply(Command::RemoveOutput { id: 1 });
    for _ in 0..2 {
        driver.tick();
    }
    assert_eq!(first.load(Ordering::SeqCst), 3);
    assert_eq!(second.load(Ordering::SeqCst), 5);
    assert_eq!(driver.state.output_count(), 1);
}
