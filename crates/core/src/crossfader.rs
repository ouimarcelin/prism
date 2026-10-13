//! A/B crossfader shared by the whole composition.

use std::f32::consts::FRAC_PI_2;

use serde::{Deserialize, Serialize};

use crate::Unit;

/// Side of the crossfader a layer is assigned to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CrossfaderSide {
    /// Fully visible when the crossfader is at `0.0`.
    A,
    /// Fully visible when the crossfader is at `1.0`.
    B,
}

/// How the crossfader position maps to the level of each side.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CrossfaderCurve {
    /// Both sides at full level in the center; each side fades out over the
    /// opposite half of the travel.
    #[default]
    FullCenter,
    /// One side fades out while the other fades in; both at half in the center.
    Linear,
    /// Constant perceived power: both sides at about 0.707 in the center.
    EqualPower,
}

/// Crossfader state: position and curve.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Crossfader {
    /// `0.0` is full A, `1.0` is full B.
    pub position: Unit,
    /// Mapping from position to side levels.
    pub curve: CrossfaderCurve,
}

impl Default for Crossfader {
    fn default() -> Self {
        Self {
            position: Unit::new(0.5),
            curve: CrossfaderCurve::default(),
        }
    }
}

impl Crossfader {
    /// Level, within `0.0..=1.0`, applied to layers assigned to `side`.
    pub fn gain(&self, side: CrossfaderSide) -> f32 {
        let p = self.position.get();
        let (a, b) = match self.curve {
            CrossfaderCurve::FullCenter => ((2.0 * (1.0 - p)).min(1.0), (2.0 * p).min(1.0)),
            CrossfaderCurve::Linear => (1.0 - p, p),
            CrossfaderCurve::EqualPower => ((p * FRAC_PI_2).cos(), (p * FRAC_PI_2).sin()),
        };
        match side {
            CrossfaderSide::A => a,
            CrossfaderSide::B => b,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gains(curve: CrossfaderCurve, position: f32) -> (f32, f32) {
        let xf = Crossfader {
            position: Unit::new(position),
            curve,
        };
        (xf.gain(CrossfaderSide::A), xf.gain(CrossfaderSide::B))
    }

    fn assert_close(actual: (f32, f32), expected: (f32, f32)) {
        assert!(
            (actual.0 - expected.0).abs() < 1e-6 && (actual.1 - expected.1).abs() < 1e-6,
            "expected {expected:?}, got {actual:?}"
        );
    }

    #[test]
    fn every_curve_isolates_a_side_at_the_ends() {
        for curve in [
            CrossfaderCurve::FullCenter,
            CrossfaderCurve::Linear,
            CrossfaderCurve::EqualPower,
        ] {
            assert_close(gains(curve, 0.0), (1.0, 0.0));
            assert_close(gains(curve, 1.0), (0.0, 1.0));
        }
    }

    #[test]
    fn center_levels_depend_on_curve() {
        assert_close(gains(CrossfaderCurve::FullCenter, 0.5), (1.0, 1.0));
        assert_close(gains(CrossfaderCurve::Linear, 0.5), (0.5, 0.5));
        let h = std::f32::consts::FRAC_1_SQRT_2;
        assert_close(gains(CrossfaderCurve::EqualPower, 0.5), (h, h));
    }

    #[test]
    fn full_center_fades_only_the_far_side() {
        assert_close(gains(CrossfaderCurve::FullCenter, 0.25), (1.0, 0.5));
        assert_close(gains(CrossfaderCurve::FullCenter, 0.75), (0.5, 1.0));
    }

    #[test]
    fn default_shows_both_sides() {
        let xf = Crossfader::default();
        assert_eq!(xf.gain(CrossfaderSide::A), 1.0);
        assert_eq!(xf.gain(CrossfaderSide::B), 1.0);
    }
}
