//! Mixing the sound of the clips playing, in the audio callback.
//!
//! The render thread tells, at each frame, where each clip is and how fast
//! it moves; the mixer plays the sound from there at that rate, between two
//! frames, and corrects its drift gently, or jumps when the clip jumped.

use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};

use prism_media::ClipAudio;

/// What the render thread tells the mixer.
#[derive(Debug)]
pub enum VoiceCommand {
    /// A clip with sound starts; it stays silent until updated.
    Play {
        /// Number of the voice, chosen by the sender.
        id: u64,
        /// Its sound.
        audio: Arc<ClipAudio>,
    },
    /// Where a clip is now.
    Update {
        /// Number of the voice.
        id: u64,
        /// Position in the sound, in seconds.
        position: f64,
        /// Seconds of sound per second: the speed, negative backward, `0`
        /// when the clip does not move.
        rate: f64,
        /// Volume, from 0.
        gain: f32,
    },
    /// The clip stopped: its sound fades out.
    Stop {
        /// Number of the voice.
        id: u64,
    },
}

/// Farther than this from where the clip is, in seconds, the sound jumps
/// there; closer, it is pulled there.
const JUMP: f64 = 0.1;

/// Fraction of the drift corrected per second.
const PULL: f64 = 2.0;

/// Seconds for a volume change to be done for the most part: no clicks.
const RAMP: f32 = 0.005;

struct Voice {
    id: u64,
    audio: Arc<ClipAudio>,
    position: f64,
    rate: f64,
    correction: f64,
    gain: f32,
    target: f32,
    synced: bool,
    stopping: bool,
}

/// The voices of the clips, mixed into the output.
pub struct Mixer {
    commands: Receiver<VoiceCommand>,
    voices: Vec<Voice>,
}

impl Mixer {
    /// A mixer and the sender of its commands.
    pub fn new() -> (Self, Sender<VoiceCommand>) {
        let (sender, commands) = mpsc::channel();
        (
            Self {
                commands,
                voices: Vec::with_capacity(32),
            },
            sender,
        )
    }

    /// Number of voices playing or fading out.
    pub fn voice_count(&self) -> usize {
        self.voices.len()
    }

    fn apply(&mut self, command: VoiceCommand) {
        match command {
            VoiceCommand::Play { id, audio } => {
                self.voices.retain(|voice| voice.id != id);
                self.voices.push(Voice {
                    id,
                    audio,
                    position: 0.0,
                    rate: 0.0,
                    correction: 0.0,
                    gain: 0.0,
                    target: 0.0,
                    synced: false,
                    stopping: false,
                });
            }
            VoiceCommand::Update {
                id,
                position,
                rate,
                gain,
            } => {
                let Some(voice) = self.voices.iter_mut().find(|voice| voice.id == id) else {
                    return;
                };
                let finite = |value: f64| if value.is_finite() { value } else { 0.0 };
                let (position, rate) = (finite(position), finite(rate));
                let drift = position - voice.position;
                if !voice.synced || drift.abs() > JUMP {
                    voice.position = position;
                    voice.correction = 0.0;
                    voice.synced = true;
                } else {
                    voice.correction = drift * PULL;
                }
                voice.rate = rate;
                // A clip that does not move is silent.
                voice.target = if rate == 0.0 || !gain.is_finite() {
                    0.0
                } else {
                    gain.max(0.0)
                };
            }
            VoiceCommand::Stop { id } => {
                for voice in self.voices.iter_mut().filter(|voice| voice.id == id) {
                    voice.stopping = true;
                    voice.target = 0.0;
                }
            }
        }
    }

