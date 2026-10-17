//! The render thread: plays the clips of a composition, mixes them and keeps
//! the program image up to date, at the composition frame rate.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use prism_audio::{AudioLevels, VoiceCommand};
use prism_clock::BeatClock;
use prism_core::{Composition, Effect, LayerId, Resolution, ScreenId, Slot, Unit};
use prism_fx::{EffectChain, EffectRenderer, FrameTime};
use prism_render::{
    Compositor, FrameUploader, Gpu, LayerInput, ScreenTarget, StageRenderer, Transition,
    WORKING_FORMAT, solid_color_texture, wgpu,
};

use crate::random::Random;
use crate::status::{EngineStatus, FrameSteps, Hitch, LayerStatus};
use crate::stream::ClipStream;

/// A destination of the program image: a window, later a network stream.
///
/// The render thread calls [`Output::present`] after every frame, so it must
/// not wait long: skip the frame rather than block when the destination is
/// not ready.
pub trait Output: Send {
    /// Shows `program`.
    fn present(&mut self, gpu: &Gpu, program: &ProgramOutput);

    /// Shows one of `images`: the program, unless the output shows a screen
    /// of the stage.
    fn present_images(&mut self, gpu: &Gpu, images: &OutputImages<'_>) {
        self.present(gpu, images.program());
    }
}

/// The images of a frame an output may show: the program and the screens
/// of the stage.
pub struct OutputImages<'a> {
    program: &'a ProgramOutput,
    screens: &'a [(ScreenId, ProgramOutput)],
}

impl<'a> OutputImages<'a> {
    /// The program image.
    pub fn program(&self) -> &'a ProgramOutput {
        self.program
    }

    /// The image of the screen `id`, drawn from the program this frame.
    pub fn screen(&self, id: ScreenId) -> Option<&'a ProgramOutput> {
        self.screens
            .iter()
            .find(|(screen, _)| *screen == id)
            .map(|(_, image)| image)
    }
}

/// Instruction sent to the render thread.
pub enum Command {
    /// Plays this composition from now on. Clips keep playing when they are
    /// still triggered; [`prism_core::Layer::trigger_count`] tells the engine
    /// to restart a clip triggered again.
    SetComposition(Box<Composition>),
    /// Moves the playhead of a layer; `hold` keeps it there (scratching)
    /// until [`Command::Release`].
    Seek {
        /// Layer concerned.
        layer: LayerId,
        /// Position in seconds from the start of the media.
        position: f64,
        /// Whether to hold the playhead at that position.
        hold: bool,
    },
    /// Lets a held playhead move again.
    Release {
        /// Layer concerned.
        layer: LayerId,
    },
    /// Jumps to a cue point of the clip playing on a layer.
    JumpToCue {
        /// Layer concerned.
        layer: LayerId,
        /// Cue point index.
        index: usize,
    },
    /// Presents the program to `output` after every frame, from now on.
    AddOutput {
        /// Identifier chosen by the caller, to remove the output later.
        id: u64,
        /// The destination.
        output: Box<dyn Output>,
    },
    /// Stops presenting to an output and drops it.
    RemoveOutput {
        /// Identifier given to [`Command::AddOutput`].
        id: u64,
    },
    /// A tap of the tap tempo: from `at` on, the tempo is `bpm` and a beat
    /// falls at `at`. Set the same tempo in the composition.
    Tap {
        /// Instant of the tap.
        at: Instant,
        /// Tempo measured from the taps, in beats per minute.
        bpm: f64,
    },
    /// Starts a bar at `at`.
    Resync {
        /// Instant of the first beat of the bar.
        at: Instant,
    },
    /// Plays the sound of the clips through this mixer, or stops it.
    SetSound(Option<Sender<VoiceCommand>>),
}

impl std::fmt::Debug for Command {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SetComposition(_) => f.write_str("SetComposition"),
            Self::Seek {
                layer,
                position,
                hold,
            } => write!(f, "Seek({layer}, {position}, hold: {hold})"),
            Self::Release { layer } => write!(f, "Release({layer})"),
            Self::JumpToCue { layer, index } => write!(f, "JumpToCue({layer}, {index})"),
            Self::AddOutput { id, .. } => write!(f, "AddOutput({id})"),
            Self::RemoveOutput { id } => write!(f, "RemoveOutput({id})"),
            Self::Tap { bpm, .. } => write!(f, "Tap({bpm} BPM)"),
            Self::Resync { .. } => f.write_str("Resync"),
            Self::SetSound(sound) => write!(f, "SetSound({})", sound.is_some()),
        }
    }
}

