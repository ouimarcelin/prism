//! The sound of a clip playing: decoded on a thread of its own, then told
//! to the mixer frame after frame.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use prism_audio::VoiceCommand;
use prism_media::ClipAudio;

enum State {
    /// Decoding.
    Loading(Receiver<Option<Arc<ClipAudio>>>),
    /// Given to the mixer.
    Playing,
    /// No sound, or it could not be decoded.
    Silent,
}

/// Sound of one clip stream. Its voice fades out when it is dropped.
pub(crate) struct ClipSound {
    id: u64,
    mixer: Sender<VoiceCommand>,
    state: State,
}

impl ClipSound {
    /// Starts decoding the sound of the file at `path`, for voice `id`.
    pub(crate) fn start(path: PathBuf, id: u64, mixer: Sender<VoiceCommand>) -> Self {
        let (sender, receiver) = mpsc::channel();
        let spawned = thread::Builder::new()
            .name("prism-sound".into())
            .spawn(move || {
                let audio = prism_media::decode_audio(&path)
                    .ok()
                    .flatten()
                    .filter(|audio| audio.frames() > 0)
                    .map(Arc::new);
                let _ = sender.send(audio);
            });
        Self {
            id,
            mixer,
            state: if spawned.is_ok() {
                State::Loading(receiver)
            } else {
                State::Silent
            },
        }
    }

    /// Tells the mixer where the clip is: `position` in seconds, moving at
    /// `rate`, at volume `gain`. Before the sound is decoded, nothing.
    pub(crate) fn update(&mut self, position: f64, rate: f64, gain: f32) {
        if let State::Loading(receiver) = &self.state {
            match receiver.try_recv() {
                Ok(Some(audio)) => {
                    let _ = self.mixer.send(VoiceCommand::Play { id: self.id, audio });
                    self.state = State::Playing;
                }
                Ok(None) | Err(mpsc::TryRecvError::Disconnected) => self.state = State::Silent,
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if matches!(self.state, State::Playing) {
            let _ = self.mixer.send(VoiceCommand::Update {
                id: self.id,
                position,
                rate,
                gain,
            });
        }
    }
}

impl Drop for ClipSound {
    fn drop(&mut self) {
        if matches!(self.state, State::Playing) {
            let _ = self.mixer.send(VoiceCommand::Stop { id: self.id });
        }
    }
}
