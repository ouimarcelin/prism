//! The render thread: plays the clips of a composition, mixes them and keeps
//! the program image up to date, at the composition frame rate.

use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use prism_core::{Composition, LayerId, Resolution, Slot};
use prism_render::{
    Compositor, FrameUploader, Gpu, LayerInput, Transition, WORKING_FORMAT, solid_color_texture,
    wgpu,
};

use crate::random::Random;
use crate::status::{EngineStatus, LayerStatus};
use crate::stream::ClipStream;

/// A destination of the program image: a window, later a network stream.
///
/// The render thread calls [`Output::present`] after every frame, so it must
/// not wait long: skip the frame rather than block when the destination is
/// not ready.
pub trait Output: Send {
    /// Shows `program`.
    fn present(&mut self, gpu: &Gpu, program: &ProgramOutput);
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
        }
    }
}

/// The mixed image of the composition, refreshed after every frame.
///
/// Readers sample it at any time: it always holds a complete frame.
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
            usage: wgpu::TextureUsages::COPY_DST
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

/// Frame-rate statistics over a sliding second.
struct Stats {
    frames: u64,
    late_ticks: u64,
    window_start: Option<Instant>,
    window_frames: u32,
    fps: f64,
    frame_time_ms: f64,
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
    outputs: Vec<(u64, Box<dyn Output>)>,
    random: Random,
    last_tick: Option<Instant>,
    stats: Stats,
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
            program,
            outputs: Vec::new(),
            random: Random::new(seed),
            last_tick: None,
            stats: Stats {
                frames: 0,
                late_ticks: 0,
                window_start: None,
                window_frames: 0,
                fps: 0.0,
                frame_time_ms: 0.0,
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

    /// Plays `composition` from the next frame.
    pub fn set_composition(&mut self, composition: Composition) {
        let resolution = composition.resolution();
        if resolution != self.program.resolution() {
            self.compositor.resize(&self.gpu.device, resolution);
            self.program = Arc::new(ProgramOutput::new(&self.gpu.device, resolution));
        }
        self.composition = composition;
        self.sync_layers();
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

            let wanted = composition.active_clip(id).and_then(|(column, clip)| {
                clip.source
                    .path()
                    .map(|path| (Slot::new(id, column), path.to_path_buf(), &clip.playback))
            });
            match wanted {
                Some((slot, path, playback)) => match runtime.current.as_mut() {
                    Some(stream) if stream.plays(slot, &path) => {
                        stream.set_playback(playback);
                        if retriggered {
                            stream.restart();
                        }
                    }
                    _ => {
                        let stream = ClipStream::open(slot, path, playback.clone());
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

        // GPU: frame uploads, mixing and copy to the program image, in one
        // submission.
        let gpu = Arc::clone(&self.gpu);
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("engine frame"),
            });
        for runtime in &mut self.layers {
            for stream in runtime.streams_mut() {
                stream.upload(&gpu, &self.uploader, &mut encoder);
            }
        }

        let mut inputs = Vec::with_capacity(self.layers.len());
        for runtime in &self.layers {
            let Some(opacity) = self.composition.effective_opacity(runtime.id) else {
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
        self.compositor
            .encode(&gpu.device, &gpu.queue, &mut encoder, &inputs);
        let resolution = self.program.resolution();
        encoder.copy_texture_to_texture(
            self.compositor.output().as_image_copy(),
            self.program.texture.as_image_copy(),
            wgpu::Extent3d {
                width: resolution.width,
                height: resolution.height,
                depth_or_array_layers: 1,
            },
        );
        gpu.queue.submit([encoder.finish()]);
        let work = started.elapsed();

        for (_, output) in &mut self.outputs {
            output.present(&gpu, &self.program);
        }
        self.update_stats(now, work);
    }

    fn update_stats(&mut self, now: Instant, work: Duration) {
        let stats = &mut self.stats;
        stats.frames += 1;
        let work_ms = work.as_secs_f64() * 1000.0;
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

    /// State of the engine after the last frame.
    pub fn status(&self) -> EngineStatus {
        EngineStatus {
            frames: self.stats.frames,
            fps: self.stats.fps,
            frame_time_ms: self.stats.frame_time_ms,
            late_ticks: self.stats.late_ticks,
            layers: self.layers.iter().map(LayerRuntime::status).collect(),
        }
    }

    /// Frame rate of the composition, in frames per second.
    fn frame_rate(&self) -> f64 {
        self.composition.frame_rate().as_f64()
    }
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
}

impl Engine {
    /// Starts the render thread on `composition`.
    pub fn start(gpu: Arc<Gpu>, composition: Composition) -> std::io::Result<Self> {
        let state = EngineState::new(gpu, composition);
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
        })
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