/// The mixed image of the composition, refreshed after every frame.
///
/// Readers sample it at any time: the GPU runs work in submission order,
/// so a reader sees a complete frame, never one being mixed.
#[derive(Debug)]
pub struct ProgramOutput {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    resolution: Resolution,
}

impl ProgramOutput {
    fn new(device: &wgpu::Device, resolution: Resolution) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("program"),
            size: wgpu::Extent3d {
                width: resolution.width,
                height: resolution.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: WORKING_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            texture,
            view,
            resolution,
        }
    }

    /// Program texture ([`WORKING_FORMAT`], premultiplied, display space).
    pub fn texture(&self) -> &wgpu::Texture {
        &self.texture
    }

    /// View of the program texture.
    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    /// Size of the program image.
    pub fn resolution(&self) -> Resolution {
        self.resolution
    }
}

/// Runtime state of one layer: the clip playing and the clip fading out.
struct LayerRuntime {
    id: LayerId,
    current: Option<ClipStream>,
    previous: Option<ClipStream>,
    transition_elapsed: f64,
    transition_duration: f64,
    seen_triggers: u64,
}

impl LayerRuntime {
    fn new(id: LayerId, seen_triggers: u64) -> Self {
        Self {
            id,
            current: None,
            previous: None,
            transition_elapsed: 0.0,
            transition_duration: 0.0,
            seen_triggers,
        }
    }

    /// Replaces the clip playing, fading from the old one when a transition
    /// time is set and the old clip has a frame on screen.
    fn switch(&mut self, next: Option<ClipStream>, transition_ms: u32) {
        let old = std::mem::replace(&mut self.current, next);
        if transition_ms > 0 && old.as_ref().is_some_and(ClipStream::has_frame) {
            self.previous = old;
            self.transition_elapsed = 0.0;
            self.transition_duration = f64::from(transition_ms) / 1000.0;
        } else {
            self.previous = None;
        }
    }

    /// Progress of the running fade.
    fn transition_progress(&self) -> Option<f32> {
        self.previous.as_ref().map(|_| {
            if self.transition_duration > 0.0 {
                (self.transition_elapsed / self.transition_duration).clamp(0.0, 1.0) as f32
            } else {
                1.0
            }
        })
    }

    /// Advances the fade. It waits for the first frame of the new clip.
    fn advance_transition(&mut self, elapsed: f64) {
        if self.previous.is_none() {
            return;
        }
        let incoming_ready = self
            .current
            .as_ref()
            .is_none_or(|stream| stream.has_frame() || stream.has_failed());
        if incoming_ready {
            self.transition_elapsed += elapsed;
        }
        if self.transition_elapsed >= self.transition_duration {
            self.previous = None;
        }
    }

    fn streams_mut(&mut self) -> impl Iterator<Item = &mut ClipStream> {
        self.current.iter_mut().chain(self.previous.iter_mut())
    }

    fn status(&self) -> LayerStatus {
        let mut status = LayerStatus {
            layer: Some(self.id),
            transition: self.transition_progress(),
            ..LayerStatus::default()
        };
        if let Some(stream) = &self.current {
            status.slot = Some(stream.slot());
            status.state = stream.state();
            if let Some((position, duration, frame_count)) = stream.timing() {
                status.position = position;
                status.duration = duration;
                status.frame_count = frame_count;
            }
            status.frame = stream.shown_frame();
            status.held = stream.is_held();
            status.late_frames = stream.late_frames();
            status.frame_errors = stream.frame_errors();
        }
        status
    }
}

/// Frames rendered before intervals count, to leave out the start.
const WARMUP_FRAMES: u64 = 120;

/// Frames whose GPU work may be pending at once. When the GPU falls this
/// far behind, the render thread leaves it alone for a tick instead of
/// piling up frames, and the upload buffers they hold.
const MAX_FRAMES_IN_FLIGHT: u64 = 3;

/// Progress of the GPU, updated by completion callbacks on whichever thread
/// polls the device.
#[derive(Default)]
struct GpuProgress {
    /// Frames whose GPU work is finished.
    completed: AtomicU64,
    /// Longest time between the submission of a frame and the moment its
    /// work was seen finished, in microseconds, after the start.
    worst_latency_us: AtomicU64,
}

