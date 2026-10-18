//! Sound of media files, decoded whole into memory to be mixed while the
//! clip plays.

use std::path::Path;

use crate::decoder::DecodeError;

/// Sample rate sound is decoded at, whatever the file's.
pub const AUDIO_RATE: u32 = 48_000;

/// Longest sound decoded, in seconds: 15 minutes take 170 MB.
pub const MAX_AUDIO_SECONDS: f64 = 15.0 * 60.0;

/// The sound of a media file: stereo, 16-bit, at [`AUDIO_RATE`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClipAudio {
    /// Left and right samples, one after the other.
    pub samples: Vec<i16>,
}

impl ClipAudio {
    /// Number of stereo frames.
    pub fn frames(&self) -> usize {
        self.samples.len() / 2
    }

    /// Duration in seconds.
    pub fn duration(&self) -> f64 {
        self.frames() as f64 / f64::from(AUDIO_RATE)
    }

    /// Left and right levels at `seconds`, from -1 to 1, interpolated
    /// between samples; silence outside of the sound.
    pub fn sample_at(&self, seconds: f64) -> [f32; 2] {
        let position = seconds * f64::from(AUDIO_RATE);
        if position.is_nan() || position < 0.0 {
            return [0.0; 2];
        }
        let index = position.floor() as usize;
        let frames = self.frames();
        if index >= frames {
            return [0.0; 2];
        }
        let next = (index + 1).min(frames - 1);
        let t = (position - index as f64) as f32;
        let at =
            |frame: usize, channel: usize| f32::from(self.samples[frame * 2 + channel]) / 32768.0;
        [0, 1].map(|channel| at(index, channel) * (1.0 - t) + at(next, channel) * t)
    }
}

/// Decodes the sound of the file at `path`. `None` when it has none, or
/// on systems without a decoder for it.
pub fn decode_audio(path: &Path) -> Result<Option<ClipAudio>, DecodeError> {
    #[cfg(target_os = "macos")]
    {
        crate::avf::decode_audio(path)
    }
    #[cfg(target_os = "windows")]
    {
        crate::mf::decode_audio(path)
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = path;
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samples_are_interpolated_and_silent_outside() {
        let audio = ClipAudio {
            samples: vec![0, 0, 16384, -16384, 0, 0],
        };
        assert_eq!(audio.frames(), 3);
        let step = 1.0 / f64::from(AUDIO_RATE);
        assert_eq!(audio.sample_at(step), [0.5, -0.5]);
        assert_eq!(audio.sample_at(step * 1.5), [0.25, -0.25]);
        assert_eq!(audio.sample_at(-1.0), [0.0; 2]);
        assert_eq!(audio.sample_at(f64::NAN), [0.0; 2]);
        assert_eq!(audio.sample_at(1.0), [0.0; 2]);
    }
}
