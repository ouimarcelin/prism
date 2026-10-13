//! A clip playing on a layer: its decoding worker, frame cache, playhead and
//! layer texture.

use std::path::PathBuf;

use prism_core::{Playback, Slot};
use prism_media::{HapFrame, VideoTrack};
use prism_render::{FrameUploader, Gpu, VideoTexture, wgpu};

use crate::playhead::{MediaTiming, Playhead};
use crate::random::Random;
use crate::status::ClipState;
use crate::worker::{Event, Worker, error_chain};

/// Ticks of playback decoded ahead of the playhead.
const LOOKAHEAD_TICKS: usize = 6;

/// Largest number of decoded frames kept waiting for upload.
const CACHE_CAPACITY: usize = 8;

enum Phase {
    Opening,
    Ready {
        track: VideoTrack,
        timing: MediaTiming,
        playhead: Playhead,
    },
    Failed(String),
}

pub(crate) struct ClipStream {
    slot: Slot,
    source: PathBuf,
    playback: Playback,
    worker: Option<Worker>,
    phase: Phase,
    /// Decoded frames waiting for upload.
    cache: Vec<(usize, HapFrame)>,
    /// Frames last asked of the worker.
    requested: Vec<usize>,
    /// Frame wanted on screen now.
    wanted: Option<usize>,
    /// Frame currently in the texture.
    shown: Option<usize>,
    texture: Option<VideoTexture>,
    late_frames: u64,
    frame_errors: u64,
    pending_seek: Option<(f64, bool)>,
}

impl ClipStream {
    /// Starts decoding `source` for the clip in `slot`.
    pub(crate) fn open(slot: Slot, source: PathBuf, playback: Playback) -> Self {
        let (worker, phase) = match Worker::spawn(source.clone()) {
            Ok(worker) => (Some(worker), Phase::Opening),
            Err(error) => (None, Phase::Failed(error_chain(&error))),
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
            late_frames: 0,
            frame_errors: 0,
            pending_seek: None,
        }
    }

    pub(crate) fn slot(&self) -> Slot {
        self.slot
    }

    pub(crate) fn plays(&self, slot: Slot, source: &std::path::Path) -> bool {
        self.slot == slot && self.source == source
    }

    /// Whether a frame is on screen.
    pub(crate) fn has_frame(&self) -> bool {
        self.shown.is_some()
    }

    pub(crate) fn has_failed(&self) -> bool {
        matches!(self.phase, Phase::Failed(_))
    }

    /// Layer texture, once a frame is uploaded.
    pub(crate) fn view(&self) -> Option<&wgpu::TextureView> {
        self.shown
            .and(self.texture.as_ref())
            .map(VideoTexture::view)
    }

    /// Updates the playback settings, from the composition.
    pub(crate) fn set_playback(&mut self, playback: &Playback) {
        if &self.playback != playback {
            self.playback = playback.clone();
        }
    }

    /// Restarts the clip from the beginning of its played range.
    pub(crate) fn restart(&mut self) {
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
            Phase::Failed(_) => {}
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
            Phase::Failed(_) => {}
        }
    }

    /// Takes the events sent by the worker.
    pub(crate) fn receive(&mut self) {
        let Some(worker) = &self.worker else {
            return;
        };
        while let Some(event) = worker.try_event() {
            match event {
                Event::Opened(track) => {
                    let timing = MediaTiming::from_track(&track);
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
                        worker.recycle(frame);
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
                worker.recycle(self.cache.swap_remove(index).1);
            }
        }

        // Ask only for what is neither shown nor cached.
        let missing: Vec<usize> = frames
            .into_iter()
            .filter(|frame| {
                Some(*frame) != self.shown && !self.cache.iter().any(|(i, _)| i == frame)
            })
            .collect();
        if missing != self.requested {
            worker.want(missing.clone());
            self.requested = missing;
        }
    }

    /// Records the upload of the wanted frame when it is decoded. Keeps the
    /// previous frame on screen, and counts a late frame, when it is not.
    pub(crate) fn upload(
        &mut self,
        gpu: &Gpu,
        uploader: &FrameUploader,
        encoder: &mut wgpu::CommandEncoder,
    ) {
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
            worker.recycle(frame);
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