/// Frame-rate statistics over a sliding second.
struct Stats {
    frames: u64,
    late_ticks: u64,
    window_start: Option<Instant>,
    window_frames: u32,
    fps: f64,
    frame_time_ms: f64,
    previous: Option<Instant>,
    worst_interval_ms: f64,
    hitches: u64,
    worst_work_ms: f64,
    last_steps: FrameSteps,
    worst_steps: FrameSteps,
    last_hitch: Option<Hitch>,
    real_time: bool,
    gpu_skips: u64,
}

/// Everything the render thread owns. Driven by [`EngineState::tick`], so
/// tests can run it frame by frame without a thread.
pub struct EngineState {
    gpu: Arc<Gpu>,
    composition: Composition,
    layers: Vec<LayerRuntime>,
    compositor: Compositor,
    uploader: FrameUploader,
    transparent: wgpu::TextureView,
    program: Arc<ProgramOutput>,
    /// Images of the screens of the stage, drawn after the program.
    screens: Vec<(ScreenId, ProgramOutput)>,
    stage_renderer: StageRenderer,
    outputs: Vec<(u64, Box<dyn Output>)>,
    random: Random,
    fx: EffectRenderer,
    composition_effects: EffectChain,
    /// Image the layers are mixed into when the composition has effects.
    mix: Option<(Resolution, wgpu::TextureView)>,
    started: Instant,
    clock: BeatClock,
    beat: f64,
    last_tick: Option<Instant>,
    submitted: u64,
    gpu_progress: Arc<GpuProgress>,
    stats: Stats,
    audio: Arc<AudioLevels>,
    /// Mixer of the sound of the clips, if sound is on.
    sound: Option<Sender<VoiceCommand>>,
    next_voice: u64,
}

impl EngineState {
    /// Prepares the GPU resources to play `composition`.
    pub fn new(gpu: Arc<Gpu>, composition: Composition) -> Self {
        let resolution = composition.resolution();
        let compositor = Compositor::new(&gpu.device, resolution);
        let uploader = FrameUploader::new(&gpu.device, &gpu.queue);
        let transparent = solid_color_texture(&gpu.device, &gpu.queue, [0.0; 4])
            .create_view(&wgpu::TextureViewDescriptor::default());
        let program = Arc::new(ProgramOutput::new(&gpu.device, resolution));
        let fx = EffectRenderer::new(&gpu.device);
        let stage_renderer = StageRenderer::new(&gpu.device);
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(1, |time| time.as_nanos() as u64);
        let mut state = Self {
            gpu,
            composition: Composition::default(),
            layers: Vec::new(),
            compositor,
            uploader,
            transparent,
            stage_renderer,
            program,
            screens: Vec::new(),
            outputs: Vec::new(),
            random: Random::new(seed),
            fx,
            composition_effects: EffectChain::default(),
            mix: None,
            started: Instant::now(),
            clock: BeatClock::new(composition.tempo.get(), Instant::now()),
            beat: 0.0,
            last_tick: None,
            submitted: 0,
            gpu_progress: Arc::default(),
            audio: Arc::default(),
            sound: None,
            next_voice: 0,
            stats: Stats {
                frames: 0,
                late_ticks: 0,
                window_start: None,
                window_frames: 0,
                fps: 0.0,
                frame_time_ms: 0.0,
                previous: None,
                worst_interval_ms: 0.0,
                hitches: 0,
                worst_work_ms: 0.0,
                last_steps: FrameSteps::default(),
                worst_steps: FrameSteps::default(),
                last_hitch: None,
                real_time: false,
                gpu_skips: 0,
            },
        };
        state.set_composition(composition);
        state
    }

    /// Composition played.
    pub fn composition(&self) -> &Composition {
        &self.composition
    }

    /// Program image. Replaced when the composition resolution changes.
    pub fn program(&self) -> Arc<ProgramOutput> {
        Arc::clone(&self.program)
    }

    /// Levels of the sound that settings follow: whoever measures the
    /// sound writes them here.
    pub fn audio_levels(&self) -> Arc<AudioLevels> {
        Arc::clone(&self.audio)
    }

