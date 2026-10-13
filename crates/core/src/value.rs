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

#[cfg(test)]
mod tests {
    use super::*;

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