    /// Fills `output`, `channels` samples per frame at `sample_rate`, with
    /// the voices at `volume`. Stereo goes to the first two channels; a
    /// single channel gets both sides.
    pub fn render(&mut self, output: &mut [f32], channels: usize, sample_rate: u32, volume: f32) {
        while let Ok(command) = self.commands.try_recv() {
            self.apply(command);
        }
        output.fill(0.0);
        let channels = channels.max(1);
        let step = 1.0 / f64::from(sample_rate.max(1));
        let smoothing = 1.0 - (-1.0 / (RAMP * sample_rate.max(1) as f32)).exp();
        for voice in &mut self.voices {
            if voice.gain < 1e-5 && voice.target == 0.0 {
                voice.gain = 0.0;
                if voice.synced {
                    voice.position +=
                        (voice.rate + voice.correction) * step * (output.len() / channels) as f64;
                }
                continue;
            }
            for frame in output.chunks_mut(channels) {
                voice.gain += (voice.target - voice.gain) * smoothing;
                let [left, right] = voice.audio.sample_at(voice.position);
                let gain = voice.gain * volume;
                if channels == 1 {
                    frame[0] += 0.5 * (left + right) * gain;
                } else {
                    frame[0] += left * gain;
                    frame[1] += right * gain;
                }
                voice.position += (voice.rate + voice.correction) * step;
            }
        }
        self.voices
            .retain(|voice| !(voice.stopping && voice.gain < 1e-4));
        for sample in output.iter_mut() {
            *sample = sample.clamp(-1.0, 1.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use prism_media::AUDIO_RATE;

    /// A ramp of `frames` frames: left rises, right falls.
    fn ramp(frames: usize) -> Arc<ClipAudio> {
        let samples = (0..frames)
            .flat_map(|frame| {
                let value = (frame % 1000) as i16 * 16;
                [value, -value]
            })
            .collect();
        Arc::new(ClipAudio { samples })
    }

    fn render(mixer: &mut Mixer, frames: usize) -> Vec<f32> {
        let mut output = vec![0.0; frames * 2];
        mixer.render(&mut output, 2, AUDIO_RATE, 1.0);
        output
    }

    #[test]
    fn voices_play_from_where_the_clip_is() {
        let (mut mixer, sender) = Mixer::new();
        let audio = ramp(48_000);
        sender
            .send(VoiceCommand::Play {
                id: 1,
                audio: Arc::clone(&audio),
            })
            .unwrap();
        // Silent until told where the clip is.
        assert!(render(&mut mixer, 64).iter().all(|sample| *sample == 0.0));
        let position = 100.0 / f64::from(AUDIO_RATE);
        sender
            .send(VoiceCommand::Update {
                id: 1,
                position,
                rate: 1.0,
                gain: 1.0,
            })
            .unwrap();
        // Once the volume is up, sample after sample from there.
        let output = render(&mut mixer, 2000);
        let expected = |frame: usize| audio.sample_at((100 + frame) as f64 / f64::from(AUDIO_RATE));
        for frame in [1500, 1999] {
            let [left, right] = expected(frame);
            assert!((output[frame * 2] - left).abs() < 1e-3, "{frame}");
            assert!((output[frame * 2 + 1] - right).abs() < 1e-3, "{frame}");
        }
    }

    #[test]
    fn far_positions_jump_near_ones_are_pulled_paused_clips_are_silent() {
        let (mut mixer, sender) = Mixer::new();
        sender
            .send(VoiceCommand::Play {
                id: 7,
                audio: ramp(96_000),
            })
            .unwrap();
        let update = |position: f64, rate: f64| VoiceCommand::Update {
            id: 7,
            position,
            rate,
            gain: 1.0,
        };
        sender.send(update(0.5, 1.0)).unwrap();
        render(&mut mixer, 480);
        assert!((mixer.voices[0].position - 0.51).abs() < 1e-9);
        // Close: pulled, not moved.
        sender.send(update(0.52, 1.0)).unwrap();
        render(&mut mixer, 0);
        assert!((mixer.voices[0].position - 0.51).abs() < 1e-9);
        assert!(mixer.voices[0].correction > 0.0);
        // Far (a loop, a cue point): jumps.
        sender.send(update(1.5, -1.0)).unwrap();
        render(&mut mixer, 480);
        assert!((mixer.voices[0].position - 1.49).abs() < 1e-9);
        // Paused: fades to silence.
        sender.send(update(1.0, 0.0)).unwrap();
        let output = render(&mut mixer, 4800);
        assert!(
            output[output.len() - 2..]
                .iter()
                .all(|sample| sample.abs() < 1e-4)
        );
    }

    #[test]
    fn stopped_voices_fade_out_then_leave() {
        let (mut mixer, sender) = Mixer::new();
        sender
            .send(VoiceCommand::Play {
                id: 3,
                audio: ramp(96_000),
            })
            .unwrap();
        sender
            .send(VoiceCommand::Update {
                id: 3,
                position: 0.0,
                rate: 1.0,
                gain: 0.5,
            })
            .unwrap();
        render(&mut mixer, 4800);
        sender.send(VoiceCommand::Stop { id: 3 }).unwrap();
        let output = render(&mut mixer, 48);
        // No click: the first frames are still near the level before.
        assert!(output[2].abs() > 0.0);
        render(&mut mixer, 4800);
        assert_eq!(mixer.voice_count(), 0);
        // Mono output: both sides.
        let mut mono = vec![0.0; 16];
        mixer.render(&mut mono, 1, AUDIO_RATE, 1.0);
        assert!(mono.iter().all(|sample| *sample == 0.0));
    }
}