    /// Plays `composition` from the next frame.
    pub fn set_composition(&mut self, composition: Composition) {
        let resolution = composition.resolution();
        if resolution != self.program.resolution() {
            self.compositor.resize(resolution);
            self.program = Arc::new(ProgramOutput::new(&self.gpu.device, resolution));
        }
        let bpm = composition.tempo.get();
        if bpm != self.clock.bpm() {
            // From the last frame on, so the beat position does not jump.
            let time = self.last_tick.unwrap_or_else(Instant::now);
            self.clock.set_bpm(bpm, time);
        }
        self.composition = composition;
        self.sync_layers();
        self.enable_sounds();
    }

    /// Applies a command.
    pub fn apply(&mut self, command: Command) {
        match command {
            Command::SetComposition(composition) => self.set_composition(*composition),
            Command::Seek {
                layer,
                position,
                hold,
            } => {
                if let Some(stream) = self.current_stream(layer) {
                    stream.seek(position, hold);
                }
            }
            Command::Release { layer } => {
                if let Some(stream) = self.current_stream(layer) {
                    stream.release();
                }
            }
            Command::JumpToCue { layer, index } => {
                let cue = self
                    .composition
                    .active_clip(layer)
                    .and_then(|(_, clip)| clip.playback.cue_points.get(index).copied().flatten());
                if let (Some(position), Some(stream)) = (cue, self.current_stream(layer)) {
                    stream.seek(position, false);
                }
            }
            Command::AddOutput { id, output } => {
                self.outputs.retain(|(existing, _)| *existing != id);
                self.outputs.push((id, output));
            }
            Command::RemoveOutput { id } => self.outputs.retain(|(existing, _)| *existing != id),
            Command::Tap { at, bpm } => {
                self.clock.set_bpm(bpm, at);
                self.clock.align_beat(at);
            }
            Command::Resync { at } => self.clock.restart_bar(at),
            Command::SetSound(sound) => {
                self.sound = sound;
                if self.sound.is_none() {
                    // Dropping their sound stops the voices.
                    for runtime in &mut self.layers {
                        for stream in runtime.streams_mut() {
                            stream.disable_sound();
                        }
                    }
                }
                self.enable_sounds();
            }
        }
    }

    /// Tells the mixer where each clip with sound is. A clip sounds at its
    /// volume times the opacity of its layer (crossfader, solo and bypass
    /// included); during a transition, the two clips fade into each other.
    fn update_sounds(&mut self, elapsed: f64, time: FrameTime) {
        if self.sound.is_none() {
            return;
        }
        for runtime in &mut self.layers {
            let progress = runtime.transition_progress();
            let (beat, seconds) = runtime
                .current
                .as_ref()
                .map_or((0.0, 0.0), ClipStream::launched);
            let moment = time.moment().launched_at(beat, seconds);
            let layer = self
                .composition
                .effective_opacity_at(runtime.id, moment, time.audio)
                .unwrap_or(0.0);
            let composition = &self.composition;
            let volume = |stream: &ClipStream| {
                composition
                    .clip(stream.slot())
                    .map_or(0.0, |clip| clip.playback.volume.get())
            };
            if let Some(stream) = &mut runtime.current {
                let gain = volume(stream) * layer * progress.unwrap_or(1.0);
                stream.update_sound(elapsed, gain);
            }
            if let Some(stream) = &mut runtime.previous {
                let gain = volume(stream) * layer * (1.0 - progress.unwrap_or(1.0));
                stream.update_sound(elapsed, gain);
            }
        }
    }

    /// Gives every clip playing its sound, when there is a mixer.
    fn enable_sounds(&mut self) {
        let Some(mixer) = &self.sound else {
            return;
        };
        for runtime in &mut self.layers {
            for stream in runtime.streams_mut() {
                self.next_voice += 1;
                stream.enable_sound(self.next_voice, mixer);
            }
        }
    }

    /// Number of outputs presented after every frame.
    pub fn output_count(&self) -> usize {
        self.outputs.len()
    }

    fn current_stream(&mut self, layer: LayerId) -> Option<&mut ClipStream> {
        self.layers
            .iter_mut()
            .find(|runtime| runtime.id == layer)
            .and_then(|runtime| runtime.current.as_mut())
    }

