//! Bounded numeric values shared by the data model.

use serde::{Deserialize, Serialize};

/// A value clamped to the `0.0..=1.0` range (opacity, crossfader position…).
///
/// Out-of-range or NaN inputs are clamped rather than rejected, so that
/// hand-edited project files stay loadable.
#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(from = "f32", into = "f32")]
pub struct Unit(f32);

impl Unit {
    /// `0.0`.
    pub const ZERO: Self = Self(0.0);
    /// `1.0`.
    pub const ONE: Self = Self(1.0);

    /// Creates a value, clamping it to `0.0..=1.0`. NaN becomes `0.0`.
    pub fn new(value: f32) -> Self {
        if value.is_nan() {
            Self::ZERO
        } else {
            Self(value.clamp(0.0, 1.0))
        }
    }

    /// Returns the inner value, always within `0.0..=1.0`.
    pub fn get(self) -> f32 {
        self.0
    }
}

impl From<f32> for Unit {
    fn from(value: f32) -> Self {
        Self::new(value)
    }
}

impl From<Unit> for f32 {
    fn from(value: Unit) -> Self {
        value.0
    }
}

/// Lowest tempo, in beats per minute.
pub const MIN_BPM: f64 = 20.0;

/// Highest tempo, in beats per minute.
pub const MAX_BPM: f64 = 999.0;

/// A tempo in beats per minute, within [`MIN_BPM`]`..=`[`MAX_BPM`].
///
/// Out-of-range inputs are clamped and NaN gives the default 120, so that
/// hand-edited project files stay loadable.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(from = "f64", into = "f64")]
pub struct Bpm(f64);

impl Bpm {
    /// 120 beats per minute.
    pub const DEFAULT: Self = Self(120.0);

    /// Creates a tempo, clamping it to the accepted range.
    pub fn new(value: f64) -> Self {
        if value.is_nan() {
            Self::DEFAULT
        } else {
            Self(value.clamp(MIN_BPM, MAX_BPM))
        }
    }

    /// Returns the tempo, always within the accepted range.
    pub fn get(self) -> f64 {
        self.0
    }
}

impl Default for Bpm {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl From<f64> for Bpm {
    fn from(value: f64) -> Self {
        Self::new(value)
    }
}

impl From<Bpm> for f64 {
    fn from(value: Bpm) -> Self {
        value.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tempos_stay_in_range() {
        assert_eq!(Bpm::new(5.0).get(), MIN_BPM);
        assert_eq!(Bpm::new(5000.0).get(), MAX_BPM);
        assert_eq!(Bpm::new(f64::NAN), Bpm::DEFAULT);
        assert_eq!(Bpm::new(128.5).get(), 128.5);
        let read: Bpm = serde_json::from_str("1.0").unwrap();
        assert_eq!(read.get(), MIN_BPM);
        assert_eq!(serde_json::to_string(&Bpm::new(128.0)).unwrap(), "128.0");
    }

    #[test]
    fn clamps_out_of_range_values() {
        assert_eq!(Unit::new(-0.5), Unit::ZERO);
        assert_eq!(Unit::new(1.5), Unit::ONE);
        assert_eq!(Unit::new(0.25).get(), 0.25);
    }

    #[test]
    fn nan_becomes_zero() {
        assert_eq!(Unit::new(f32::NAN), Unit::ZERO);
    }

    #[test]
    fn deserializing_clamps() {
        let value: Unit = serde_json::from_str("3.0").unwrap();
        assert_eq!(value, Unit::ONE);
        assert_eq!(serde_json::to_string(&Unit::new(0.5)).unwrap(), "0.5");
    }
}
