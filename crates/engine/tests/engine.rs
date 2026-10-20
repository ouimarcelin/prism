//! The engine driven frame by frame on real HAP files.
//!
//! Needs a GPU with BC texture support; otherwise the tests print a notice
//! and pass.

#![allow(clippy::unwrap_used)]

use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use prism_core::{
    Bpm, Clip, Composition, Direction, Effect, FrameRate, LoopMode, Resolution, Slot,
};
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
        self.tick_and_wait();
        self.now += Duration::from_secs_f64(1.0 / FPS);
    }

    /// Renders a frame and waits for the GPU, as a real frame period would:
    /// back-to-back ticks would otherwise outrun the GPU and skip frames.
    fn tick_and_wait(&mut self) {
        self.state.tick(self.now);
        let _ = self
            .gpu
            .device
            .poll(prism_render::wgpu::PollType::wait_indefinitely());
    }

    /// Ticks without advancing time until `done`, giving decoding threads
    /// real time to work. Fails after ten seconds: the first use of a system
    /// decoder, with the other tests running, can take a while.
    fn settle(&mut self, done: impl Fn(&LayerStatus) -> bool) -> LayerStatus {
        let limit = Instant::now() + Duration::from_secs(10);
        loop {
            self.tick_and_wait();
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

/// Writes a `SIZE` PNG of one opaque color.
fn solid_png(path: &Path, rgb: [u8; 3]) {
    let (width, height) = (u32::from(SIZE.0), u32::from(SIZE.1));
    let data: Vec<u8> = (0..width * height).flat_map(|_| rgb).collect();
    let mut encoder = png::Encoder::new(
        std::io::BufWriter::new(File::create(path).unwrap()),
        width,
        height,
    );
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(&data)
        .unwrap();
}

#[test]
fn image_sequences_and_stills_play_like_movies() {
    let Some(gpu) = gpu() else { return };
    let dir = tempfile::tempdir().unwrap();
    let frames = dir.path().join("frames");
    std::fs::create_dir(&frames).unwrap();
    // Three frames at 10 per second, green level naming the frame.
    for index in 0..3u8 {
        solid_png(
            &frames.join(format!("f{}.png", index + 1)),
            [0, index * 4, 0],
        );
    }
    let (mut comp, slot) = composition_with(Clip::from_image_sequence(&frames, 10.0));
    comp.trigger_clip(slot).unwrap();
    let mut driver = Driver::new(Arc::clone(&gpu), comp.clone());
    let status =
        driver.settle(|status| status.state == ClipState::Playing && status.frame.is_some());
    assert_eq!(status.frame_count, 3);
    assert!((status.duration - 0.3).abs() < 1e-9);
    // 0.15 s later: second frame.
    for _ in 0..9 {
        driver.tick();
    }
    let status = driver.settle(|status| status.frame == Some(1));
    assert_eq!(driver.program_frame(), 1);
    assert!(status.position >= 0.1);

    // A still image: one frame, shown as long as the clip plays.
    let still = dir.path().join("still.png");
    solid_png(&still, [0, 8, 0]);
    comp.set_clip(slot, Some(Clip::from_file(&still))).unwrap();
    comp.trigger_clip(slot).unwrap();
    driver.state.set_composition(comp);
    driver.settle(|status| status.frame_count == 1 && status.frame == Some(0));
    for _ in 0..30 {
        driver.tick();
    }
    assert_eq!(driver.program_frame(), 2);
    assert_eq!(driver.layer().state, ClipState::Playing);
}

#[cfg(target_os = "macos")]
#[test]
fn h264_movies_play_through_avfoundation() {
    let Some(gpu) = gpu() else { return };
    // 30 frames at 30 per second: ten red, ten green, ten blue.
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../media/tests/fixtures/rgb-thirds-h264.mp4");
    let (mut comp, slot) = composition_with(Clip::from_file(path));
    comp.set_resolution(Resolution {
        width: 64,
        height: 32,
    })
    .unwrap();
    comp.trigger_clip(slot).unwrap();
    let mut driver = Driver::new(Arc::clone(&gpu), comp);
    let status =
        driver.settle(|status| status.state == ClipState::Playing && status.frame.is_some());
    assert_eq!(status.frame_count, 30);
    // Half a second later: a green frame.
    for _ in 0..30 {
        driver.tick();
    }
    let status =
        driver.settle(|status| status.frame.is_some_and(|frame| (14..=16).contains(&frame)));
    let program = driver.state.program();
    let pixel = read_texture(&gpu.device, &gpu.queue, program.texture()).unwrap()[32 * 16 + 32];
    assert!(
        pixel[1] > pixel[0] && pixel[1] > pixel[2],
        "frame {:?}: {pixel:?}",
        status.frame
    );
}

#[test]
fn stutters_are_counted_after_the_start() {
    let Some(gpu) = gpu() else { return };
    let (comp, _) = composition_with(Clip::from_file("/unused.mov"));
    let mut driver = Driver::new(gpu, comp);
    // A slow start is not a stutter.
    driver.tick_and_wait();
    driver.now += Duration::from_millis(200);
    for _ in 0..130 {
        driver.tick();
    }
    let status = driver.state.status();
    assert_eq!(status.hitches, 0);
    assert_eq!(status.last_hitch, None);
    assert_eq!(status.gpu_skips, 0);
    assert!(status.worst_gpu_latency_ms > 0.0);
    assert!(
        (status.worst_interval_ms - 1000.0 / 60.0).abs() < 0.5,
        "{}",
        status.worst_interval_ms
    );
    assert!(status.worst_steps.total_ms() > 0.0);

    // One frame shown 50 ms after the previous one.
    driver.now += Duration::from_millis(33);
    driver.tick();
    let status = driver.state.status();
    assert_eq!(status.hitches, 1);
    assert!(
        (status.worst_interval_ms - 50.0).abs() < 0.5,
        "{}",
        status.worst_interval_ms
    );
    let hitch = status.last_hitch.unwrap();
    assert_eq!(hitch.frame, status.frames);
    assert!((hitch.interval_ms - 50.0).abs() < 0.5, "{hitch:?}");
    // The delay came from the clock, not from the previous frame.
    assert!(hitch.previous.total_ms() < 25.0, "{hitch:?}");
}

#[test]
fn beats_follow_the_tempo_taps_and_resync() {
    let Some(gpu) = gpu() else { return };
    let (mut comp, _) = composition_with(Clip::from_file("/unused.mov"));
    comp.tempo = Bpm::new(120.0);
    let mut driver = Driver::new(gpu, comp.clone());

    // A bar starts now: one second later, two beats have passed.
    driver.state.apply(Command::Resync { at: driver.now });
    for _ in 0..=60 {
        driver.tick();
    }
    let status = driver.state.status();
    assert_eq!(status.bpm, 120.0);
    assert!((status.beat - 2.0).abs() < 1e-6, "{}", status.beat);

    // A tap at 90 BPM sets the tempo and puts a beat on the tap.
    driver.state.apply(Command::Tap {
        at: driver.now,
        bpm: 90.0,
    });
    driver.tick();
    let status = driver.state.status();
    assert_eq!(status.bpm, 90.0);
    assert!(
        (status.beat - status.beat.round()).abs() < 1e-9,
        "{}",
        status.beat
    );

    // A tempo change in the composition starts from the last frame: one
    // frame at 60 BPM later, the position moved by 1/60 of a beat.
    let before = status.beat;
    comp.tempo = Bpm::new(60.0);
    driver.state.set_composition(comp);
    driver.tick();
    let status = driver.state.status();
    assert_eq!(status.bpm, 60.0);
    assert!(
        (status.beat - before - 1.0 / 60.0).abs() < 1e-6,
        "{}",
        status.beat
    );
}

#[test]
fn effects_of_clip_layer_and_composition_apply_in_order() {
    let Some(gpu) = gpu() else { return };
    let dir = tempfile::tempdir().unwrap();
    let mut clip = Clip::from_file(solid_clip(dir.path(), "red.mov", [255, 0, 0]));
    // Clip: inverted to cyan.
    clip.effects.push(Effect::new("invert"));
    let (mut comp, slot) = composition_with(clip);
    comp.trigger_clip(slot).unwrap();
    let mut driver = Driver::new(Arc::clone(&gpu), comp.clone());
    driver.settle(|status| status.state == ClipState::Playing);
    let program = driver.state.program();
    let pixel = |driver: &mut Driver| {
        driver.tick();
        read_texture(&gpu.device, &gpu.queue, program.texture()).unwrap()[0]
    };
    let cyan = pixel(&mut driver);
    assert!(
        cyan[0] < 0.01 && cyan[1] > 0.99 && cyan[2] > 0.99,
        "{cyan:?}"
    );

    // Layer: no saturation, so the cyan turns gray.
    let mut gray = Effect::new("color");
    gray.set_param("saturation", 0.0);
    comp.layer_mut(slot.layer).unwrap().effects.push(gray);
    driver.state.set_composition(comp.clone());
    let gray = pixel(&mut driver);
    assert!(
        (gray[0] - gray[2]).abs() < 0.01 && gray[0] > 0.5,
        "{gray:?}"
    );

    // Composition: inverted after the mix, a bypassed effect skipped.
    let mut skipped = Effect::new("invert");
    skipped.bypass = true;
    comp.effects = vec![Effect::new("invert"), skipped];
    driver.state.set_composition(comp.clone());
    let inverted = pixel(&mut driver);
    assert!((inverted[0] + gray[0] - 1.0).abs() < 0.01, "{inverted:?}");

    // Without effects, back to the clip as it is.
    comp.effects.clear();
    comp.layer_mut(slot.layer).unwrap().effects.clear();
    comp.clip_mut(slot).unwrap().effects.clear();
    driver.state.set_composition(comp);
    let red = pixel(&mut driver);
    assert!(red[0] > 0.99 && red[1] < 0.01, "{red:?}");
}

#[test]
fn a_group_mixes_its_layers_through_its_effects() {
    let Some(gpu) = gpu() else { return };
    let mut clip = Clip::from_generator("Couleur", "solid");
    clip.source
        .generator_mut()
        .unwrap()
        .set_param("saturation", 1.0);
    let (mut comp, slot) = composition_with(clip);
    // Red layer grouped with an empty layer above it.
    comp.add_layer("dessus");
    let group = comp.group_with_above(slot.layer).unwrap();
    comp.trigger_clip(slot).unwrap();
    let mut driver = Driver::new(Arc::clone(&gpu), comp.clone());
    driver.settle(|status| status.state == ClipState::Playing);
    let program = driver.state.program();
    let pixel = |driver: &mut Driver| {
        driver.tick();
        read_texture(&gpu.device, &gpu.queue, program.texture()).unwrap()[0]
    };
    let red = pixel(&mut driver);
    assert!(red[0] > 0.99 && red[1] < 0.01, "{red:?}");

    // The group's effect, then its opacity over black.
    let settings = comp.group_mut(group).unwrap();
    settings.effects.push(Effect::new("invert"));
    settings.opacity = prism_core::Unit::new(0.5);
    driver.state.set_composition(comp.clone());
    let cyan = pixel(&mut driver);
    assert!(
        cyan[0] < 0.01 && (cyan[1] - 0.5).abs() < 0.02 && (cyan[2] - 0.5).abs() < 0.02,
        "{cyan:?}"
    );

    // Bypassed, the group shows nothing.
    comp.group_mut(group).unwrap().bypass = true;
    driver.state.set_composition(comp);
    let black = pixel(&mut driver);
    assert!(black[0] < 0.01 && black[1] < 0.01, "{black:?}");
}

#[test]
fn generated_clips_follow_their_settings_without_restarting() {
    let Some(gpu) = gpu() else { return };
    let mut clip = Clip::from_generator("Couleur", "solid");
    let red = clip.source.generator_mut().unwrap();
    red.set_param("saturation", 1.0);
    let (mut comp, slot) = composition_with(clip);
    // A fade would show if a new setting restarted the clip.
    comp.layer_mut(slot.layer).unwrap().transition_ms = 1000;
    comp.trigger_clip(slot).unwrap();
    let mut driver = Driver::new(Arc::clone(&gpu), comp.clone());
    let status = driver.settle(|status| status.state == ClipState::Playing);
    assert_eq!(status.slot, Some(slot));
    let program = driver.state.program();
    let pixel = read_texture(&gpu.device, &gpu.queue, program.texture()).unwrap()[0];
    assert!(pixel[0] > 0.99 && pixel[1] < 0.01, "{pixel:?}");

    // Green from the next frame on, without a fade.
    comp.clip_mut(slot)
        .unwrap()
        .source
        .generator_mut()
        .unwrap()
        .set_param("hue", 120.0);
    driver.state.set_composition(comp);
    driver.tick();
    assert_eq!(driver.layer().transition, None);
    let pixel = read_texture(&gpu.device, &gpu.queue, program.texture()).unwrap()[0];
    assert!(pixel[1] > 0.99 && pixel[0] < 0.01, "{pixel:?}");
}

#[test]
fn dashboard_sliders_and_animated_opacity_drive_the_image() {
    let Some(gpu) = gpu() else { return };
    // White solid color whose brightness follows the first slider.
    let mut clip = Clip::from_generator("Couleur", "solid");
    clip.source
        .generator_mut()
        .unwrap()
        .links
        .insert("brightness".into(), 0);
    clip.dashboard[0] = prism_core::Unit::new(0.5);
    let (mut comp, slot) = composition_with(clip);
    comp.trigger_clip(slot).unwrap();
    let mut driver = Driver::new(Arc::clone(&gpu), comp.clone());
    driver.settle(|status| status.state == ClipState::Playing);
    let program = driver.state.program();
    let pixel = |driver: &mut Driver| {
        driver.tick();
        read_texture(&gpu.device, &gpu.queue, program.texture()).unwrap()[0]
    };
    let half = pixel(&mut driver);
    assert!((half[0] - 0.5).abs() < 0.01, "{half:?}");

    comp.clip_mut(slot).unwrap().dashboard[0] = prism_core::Unit::ONE;
    driver.state.set_composition(comp.clone());
    let full = pixel(&mut driver);
    assert!(full[0] > 0.99, "{full:?}");

    // An opacity held at a quarter by its animation.
    comp.layer_mut(slot.layer).unwrap().opacity_animation =
        Some(prism_core::Animation::new(0.25, 0.25));
    driver.state.set_composition(comp);
    let quarter = pixel(&mut driver);
    assert!((quarter[0] - 0.25).abs() < 0.01, "{quarter:?}");
}

#[test]
fn envelopes_start_at_each_launch() {
    let Some(gpu) = gpu() else { return };
    // White solid color whose brightness ramps up over one beat from the
    // launch, then holds.
    let mut clip = Clip::from_generator("Couleur", "solid");
    let mut envelope = prism_core::Animation::new(0.0, 1.0);
    envelope.shape = prism_core::Shape::Ramp;
    envelope.period = prism_core::Period::Beats(1.0);
    envelope.once = true;
    clip.source
        .generator_mut()
        .unwrap()
        .animations
        .insert("brightness".into(), envelope);
    let (mut comp, slot) = composition_with(clip);
    comp.tempo = prism_core::Bpm::new(60.0);
    let mut driver = Driver::new(Arc::clone(&gpu), comp.clone());
    // Half a minute in, long after the start of the engine.
    for _ in 0..(30.0 * FPS) as usize {
        driver.now += Duration::from_secs_f64(1.0 / FPS);
    }
    driver.tick();
    comp.trigger_clip(slot).unwrap();
    driver.state.set_composition(comp.clone());
    driver.settle(|status| status.state == ClipState::Playing);
    let program = driver.state.program();
    let brightness = |driver: &mut Driver| {
        driver.tick();
        read_texture(&gpu.device, &gpu.queue, program.texture()).unwrap()[0][0]
    };
    // At 60 BPM a beat lasts a second: dark at the launch, half way after
    // half a second, then full and held.
    let start = brightness(&mut driver);
    assert!(start < 0.1, "{start}");
    for _ in 0..(FPS / 2.0) as usize - 2 {
        driver.tick();
    }
    let half = brightness(&mut driver);
    assert!((half - 0.5).abs() < 0.1, "{half}");
    for _ in 0..(FPS * 2.0) as usize {
        driver.tick();
    }
    assert!(brightness(&mut driver) > 0.99);

    // Launched again: back to the start.
    comp.trigger_clip(slot).unwrap();
    driver.state.set_composition(comp);
    let again = brightness(&mut driver);
    assert!(again < 0.1, "{again}");
}

#[test]
fn settings_follow_the_bands_of_the_sound() {
    let Some(gpu) = gpu() else { return };
    // White solid color whose brightness follows the bass.
    let mut clip = Clip::from_generator("Couleur", "solid");
    clip.source
        .generator_mut()
        .unwrap()
        .audio
        .insert("brightness".into(), prism_core::Band::Bass);
    let (mut comp, slot) = composition_with(clip);
    comp.trigger_clip(slot).unwrap();
    let mut driver = Driver::new(Arc::clone(&gpu), comp);
    driver.settle(|status| status.state == ClipState::Playing);
    let program = driver.state.program();
    let levels = driver.state.audio_levels();
    let brightness = |driver: &mut Driver| {
        driver.tick();
        read_texture(&gpu.device, &gpu.queue, program.texture()).unwrap()[0][0]
    };
    assert!(brightness(&mut driver) < 0.01);
    levels.set([0.0, 0.75, 0.0, 0.0]);
    let loud = brightness(&mut driver);
    assert!((loud - 0.75).abs() < 0.01, "{loud}");
}

#[cfg(target_os = "macos")]
#[test]
fn clips_sound_through_the_mixer_at_their_volume() {
    let Some(gpu) = gpu() else { return };
    // A quarter of a second: 440 Hz on the left, 1 kHz on the right.
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../media/tests/fixtures/tone-440-1000.mov");
    let (mut comp, slot) = composition_with(Clip::from_file(path));
    comp.trigger_clip(slot).unwrap();
    let (mut mixer, sender) = prism_audio::Mixer::new();
    let mut driver = Driver::new(Arc::clone(&gpu), comp.clone());
    driver.state.apply(Command::SetSound(Some(sender)));
    driver.settle(|status| status.state == ClipState::Playing);
    let loudness = |mixer: &mut prism_audio::Mixer| {
        let mut output = vec![0.0f32; 2 * 4800];
        mixer.render(&mut output, 2, 48_000, 1.0);
        output.iter().map(|sample| sample.abs()).sum::<f32>() / output.len() as f32
    };
    // The sound is decoded on a thread of its own: wait for its voice.
    let limit = Instant::now() + Duration::from_secs(10);
    while loudness(&mut mixer) < 0.01 {
        assert!(
            Instant::now() < limit,
            "no sound: {} voices, {:?}",
            mixer.voice_count(),
            driver.layer()
        );
        driver.tick();
        thread::sleep(Duration::from_millis(5));
    }

    // Volume at zero: silence.
    comp.clip_mut(slot).unwrap().playback.volume = prism_core::Unit::ZERO;
    driver.state.set_composition(comp.clone());
    driver.tick();
    loudness(&mut mixer);
    assert!(loudness(&mut mixer) < 1e-4);

    // Stopped: the voice fades out and leaves.
    comp.stop_all();
    driver.state.set_composition(comp);
    driver.tick();
    loudness(&mut mixer);
    assert_eq!(mixer.voice_count(), 0);
}

#[test]
fn a_missing_camera_is_reported_on_its_layer() {
    let Some(gpu) = gpu() else { return };
    let (mut comp, slot) = composition_with(Clip::from_camera("absent", "Caméra absente"));
    comp.trigger_clip(slot).unwrap();
    let mut driver = Driver::new(Arc::clone(&gpu), comp);
    let status = driver.settle(|status| matches!(status.state, ClipState::Error(_)));
    assert_eq!(status.slot, Some(slot));
    assert_eq!(status.duration, 0.0);
}

#[cfg(target_os = "macos")]
#[test]
fn an_absent_syphon_server_is_waited_for() {
    let Some(gpu) = gpu() else { return };
    let (mut comp, slot) = composition_with(Clip::from_syphon("Nulle part", "Rien"));
    comp.trigger_clip(slot).unwrap();
    let mut driver = Driver::new(Arc::clone(&gpu), comp);
    let status = driver.settle(|status| matches!(status.state, ClipState::Error(_)));
    let ClipState::Error(message) = status.state else {
        unreachable!()
    };
    assert!(message.contains("en attente"), "{message}");
}

#[test]
fn an_ndi_source_is_waited_for_or_explained() {
    let Some(gpu) = gpu() else { return };
    let (mut comp, slot) = composition_with(Clip::from_ndi("NULLE PART (Rien)"));
    comp.trigger_clip(slot).unwrap();
    let mut driver = Driver::new(Arc::clone(&gpu), comp);
    let status = driver.settle(|status| matches!(status.state, ClipState::Error(_)));
    let ClipState::Error(message) = status.state else {
        unreachable!()
    };
    // Without the NDI library or the `ndi` feature, the clip says why.
    let expected = match prism_engine::ndi_status() {
        Ok(_) => "en attente",
        Err(_) => "NDI",
    };
    assert!(message.contains(expected), "{message}");
}

/// Records the size of the image of one screen at each frame.
struct ScreenOutput(prism_core::ScreenId, Arc<Mutex<Vec<Option<u32>>>>);

impl Output for ScreenOutput {
    fn present(&mut self, _gpu: &Gpu, _program: &ProgramOutput) {
        unreachable!("screens are presented through present_images");
    }

    fn present_images(&mut self, _gpu: &Gpu, images: &prism_engine::OutputImages<'_>) {
        let width = images.screen(self.0).map(|image| image.resolution().width);
        self.1.lock().unwrap().push(width);
    }
}

#[test]
fn screens_of_the_stage_are_drawn_for_the_outputs() {
    let Some(gpu) = gpu() else { return };
    let (mut comp, _) = composition_with(Clip::from_file("/unused.mov"));
    let screen = comp.stage.add_screen("Projecteur", 24, 12).unwrap();
    let mut driver = Driver::new(gpu, comp.clone());
    let seen = Arc::new(Mutex::new(Vec::new()));
    driver.state.apply(Command::AddOutput {
        id: 1,
        output: Box::new(ScreenOutput(screen, Arc::clone(&seen))),
    });
    driver.tick();
    assert_eq!(driver.state.screen_count(), 1);
    // A new size makes a new image; a removed screen is no longer drawn.
    comp.stage
        .screen_mut(screen)
        .unwrap()
        .set_size(40, 20)
        .unwrap();
    driver.state.set_composition(comp.clone());
    driver.tick();
    comp.stage.remove_screen(screen);
    driver.state.set_composition(comp);
    driver.tick();
    assert_eq!(driver.state.screen_count(), 0);
    assert_eq!(*seen.lock().unwrap(), [Some(24), Some(40), None]);
}

#[test]
fn the_simulation_is_drawn_while_the_stage_asks_for_it() {
    let Some(gpu) = gpu() else { return };
    let (mut comp, _) = composition_with(Clip::from_file("/unused.mov"));
    comp.stage.add_screen("A", 40, 20).unwrap();
    comp.stage.add_screen("B", 40, 20).unwrap();
    comp.stage.simulation.overlap = 10;
    let mut driver = Driver::new(gpu, comp.clone());
    driver.tick();
    assert!(driver.state.simulation_image().is_none());
    comp.stage.simulation.enabled = true;
    driver.state.set_composition(comp);
    driver.tick();
    let image = driver.state.simulation_image().unwrap();
    assert_eq!(image.resolution().width, 70);
}

/// The criterion of phase 4, rehearsed: two projectors overlapping on a
/// strip, each fading its edge there, give an even image.
#[test]
fn two_blended_projectors_show_no_seam_in_the_simulation() {
    let Some(gpu) = gpu() else { return };
    let mut clip = Clip::from_generator("Gris", "solid");
    clip.source
        .generator_mut()
        .unwrap()
        .set_param("brightness", 0.6);
    let (mut comp, slot) = composition_with(clip);
    comp.trigger_clip(slot).unwrap();
    // Composition 32 pixels wide, spread over two projectors of 40 pixels
    // sharing 16: each shows 20 composition pixels, 8 of them shared.
    comp.stage.add_screen("Gauche", 40, 16).unwrap();
    comp.stage.add_screen("Droite", 40, 16).unwrap();
    comp.stage.simulation.overlap = 16;
    comp.stage.spread();
    comp.stage.simulation.enabled = true;
    comp.stage.simulation.overlap = 16;
    let mut driver = Driver::new(Arc::clone(&gpu), comp);
    driver.settle(|status| status.state == ClipState::Playing);
    driver.tick();
    let image = driver.state.simulation_image().unwrap();
    assert_eq!(image.resolution().width, 64);
    let pixels = read_texture(&gpu.device, &gpu.queue, image.texture()).unwrap();
    for x in 0..64 {
        let value = pixels[8 * 64 + x][0];
        assert!((value - 0.6).abs() < 0.02, "pixel {x}: {value}");
    }
    // Without the fades, the strip is plainly brighter.
    let mut comp = driver.state.composition().clone();
    for screen in comp
        .stage
        .screens()
        .iter()
        .map(prism_core::Screen::id)
        .collect::<Vec<_>>()
    {
        comp.stage.screen_mut(screen).unwrap().slices[0].blend = Default::default();
    }
    driver.state.set_composition(comp);
    driver.tick();
    let pixels = read_texture(&gpu.device, &gpu.queue, image.texture()).unwrap();
    assert!(pixels[8 * 64 + 32][0] > 0.75, "{}", pixels[8 * 64 + 32][0]);
}

/// The criterion of phase 5, first half: an Art-Net LED strip follows the
/// image.
#[test]
fn an_art_net_strip_follows_the_image() {
    let Some(gpu) = gpu() else { return };
    let receiver = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
    receiver
        .set_read_timeout(Some(Duration::from_millis(200)))
        .unwrap();
    let port = receiver.local_addr().unwrap().port();

    let mut clip = Clip::from_generator("Couleur", "solid");
    clip.source
        .generator_mut()
        .unwrap()
        .set_param("saturation", 1.0);
    let (mut comp, slot) = composition_with(clip);
    comp.trigger_clip(slot).unwrap();
    comp.lighting.enabled = true;
    comp.lighting.target = format!("127.0.0.1:{port}");
    comp.lighting.rate = 60.0;
    let strip = comp.lighting.add_strip("Bande", 10);
    comp.lighting.fixture_mut(strip).unwrap().universe = 3;
    let mut driver = Driver::new(Arc::clone(&gpu), comp.clone());
    driver.settle(|status| status.state == ClipState::Playing);

    // Ticks until a universe arrives whose first pixel is `color`.
    let wait_for = |driver: &mut Driver, color: [u8; 3]| {
        let limit = Instant::now() + Duration::from_secs(5);
        let mut buffer = [0u8; 1024];
        loop {
            driver.tick();
            if let Ok(length) = receiver.recv(&mut buffer)
                && let Some((universe, channels)) = prism_dmx::artnet::parse_dmx(&buffer[..length])
                && channels[..3] == color
            {
                assert_eq!(universe, 3);
                assert_eq!(channels[27..30], color, "tenth pixel");
                assert_eq!(channels[30..33], [0, 0, 0], "past the strip");
                return;
            }
            assert!(Instant::now() < limit, "no universe with {color:?}");
        }
    };
    wait_for(&mut driver, [255, 0, 0]);
    comp.clip_mut(slot)
        .unwrap()
        .source
        .generator_mut()
        .unwrap()
        .set_param("hue", 120.0);
    driver.state.set_composition(comp);
    wait_for(&mut driver, [0, 255, 0]);
    assert!(driver.state.status().dmx_sent > 0);
    assert_eq!(driver.state.status().dmx_error, None);
}

#[test]
fn a_clip_set_on_the_timecode_plays_where_it_says() {
    let Some(gpu) = gpu() else { return };
    let dir = tempfile::tempdir().unwrap();
    let mut clip = Clip::from_file(numbered_clip(dir.path()));
    // The media starts at 100 s of timecode.
    clip.playback.timecode_start = Some(100.0);
    let (mut comp, slot) = composition_with(clip);
    comp.trigger_clip(slot).unwrap();
    let mut driver = Driver::new(gpu, comp);
    driver.wait_for_frame();

    // Timecode at 100.25 s now: frame 15 of the 60 frames per second
    // clip; read one frame period later, frame 16.
    driver
        .state
        .apply(Command::Timecode(Some((100.25, driver.now))));
    driver.tick();
    let status = driver.wait_for_frame();
    assert_eq!(status.frame, Some(16));
    // The timecode runs on with time: a quarter second later, frame 30.
    for _ in 0..14 {
        driver.tick();
    }
    let status = driver.wait_for_frame();
    assert_eq!(status.frame, Some(30));
    // Stopped: the clip holds still.
    driver.state.apply(Command::Timecode(None));
    for _ in 0..10 {
        driver.tick();
    }
    assert_eq!(driver.wait_for_frame().frame, Some(30));
}