    /// Starts, restarts or stops clips to match the composition.
    fn sync_layers(&mut self) {
        // Clips launched now start their envelopes at the last frame, on the
        // clock frames are rendered with.
        let now = self.last_tick.unwrap_or_else(Instant::now);
        let launch = (
            self.clock.beat_at(now),
            now.saturating_duration_since(self.started).as_secs_f64(),
        );
        let composition = &self.composition;
        self.layers
            .retain(|runtime| composition.layer(runtime.id).is_some());

        for layer in composition.layers() {
            let id = layer.id();
            let index = match self.layers.iter().position(|runtime| runtime.id == id) {
                Some(index) => index,
                None => {
                    // A new layer starts nothing that was triggered before.
                    self.layers.push(LayerRuntime::new(id, 0));
                    self.layers.len() - 1
                }
            };
            let runtime = &mut self.layers[index];
            let retriggered = layer.trigger_count() != runtime.seen_triggers;
            runtime.seen_triggers = layer.trigger_count();

            let wanted = composition
                .active_clip(id)
                .map(|(column, clip)| (Slot::new(id, column), &clip.source, &clip.playback));
            match wanted {
                Some((slot, source, playback)) => match runtime.current.as_mut() {
                    Some(stream) if stream.plays(slot, source) => {
                        stream.set_playback(playback);
                        if retriggered {
                            stream.restart(launch);
                        }
                    }
                    _ => {
                        let stream =
                            ClipStream::open(slot, source.clone(), playback.clone(), launch);
                        runtime.switch(Some(stream), layer.transition_ms);
                    }
                },
                None => {
                    if runtime.current.is_some() {
                        runtime.switch(None, layer.transition_ms);
                    }
                }
            }
        }
        // Keep the runtime list in composition order.
        let order: Vec<LayerId> = composition
            .layers()
            .iter()
            .map(|layer| layer.id())
            .collect();
        self.layers.sort_by_key(|runtime| {
            order
                .iter()
                .position(|id| *id == runtime.id)
                .unwrap_or(usize::MAX)
        });
    }

