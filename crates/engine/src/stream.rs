//! A clip playing on a layer: its decoding worker, frame cache, playhead and
//! layer texture.

use prism_core::{Effect, MediaSource, Playback, Slot};
use prism_fx::{EffectChain, EffectRenderer, FrameTime};
use prism_media::{Frame, MediaInfo};
use prism_render::{FrameUploader, Gpu, VideoTexture, wgpu};

use crate::live::Live;
use crate::playhead::{MediaTiming, Playhead};
use crate::random::Random;
use crate::sound::ClipSound;
use crate::status::ClipState;
use crate::worker::{Event, Worker, error_chain};

/// Ticks of playback decoded ahead of the playhead.
const LOOKAHEAD_TICKS: usize = 6;

/// Largest number of decoded frames kept waiting for upload.
const CACHE_CAPACITY: usize = 8;

enum Phase {
    Opening,
    Ready {
        track: MediaInfo,
        timing: MediaTiming,
        playhead: Playhead,
    },
    Failed(String),
    /// Drawn by a generator on the render thread, frame after frame.
    Generating,
    /// Read live from a camera.
    Live(Live),
    /// Read live from another application over Syphon.
    #[cfg(target_os = "macos")]
    Syphon(SyphonInput),
    /// Read live from another application over Spout.
    #[cfg(target_os = "windows")]
    Spout {
        name: String,
        receiver: prism_io_video::SpoutReceiver,
    },
    /// Read live from a source of the network over NDI.
    #[cfg(feature = "ndi")]
    Ndi {
        name: String,
        receiver: prism_io_video::NdiReceiver,
    },
}

/// A Syphon server to read, connected when it is published.
#[cfg(target_os = "macos")]
struct SyphonInput {
    app: String,
    name: String,
    client: Option<prism_io_video::SyphonClient>,
    /// Last attempt to connect: retried every second.
    tried: Option<std::time::Instant>,
}

pub(crate) struct ClipStream {
    slot: Slot,
    source: MediaSource,
    playback: Playback,
    worker: Option<Worker>,
    phase: Phase,
    /// Decoded frames waiting for upload.
    cache: Vec<(usize, Frame)>,
    /// Frames last asked of the worker.
    requested: Vec<usize>,
    /// Frame wanted on screen now.
    wanted: Option<usize>,
    /// Frame currently in the texture.
    shown: Option<usize>,
    texture: Option<VideoTexture>,
    effects: EffectChain,
    late_frames: u64,
    frame_errors: u64,
    pending_seek: Option<(f64, bool)>,
    /// Whether a timecode holds the playhead.
    chasing: bool,
    /// Beat position and seconds of the engine when the clip was launched.
    launched: (f64, f64),
    sound: Option<ClipSound>,
    /// Position at the last sound update, to follow scratching.
    sound_position: Option<f64>,
}

impl ClipStream {
    /// Starts decoding `source` for the clip in `slot`, launched at
    /// `launched` (beat position and seconds of the engine).
    pub(crate) fn open(
        slot: Slot,
        source: MediaSource,
        playback: Playback,
        launched: (f64, f64),
    ) -> Self {
        let (worker, phase) = if source.generator().is_some() {
            (None, Phase::Generating)
        } else if let MediaSource::Camera { id, .. } = &source {
            (None, Phase::Live(Live::open(id.clone())))
        } else if let MediaSource::Syphon { app, name } = &source {
            #[cfg(target_os = "macos")]
            let phase = Phase::Syphon(SyphonInput {
                app: app.clone(),
                name: name.clone(),
                client: None,
                tried: None,
            });
            #[cfg(not(target_os = "macos"))]
            let phase = Phase::Failed(format!("Syphon ({app} – {name}) : macOS seulement"));
            (None, phase)
        } else if let MediaSource::Spout { name } = &source {
            #[cfg(target_os = "windows")]
            let phase = Phase::Spout {
                name: name.clone(),
                receiver: prism_io_video::SpoutReceiver::connect(name),
            };
            #[cfg(not(target_os = "windows"))]
            let phase = Phase::Failed(format!("Spout ({name}) : Windows seulement"));
            (None, phase)
        } else if let MediaSource::Ndi { name } = &source {
            #[cfg(feature = "ndi")]
            let phase = match prism_io_video::NdiReceiver::connect(name) {
                Ok(receiver) => Phase::Ndi {
                    name: name.clone(),
                    receiver,
                },
                Err(error) => Phase::Failed(format!("NDI ({name}) : {error}")),
            };
            #[cfg(not(feature = "ndi"))]
            let phase = Phase::Failed(format!(
                "NDI ({name}) : Prism compilé sans NDI (option « ndi »)"
            ));
            (None, phase)
        } else {
            match Worker::spawn(source.clone()) {
                Ok(worker) => (Some(worker), Phase::Opening),
                Err(error) => (None, Phase::Failed(error_chain(&error))),
            }
        };
        Self {
            slot,
            source,
            playback,
            worker,
            phase,
            cache: Vec::new(),
            requested: Vec::new(),
            wanted: None,
            shown: None,
            texture: None,
            effects: EffectChain::default(),
            late_frames: 0,
            frame_errors: 0,
            pending_seek: None,
            chasing: false,
            launched,
            sound: None,
            sound_position: None,
        }
    }

