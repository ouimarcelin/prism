//! Levels shared between the audio callback, the render thread and the
//! interface, without locks.

use std::sync::atomic::{AtomicU32, Ordering};

use prism_core::BAND_COUNT;

/// The last levels measured, from 0 to 1, in the order of
/// [`prism_core::Band::ALL`], and the gain to measure with.
#[derive(Debug, Default)]
pub struct AudioLevels {
    values: [AtomicU32; BAND_COUNT],
    gain_db: AtomicU32,
}

impl AudioLevels {
    /// Current levels; all at 0 without input.
    pub fn get(&self) -> [f32; BAND_COUNT] {
        std::array::from_fn(|index| f32::from_bits(self.values[index].load(Ordering::Relaxed)))
    }

    /// Publishes new levels.
    pub fn set(&self, levels: [f32; BAND_COUNT]) {
        for (value, level) in self.values.iter().zip(levels) {
            value.store(level.to_bits(), Ordering::Relaxed);
        }
    }

    /// Gain applied before measuring, in decibels.
    pub fn gain_db(&self) -> f32 {
        f32::from_bits(self.gain_db.load(Ordering::Relaxed))
    }

    /// Sets the gain applied before measuring, within 0 to [`MAX_GAIN_DB`].
    pub fn set_gain_db(&self, gain: f32) {
        let gain = if gain.is_finite() {
            gain.clamp(0.0, MAX_GAIN_DB)
        } else {
            0.0
        };
        self.gain_db.store(gain.to_bits(), Ordering::Relaxed);
    }
}

/// Highest gain, in decibels: enough for a quiet line or a far microphone.
pub const MAX_GAIN_DB: f32 = 40.0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_and_gain_round_trip() {
        let levels = AudioLevels::default();
        assert_eq!(levels.get(), [0.0; BAND_COUNT]);
        levels.set([0.1, 0.2, 0.3, 0.4]);
        assert_eq!(levels.get(), [0.1, 0.2, 0.3, 0.4]);
        levels.set_gain_db(12.0);
        assert_eq!(levels.gain_db(), 12.0);
        levels.set_gain_db(99.0);
        assert_eq!(levels.gain_db(), MAX_GAIN_DB);
        levels.set_gain_db(f32::NAN);
        assert_eq!(levels.gain_db(), 0.0);
    }
}