    /// Renders one frame at time `now`.
    pub fn tick(&mut self, now: Instant) {
        let started = Instant::now();
        let elapsed = self.last_tick.map_or(0.0, |last| {
            now.saturating_duration_since(last).as_secs_f64()
        });
        self.last_tick = Some(now);
        self.beat = self.clock.beat_at(now);
        let tick = 1.0 / self.composition.frame_rate().as_f64();

        // Playback.
        for runtime in &mut self.layers {
            runtime.advance_transition(elapsed);
            for stream in runtime.streams_mut() {
                if let Some(clip) = self.composition.clip(stream.slot()) {
                    stream.set_playback(&clip.playback);
                }
                stream.receive();
                stream.advance(elapsed, tick, &mut self.random);
            }
        }
        let played = Instant::now();

        // GPU: frame uploads and mixing into the program image, in one
        // submission. Finished frames, and the upload buffers they held, are
        // collected first, without waiting.
        let gpu = Arc::clone(&self.gpu);
        let _ = gpu.device.poll(wgpu::PollType::Poll);
        let finished = self.gpu_progress.completed.load(Ordering::Acquire);
        if self.submitted.saturating_sub(finished) >= MAX_FRAMES_IN_FLIGHT {
            // No new image: the previous one stays on screen, and the next
            // image counts as a stutter.
            self.stats.gpu_skips += 1;
            return;
        }
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("engine frame"),
            });
        for runtime in &mut self.layers {
            for stream in runtime.streams_mut() {
                stream.upload(&gpu, &mut self.uploader, &mut encoder);
            }
        }
        self.uploader.finish(&encoder);

        // Effects of each clip, then of its layer; generated clips drawn.
        let time = FrameTime {
            seconds: now.saturating_duration_since(self.started).as_secs_f64(),
            beat: self.beat,
            audio: self.audio.get(),
            ..FrameTime::default()
        };
        for runtime in &mut self.layers {
            let layer_effects = self
                .composition
                .layer(runtime.id)
                .map_or(&[][..], |layer| &layer.effects[..]);
            for stream in runtime.streams_mut() {
                let clip = self.composition.clip(stream.slot());
                let clip_effects = clip.map_or(&[][..], |clip| &clip.effects[..]);
                // Settings follow the dashboard of the clip, envelopes its
                // launch.
                let mut time = time;
                if let Some(clip) = clip {
                    time.dashboard = clip.dashboard.map(Unit::get);
                }
                (time.launch_beat, time.launch_seconds) = stream.launched();
                let generator = clip.and_then(|clip| clip.source.generator());
                match generator {
                    // A generated clip is drawn at the composition size.
                    Some(generator) if stream.is_generator() => {
                        let effects: Vec<&Effect> = std::iter::once(generator)
                            .chain(clip_effects)
                            .chain(layer_effects)
                            .collect();
                        let size = self.composition.resolution();
                        stream.generate(
                            &self.fx,
                            &gpu,
                            &mut encoder,
                            &effects,
                            time,
                            &self.transparent,
                            (size.width, size.height),
                        );
                    }
                    _ => {
                        let effects: Vec<&Effect> =
                            clip_effects.iter().chain(layer_effects).collect();
                        stream.apply_effects(&self.fx, &gpu, &mut encoder, &effects, time);
                    }
                }
            }
        }
        // With effects on the composition, the layers are mixed into an
        // image of their own first.
        let composition_effects: Vec<&Effect> = self.composition.effects.iter().collect();
        let resolution = self.program.resolution();
        let mix = prism_fx::is_active(&composition_effects).then(|| {
            self.mix
                .take()
                .filter(|(size, _)| *size == resolution)
                .unwrap_or_else(|| (resolution, mix_texture(&gpu.device, resolution)))
        });
        let uploaded = Instant::now();

        let mut inputs = Vec::with_capacity(self.layers.len());
        for runtime in &self.layers {
            // An opacity envelope starts with the clip playing on the layer.
            let (beat, seconds) = runtime
                .current
                .as_ref()
                .map_or((0.0, 0.0), ClipStream::launched);
            let moment = time.moment().launched_at(beat, seconds);
            let Some(opacity) = self
                .composition
                .effective_opacity_at(runtime.id, moment, time.audio)
            else {
                continue;
            };
            let Some(layer) = self.composition.layer(runtime.id) else {
                continue;
            };
            if opacity <= 0.0 {
                continue;
            }
            let current = runtime.current.as_ref().and_then(ClipStream::view);
            let previous = runtime.previous.as_ref().and_then(ClipStream::view);
            let input = match (current, previous) {
                (texture, Some(previous)) => LayerInput {
                    transition: Some(Transition {
                        previous,
                        progress: runtime.transition_progress().unwrap_or(1.0),
                    }),
                    ..LayerInput::new(
                        texture.unwrap_or(&self.transparent),
                        opacity,
                        layer.blend_mode,
                    )
                },
                (Some(texture), None) => LayerInput::new(texture, opacity, layer.blend_mode),
                (None, None) => continue,
            };
            inputs.push(input);
        }
        let destination = mix.as_ref().map_or(&self.program.view, |(_, view)| view);
        self.compositor
            .encode(&gpu.device, &gpu.queue, &mut encoder, &inputs, destination);
        drop(inputs);
        if let Some((_, view)) = &mix {
            // Envelopes of the composition start at the last launch.
            let mut time = time;
            if let Some((beat, seconds)) = self
                .layers
                .iter()
                .filter_map(|runtime| runtime.current.as_ref().map(ClipStream::launched))
                .max_by(|a, b| a.1.total_cmp(&b.1))
            {
                (time.launch_beat, time.launch_seconds) = (beat, seconds);
            }
            self.composition_effects.apply_into(
                &self.fx,
                &gpu.device,
                &gpu.queue,
                &mut encoder,
                &composition_effects,
                view,
                (resolution.width, resolution.height),
                time,
                &self.program.view,
            );
        }
        self.mix = mix;
        self.draw_screens(&gpu, &mut encoder);
        self.update_sounds(elapsed, time);
        gpu.queue.submit([encoder.finish()]);
        self.watch_gpu(&gpu);
        let submitted = Instant::now();

        let images = OutputImages {
            program: &self.program,
            screens: &self.screens,
        };
        for (_, output) in &mut self.outputs {
            output.present_images(&gpu, &images);
        }
        let steps = FrameSteps {
            playback_ms: milliseconds(played - started),
            upload_ms: milliseconds(uploaded - played),
            mix_ms: milliseconds(submitted - uploaded),
            present_ms: milliseconds(submitted.elapsed()),
        };
        self.update_stats(now, submitted - started, steps);
    }
}