    /// Plays the sound of the clip's file through `mixer`, as voice `id`.
    /// Generated clips and clips already sounding are left as they are.
    pub(crate) fn enable_sound(
        &mut self,
        id: u64,
        mixer: &std::sync::mpsc::Sender<prism_audio::VoiceCommand>,
    ) {
        if self.sound.is_some() || self.is_generator() {
            return;
        }
        if let Some(path) = self.source.path() {
            self.sound = Some(ClipSound::start(path.to_path_buf(), id, mixer.clone()));
        }
    }

    /// Stops the sound of the clip.
    pub(crate) fn disable_sound(&mut self) {
        self.sound = None;
        self.sound_position = None;
    }

    /// Tells the mixer where the clip is after `elapsed` seconds, at volume
    /// `gain`. Paused, finished and random clips are silent; a held clip
    /// (scratching) sounds at the speed it is moved.
    pub(crate) fn update_sound(&mut self, elapsed: f64, gain: f32) {
        let Some(sound) = &mut self.sound else {
            return;
        };
        let Phase::Ready { playhead, .. } = &self.phase else {
            return;
        };
        let position = playhead.position();
        let previous = self.sound_position.replace(position);
        let rate = if playhead.is_held() {
            match previous {
                Some(previous) if elapsed > 0.0 => {
                    ((position - previous) / elapsed).clamp(-4.0, 4.0)
                }
                _ => 0.0,
            }
        } else if playhead.is_finished() {
            0.0
        } else {
            match self.playback.direction {
                prism_core::Direction::Forward | prism_core::Direction::Backward => {
                    playhead.travel() * f64::from(self.playback.effective_speed())
                }
                prism_core::Direction::Paused | prism_core::Direction::Random => 0.0,
            }
        };
        sound.update(position, rate, gain);
    }

    /// Beat position and seconds of the engine at the last launch.
    pub(crate) fn launched(&self) -> (f64, f64) {
        self.launched
    }

    pub(crate) fn slot(&self) -> Slot {
        self.slot
    }

    /// Whether the stream plays this source in this slot. A generator whose
    /// settings changed still counts as the same: its settings are read at
    /// every frame.
    pub(crate) fn plays(&self, slot: Slot, source: &MediaSource) -> bool {
        self.slot == slot && self.source.same_media(source)
    }

    /// Whether the clip is drawn by a generator.
    pub(crate) fn is_generator(&self) -> bool {
        matches!(self.phase, Phase::Generating)
    }

    /// Whether a frame is on screen.
    pub(crate) fn has_frame(&self) -> bool {
        self.shown.is_some() || (self.is_generator() && self.effects.output().is_some())
    }

    pub(crate) fn has_failed(&self) -> bool {
        matches!(self.phase, Phase::Failed(_))
    }

    /// Image of the clip, once a frame is uploaded: the layer texture after
    /// the effects of the clip and of its layer.
    pub(crate) fn view(&self) -> Option<&wgpu::TextureView> {
        if self.is_generator() {
            return self.effects.output();
        }
        let (view, _) = self.source_image()?;
        Some(self.effects.output().unwrap_or(view))
    }

    /// The image of the clip before its effects, and its size.
    fn source_image(&self) -> Option<(&wgpu::TextureView, (u32, u32))> {
        self.shown?;
        #[cfg(target_os = "macos")]
        if let Phase::Syphon(input) = &self.phase {
            let client = input.client.as_ref()?;
            let texture = client.texture()?;
            return Some((client.view()?, (texture.width(), texture.height())));
        }
        let texture = self.texture.as_ref()?;
        Some((texture.view(), (texture.width(), texture.height())))
    }