impl EngineState {
    /// Records the drawing of the screens of the stage from the program,
    /// after creating, resizing or dropping their images to match it.
    fn draw_screens(&mut self, gpu: &Gpu, encoder: &mut wgpu::CommandEncoder) {
        let stage = &self.composition.stage;
        self.screens.retain(|(id, image)| {
            stage.screen(*id).is_some_and(|screen| {
                let (width, height) = screen.size();
                image.resolution() == Resolution { width, height }
            })
        });
        for screen in stage.screens() {
            if !self.screens.iter().any(|(id, _)| *id == screen.id()) {
                let (width, height) = screen.size();
                self.screens.push((
                    screen.id(),
                    ProgramOutput::new(&gpu.device, Resolution { width, height }),
                ));
            }
        }
        if self.screens.is_empty() {
            return;
        }
        let targets: Vec<ScreenTarget<'_>> = self
            .screens
            .iter()
            .filter_map(|(id, image)| {
                Some(ScreenTarget {
                    screen: stage.screen(*id)?,
                    view: image.view(),
                })
            })
            .collect();
        self.stage_renderer.encode(
            &gpu.device,
            &gpu.queue,
            encoder,
            &self.program.view,
            &targets,
        );
    }

    /// Number of screen images drawn after each frame.
    pub fn screen_count(&self) -> usize {
        self.screens.len()
    }
}

/// Image the layers are mixed into before the effects of the composition.
fn mix_texture(device: &wgpu::Device, resolution: Resolution) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("mix"),
            size: wgpu::Extent3d {
                width: resolution.width,
                height: resolution.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: WORKING_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
        .create_view(&wgpu::TextureViewDescriptor::default())
}

impl EngineState {
    /// Counts the frame just submitted as finished once the GPU has run it.
    fn watch_gpu(&mut self, gpu: &Gpu) {
        self.submitted += 1;
        let progress = Arc::clone(&self.gpu_progress);
        let submitted_at = Instant::now();
        let measured = self.stats.frames >= WARMUP_FRAMES;
        gpu.queue.on_submitted_work_done(move || {
            progress.completed.fetch_add(1, Ordering::Release);
            if measured {
                let latency = u64::try_from(submitted_at.elapsed().as_micros()).unwrap_or(u64::MAX);
                progress
                    .worst_latency_us
                    .fetch_max(latency, Ordering::Relaxed);
            }
        });
    }

    fn update_stats(&mut self, now: Instant, work: Duration, steps: FrameSteps) {
        let period_ms = 1000.0 / self.composition.frame_rate().as_f64();
        let stats = &mut self.stats;
        stats.frames += 1;
        if let Some(previous) = stats.previous.replace(now)
            && stats.frames > WARMUP_FRAMES
        {
            let interval_ms = now.saturating_duration_since(previous).as_secs_f64() * 1000.0;
            stats.worst_interval_ms = stats.worst_interval_ms.max(interval_ms);
            if interval_ms > period_ms * 1.5 {
                stats.hitches += 1;
                stats.last_hitch = Some(Hitch {
                    frame: stats.frames,
                    interval_ms,
                    previous: stats.last_steps,
                });
            }
        }
        stats.last_steps = steps;
        let work_ms = milliseconds(work);
        if stats.frames > WARMUP_FRAMES {
            stats.worst_work_ms = stats.worst_work_ms.max(work_ms);
            stats.worst_steps = stats.worst_steps.max(steps);
        }
        stats.frame_time_ms = if stats.frames == 1 {
            work_ms
        } else {
            stats.frame_time_ms * 0.95 + work_ms * 0.05
        };
        let window_start = *stats.window_start.get_or_insert(now);
        stats.window_frames += 1;
        let window = now.saturating_duration_since(window_start).as_secs_f64();
        if window >= 1.0 {
            stats.fps = f64::from(stats.window_frames - 1) / window;
            stats.window_start = Some(now);
            stats.window_frames = 1;
        }
    }

    fn note_late_tick(&mut self) {
        self.stats.late_ticks += 1;
    }

    fn note_real_time(&mut self, real_time: bool) {
        self.stats.real_time = real_time;
    }

    /// State of the engine after the last frame.
    pub fn status(&self) -> EngineStatus {
        EngineStatus {
            frames: self.stats.frames,
            fps: self.stats.fps,
            frame_time_ms: self.stats.frame_time_ms,
            late_ticks: self.stats.late_ticks,
            worst_interval_ms: self.stats.worst_interval_ms,
            hitches: self.stats.hitches,
            worst_frame_time_ms: self.stats.worst_work_ms,
            worst_steps: self.stats.worst_steps,
            last_hitch: self.stats.last_hitch,
            real_time: self.stats.real_time,
            gpu_skips: self.stats.gpu_skips,
            bpm: self.clock.bpm(),
            beat: self.beat,
            worst_gpu_latency_ms: self.gpu_progress.worst_latency_us.load(Ordering::Relaxed) as f64
                / 1000.0,
            layers: self.layers.iter().map(LayerRuntime::status).collect(),
        }
    }

    /// Frame rate of the composition, in frames per second.
    fn frame_rate(&self) -> f64 {
        self.composition.frame_rate().as_f64()
    }
}

fn milliseconds(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

struct Shared {
    status: Mutex<EngineStatus>,
    program: Mutex<Arc<ProgramOutput>>,
}

/// Handle on the render thread.
///
/// The interface sends commands and reads the published status and program
/// image; it never waits for the render thread, and the render thread never
/// waits for it.
pub struct Engine {
    commands: Sender<Command>,
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
    audio: Arc<AudioLevels>,
}

impl Engine {
    /// Starts the render thread on `composition`.
    pub fn start(gpu: Arc<Gpu>, composition: Composition) -> std::io::Result<Self> {
        let state = EngineState::new(gpu, composition);
        let audio = state.audio_levels();
        let shared = Arc::new(Shared {
            status: Mutex::new(state.status()),
            program: Mutex::new(state.program()),
        });
        let (commands, receiver) = mpsc::channel();
        let thread_shared = Arc::clone(&shared);
        let thread = thread::Builder::new()
            .name("prism-render".into())
            .spawn(move || run(state, receiver, thread_shared))?;
        Ok(Self {
            commands,
            shared,
            thread: Some(thread),
            audio,
        })
    }

    /// Levels of the sound that settings follow, see
    /// [`EngineState::audio_levels`].
    pub fn audio_levels(&self) -> Arc<AudioLevels> {
        Arc::clone(&self.audio)
    }

    /// Sends a command to the render thread.
    pub fn send(&self, command: Command) {
        // The render thread only stops when the engine is dropped.
        let _ = self.commands.send(command);
    }

    /// Plays `composition` from the next frame.
    pub fn set_composition(&self, composition: Composition) {
        self.send(Command::SetComposition(Box::new(composition)));
    }

    /// State after the last frame.
    pub fn status(&self) -> EngineStatus {
        self.shared
            .status
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Program image. Replaced when the composition resolution changes.
    pub fn program(&self) -> Arc<ProgramOutput> {
        Arc::clone(
            &self
                .shared
                .program
                .lock()
                .unwrap_or_else(PoisonError::into_inner),
        )
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        // Closing the channel stops the thread after its current frame.
        let (closed, _) = mpsc::channel();
        drop(std::mem::replace(&mut self.commands, closed));
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Render loop: one frame per period of the composition frame rate.
/// Commands are applied as soon as they arrive, between frames.
fn run(mut state: EngineState, commands: Receiver<Command>, shared: Arc<Shared>) {
    let mut deadline = Instant::now();
    let mut scheduled_period = Duration::ZERO;
    loop {
        // Wait for the next frame, applying commands meanwhile.
        loop {
            let now = Instant::now();
            if now >= deadline {
                break;
            }
            match commands.recv_timeout(deadline - now) {
                Ok(command) => state.apply(command),
                Err(RecvTimeoutError::Timeout) => break,
                Err(RecvTimeoutError::Disconnected) => return,
            }
        }
        loop {
            match commands.try_recv() {
                Ok(command) => state.apply(command),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return,
            }
        }

        let period = Duration::from_secs_f64(1.0 / state.frame_rate().max(1.0));
        if period != scheduled_period {
            scheduled_period = period;
            state.note_real_time(crate::priority::render_thread(period));
        }
        let now = Instant::now();
        if now > deadline + period {
            // Too far behind to catch up: start again from now.
            state.note_late_tick();
            deadline = now;
        }
        state.tick(now);
        deadline += period;

        if let Ok(mut status) = shared.status.try_lock() {
            *status = state.status();
        }
        if let Ok(mut program) = shared.program.try_lock()
            && !Arc::ptr_eq(&program, &state.program)
        {
            *program = state.program();
        }
    }
}