    /// Records the image of a generated clip: `effects`, the generator first,
    /// drawn at `size` pixels. `blank` stands for the image the generator
    /// does not read.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn generate(
        &mut self,
        renderer: &EffectRenderer,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        effects: &[&Effect],
        time: FrameTime,
        blank: &wgpu::TextureView,
        size: (u32, u32),
    ) {
        self.effects.apply(
            renderer,
            &gpu.device,
            &gpu.queue,
            encoder,
            effects,
            blank,
            size,
            time,
        );
    }

    /// Records `effects` applied to the frame on screen.
    pub(crate) fn apply_effects(
        &mut self,
        renderer: &EffectRenderer,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        effects: &[&Effect],
        time: FrameTime,
    ) {
        let Some((view, size)) = self.source_image() else {
            return;
        };
        // The chain keeps its own images: the borrow of the source ends
        // with this call.
        let view = view.clone();
        self.effects.apply(
            renderer,
            &gpu.device,
            &gpu.queue,
            encoder,
            effects,
            &view,
            size,
            time,
        );
    }

    /// Updates the playback settings, from the composition.
    pub(crate) fn set_playback(&mut self, playback: &Playback) {
        if &self.playback != playback {
            self.playback = playback.clone();
        }
    }

    /// Restarts the clip from the beginning of its played range.
    pub(crate) fn restart(&mut self, launched: (f64, f64)) {
        self.launched = launched;
        if let Phase::Ready {
            timing, playhead, ..
        } = &mut self.phase
        {
            *playhead = Playhead::new(&self.playback, *timing);
        }
    }

    /// Moves the playhead; `hold` keeps it there until [`Self::release`].
    pub(crate) fn seek(&mut self, position: f64, hold: bool) {
        match &mut self.phase {
            Phase::Ready {
                timing, playhead, ..
            } => {
                playhead.seek(position, &self.playback, *timing);
                playhead.set_held(hold);
            }
            Phase::Opening => self.pending_seek = Some((position, hold)),
            _ => {}
        }
    }

    /// Puts the playhead where a timecode says, and holds it there until
    /// the next frame.
    pub(crate) fn chase(&mut self, position: f64) {
        self.chasing = true;
        self.seek(position, true);
    }

    /// Lets a playhead held by a timecode move again.
    pub(crate) fn stop_chasing(&mut self) {
        if std::mem::take(&mut self.chasing) {
            self.release();
        }
    }

    /// Lets a held playhead move again.
    pub(crate) fn release(&mut self) {
        match &mut self.phase {
            Phase::Ready { playhead, .. } => playhead.set_held(false),
            Phase::Opening => {
                if let Some((_, hold)) = &mut self.pending_seek {
                    *hold = false;
                }
            }
            _ => {}
        }
    }

    /// Takes the events sent by the worker.
    pub(crate) fn receive(&mut self) {
        if let Phase::Live(live) = &mut self.phase {
            live.poll();
            if let Some(message) = live.error() {
                self.phase = Phase::Failed(message.to_owned());
            }
            return;
        }
        let Some(worker) = &self.worker else {
            return;
        };
        while let Some(event) = worker.try_event() {
            match event {
                Event::Opened(track) => {
                    let timing = MediaTiming::from_info(&track);
                    let mut playhead = Playhead::new(&self.playback, timing);
                    if let Some((position, hold)) = self.pending_seek.take() {
                        playhead.seek(position, &self.playback, timing);
                        playhead.set_held(hold);
                    }
                    self.phase = Phase::Ready {
                        track,
                        timing,
                        playhead,
                    };
                }
                Event::OpenFailed(message) => self.phase = Phase::Failed(message),
                Event::Frame(index, frame) => {
                    if self.requested.contains(&index) && self.cache.len() < CACHE_CAPACITY {
                        self.cache.push((index, frame));
                    } else {
                        worker.recycle(index, frame);
                    }
                }
                Event::FrameFailed(index, message) => {
                    self.frame_errors += 1;
                    if self.shown.is_none() {
                        self.phase = Phase::Failed(format!("frame {index}: {message}"));
                    }
                }
            }
        }
    }

    /// Advances the playhead by `elapsed` seconds and asks the worker for the
    /// frames needed over the next ticks of `tick` seconds.
    pub(crate) fn advance(&mut self, elapsed: f64, tick: f64, random: &mut Random) {
        let Phase::Ready {
            track,
            timing,
            playhead,
        } = &mut self.phase
        else {
            return;
        };
        playhead.advance(&self.playback, *timing, elapsed, random);

        let current = track.frame_at(playhead.position());
        let mut frames = vec![current];
        for position in playhead.predict(&self.playback, *timing, tick, LOOKAHEAD_TICKS) {
            let frame = track.frame_at(position);
            if !frames.contains(&frame) {
                frames.push(frame);
            }
        }
        self.wanted = Some(current);

        // Free cached frames that are no longer on the way.
        let Some(worker) = &self.worker else {
            return;
        };
        let mut index = 0;
        while index < self.cache.len() {
            if frames.contains(&self.cache[index].0) {
                index += 1;
            } else {
                let (frame_index, frame) = self.cache.swap_remove(index);
                worker.recycle(frame_index, frame);
            }
        }

        // Ask for what is neither shown nor cached, again while something is
        // missing: the worker skips frames already on their way.
        let missing: Vec<usize> = frames
            .into_iter()
            .filter(|frame| {
                Some(*frame) != self.shown && !self.cache.iter().any(|(i, _)| i == frame)
            })
            .collect();
        if missing != self.requested || !missing.is_empty() {
            worker.want(missing.clone());
            self.requested = missing;
        }
    }

    /// Records the upload of the wanted frame when it is decoded. Keeps the
    /// previous frame on screen, and counts a late frame, when it is not.
    pub(crate) fn upload(
        &mut self,
        gpu: &Gpu,
        uploader: &mut FrameUploader,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        #[cfg(target_os = "macos")]
        if let Phase::Syphon(input) = &mut self.phase {
            if input
                .client
                .as_ref()
                .is_some_and(|client| !client.is_valid())
            {
                input.client = None;
                self.shown = None;
            }
            if input.client.is_none()
                && input
                    .tried
                    .is_none_or(|tried| tried.elapsed() >= std::time::Duration::from_secs(1))
            {
                input.tried = Some(std::time::Instant::now());
                input.client = prism_io_video::SyphonClient::connect(gpu, &input.app, &input.name);
            }
            if let Some(client) = &mut input.client
                && client.update(gpu)
            {
                self.shown = Some(self.shown.map_or(0, |count| count.wrapping_add(1)));
            }
            return;
        }
        #[cfg(feature = "ndi")]
        if let Phase::Ndi { receiver, .. } = &self.phase {
            let Some(received) = receiver.take_frame() else {
                return;
            };
            let (width, height) = (received.width, received.height);
            if width == 0 || height == 0 {
                receiver.recycle(received);
                return;
            }
            let frame = Frame::Rgba(prism_media::RgbaImage {
                width,
                height,
                order: prism_media::ChannelOrder::Bgra,
                pixels: received.pixels,
            });
            if self
                .texture
                .as_ref()
                .is_none_or(|texture| (texture.width(), texture.height()) != (width, height))
            {
                self.texture = Some(VideoTexture::new(&gpu.device, width, height));
            }
            if let Some(texture) = &mut self.texture
                && uploader
                    .upload(&gpu.device, &gpu.queue, encoder, texture, &frame)
                    .is_ok()
            {
                self.shown = Some(self.shown.map_or(0, |count| count.wrapping_add(1)));
            }
            if let Frame::Rgba(image) = frame {
                receiver.recycle(prism_io_video::NdiFrame {
                    width,
                    height,
                    pixels: image.pixels,
                });
            }
            return;
        }
        #[cfg(target_os = "windows")]
        if let Phase::Spout { receiver, .. } = &self.phase {
            let Some(received) = receiver.take_frame() else {
                return;
            };
            let (width, height) = (received.width, received.height);
            if width == 0 || height == 0 {
                return;
            }
            let frame = Frame::Rgba(prism_media::RgbaImage {
                width,
                height,
                order: prism_media::ChannelOrder::Bgra,
                pixels: received.pixels,
            });
            if self
                .texture
                .as_ref()
                .is_none_or(|texture| (texture.width(), texture.height()) != (width, height))
            {
                self.texture = Some(VideoTexture::new(&gpu.device, width, height));
            }
            if let Some(texture) = &mut self.texture
                && uploader
                    .upload(&gpu.device, &gpu.queue, encoder, texture, &frame)
                    .is_ok()
            {
                self.shown = Some(self.shown.map_or(0, |count| count.wrapping_add(1)));
            }
            return;
        }
        if let Phase::Live(live) = &self.phase {
            let Some(frame) = live.take_frame() else {
                return;
            };
            let (width, height) = frame_size(&frame);
            if width == 0 || height == 0 {
                live.recycle(frame);
                return;
            }
            if self
                .texture
                .as_ref()
                .is_none_or(|texture| (texture.width(), texture.height()) != (width, height))
            {
                self.texture = Some(VideoTexture::new(&gpu.device, width, height));
            }
            if let Some(texture) = &mut self.texture
                && uploader
                    .upload(&gpu.device, &gpu.queue, encoder, texture, &frame)
                    .is_ok()
            {
                self.shown = Some(self.shown.map_or(0, |count| count.wrapping_add(1)));
            }
            live.recycle(frame);
            return;
        }
        let (Some(wanted), Phase::Ready { track, .. }) = (self.wanted, &self.phase) else {
            return;
        };
        if self.shown == Some(wanted) {
            return;
        }
        let Some(position) = self.cache.iter().position(|(index, _)| *index == wanted) else {
            if self.shown.is_some() {
                self.late_frames += 1;
            }
            return;
        };
        let (index, frame) = self.cache.swap_remove(position);
        let texture = self
            .texture
            .get_or_insert_with(|| VideoTexture::new(&gpu.device, track.width, track.height));
        match uploader.upload(&gpu.device, &gpu.queue, encoder, texture, &frame) {
            Ok(()) => self.shown = Some(index),
            Err(error) => self.phase = Phase::Failed(error_chain(&error)),
        }
        if let Some(worker) = &self.worker {
            worker.recycle(index, frame);
        }
    }

    /// Playback state, for the interface.
    pub(crate) fn state(&self) -> ClipState {
        match &self.phase {
            Phase::Opening => ClipState::Loading,
            Phase::Failed(message) => ClipState::Error(message.clone()),
            Phase::Ready { playhead, .. } if playhead.is_finished() => ClipState::Finished,
            Phase::Ready { .. } if self.shown.is_none() => ClipState::Loading,
            Phase::Ready { .. } => ClipState::Playing,
            Phase::Generating if self.has_frame() => ClipState::Playing,
            Phase::Generating => ClipState::Loading,
            Phase::Live(_) if self.shown.is_some() => ClipState::Playing,
            Phase::Live(_) => ClipState::Loading,
            #[cfg(target_os = "macos")]
            Phase::Syphon(input) if input.client.is_none() => ClipState::Error(format!(
                "serveur Syphon « {} » de {} absent : en attente",
                input.name, input.app
            )),
            #[cfg(target_os = "macos")]
            Phase::Syphon(_) if self.shown.is_some() => ClipState::Playing,
            #[cfg(target_os = "macos")]
            Phase::Syphon(_) => ClipState::Loading,
            #[cfg(feature = "ndi")]
            Phase::Ndi { receiver, .. } if self.shown.is_some() && receiver.is_connected() => {
                ClipState::Playing
            }
            #[cfg(feature = "ndi")]
            Phase::Ndi { name, .. } => {
                ClipState::Error(format!("source NDI « {name} » absente : en attente"))
            }
            #[cfg(target_os = "windows")]
            Phase::Spout { receiver, .. } if self.shown.is_some() && receiver.is_connected() => {
                ClipState::Playing
            }
            #[cfg(target_os = "windows")]
            Phase::Spout { name, .. } => {
                ClipState::Error(format!("émetteur Spout « {name} » absent : en attente"))
            }
        }
    }

    /// Position in seconds and media duration, once the file is open.
    pub(crate) fn timing(&self) -> Option<(f64, f64, usize)> {
        match &self.phase {
            Phase::Ready {
                track,
                timing,
                playhead,
            } => Some((playhead.position(), timing.duration, track.frame_count())),
            _ => None,
        }
    }

    pub(crate) fn shown_frame(&self) -> Option<usize> {
        self.shown
    }

    pub(crate) fn late_frames(&self) -> u64 {
        self.late_frames
    }

    pub(crate) fn frame_errors(&self) -> u64 {
        self.frame_errors
    }

    pub(crate) fn is_held(&self) -> bool {
        matches!(&self.phase, Phase::Ready { playhead, .. } if playhead.is_held())
    }
}

/// Size of a live frame in pixels (cameras deliver pixels, not HAP).
fn frame_size(frame: &Frame) -> (u32, u32) {
    match frame {
        Frame::Hap(_) => (0, 0),
        Frame::Rgba(image) => (image.width, image.height),
        Frame::Yuv(image) => (image.width, image.height),
    }
}
