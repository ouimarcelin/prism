//! Effects: image treatments chained on a clip, a layer or the whole
//! composition.
//!
//! The model only names an effect and holds its settings; what each kind
//! does and which parameters it has is up to the renderer.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// One effect of a chain.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Effect {
    /// Kind of effect, such as `blur`. A kind this build does not know is
    /// kept in the project and skipped when rendering.
    pub kind: String,
    /// Parameter values by name. A missing parameter takes its default.
    #[serde(default)]
    pub params: BTreeMap<String, f32>,
    /// When set, the effect is skipped.
    #[serde(default)]
    pub bypass: bool,
    /// Parameters that move on their own, by name: they replace the set
    /// value while they are there.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub animations: BTreeMap<String, Animation>,
    /// Parameters that follow a slider of the clip's dashboard, by name:
    /// the index of the slider. The slider runs the whole range of the
    /// parameter. An animation goes first.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub links: BTreeMap<String, usize>,
}

impl Effect {
    /// An effect of `kind` with every parameter at its default.
    pub fn new(kind: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            params: BTreeMap::new(),
            bypass: false,
            animations: BTreeMap::new(),
            links: BTreeMap::new(),
        }
    }

    /// Value of a parameter, if it was set.
    pub fn param(&self, name: &str) -> Option<f32> {
        self.params.get(name).copied()
    }

    /// Sets a parameter. NaN is ignored.
    pub fn set_param(&mut self, name: impl Into<String>, value: f32) {
        if !value.is_nan() {
            self.params.insert(name.into(), value);
        }
    }
}

/// Waveform of an animation, over one cycle.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Shape {
    /// Smooth back and forth, starting at [`Animation::from`].
    #[default]
    Sine,
    /// Straight up, then straight down.
    Triangle,
    /// Straight up, then a jump back.
    Ramp,
    /// [`Animation::to`] for the first half, [`Animation::from`] for the
    /// second.
    Square,
    /// A new random value at every cycle, held until the next.
    Random,
}

impl Shape {
    /// Every shape, in display order.
    pub const ALL: [Self; 5] = [
        Self::Sine,
        Self::Triangle,
        Self::Ramp,
        Self::Square,
        Self::Random,
    ];
}

/// Length of one cycle of an animation.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Period {
    /// A number of beats at the tempo of the composition: the animation
    /// follows the tempo.
    Beats(f32),
    /// A number of seconds, whatever the tempo.
    Seconds(f32),
}

/// Shortest period accepted, in beats or seconds.
pub const MIN_PERIOD: f32 = 1.0 / 64.0;

/// Where the clocks animations follow stand at one frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Moment {
    /// Beat position of the tempo clock.
    pub beat: f64,
    /// Seconds since the engine started.
    pub seconds: f64,
    /// Beat position when the clip the setting belongs to was launched:
    /// envelopes start there.
    pub launch_beat: f64,
    /// Seconds since the engine started, when the clip was launched.
    pub launch_seconds: f64,
}

impl Moment {
    /// The moment at `beat` and `seconds`, the clip launched at the start.
    pub fn new(beat: f64, seconds: f64) -> Self {
        Self {
            beat,
            seconds,
            ..Self::default()
        }
    }

    /// The same moment, for a clip launched at `beat` and `seconds`.
    pub fn launched_at(self, beat: f64, seconds: f64) -> Self {
        Self {
            launch_beat: beat,
            launch_seconds: seconds,
            ..self
        }
    }
}

/// A parameter moving between two values: in a loop (a low-frequency
/// oscillator) or once from the launch of the clip (an envelope).
///
/// Its value depends only on the beat position or the time: frames skipped
/// or late do not shift it, and an animation in beats stays on the tempo,
/// however long it runs.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Animation {
    /// Waveform.
    #[serde(default)]
    pub shape: Shape,
    /// Length of a cycle.
    pub period: Period,
    /// Value at the bottom of the wave.
    pub from: f32,
    /// Value at the top of the wave.
    pub to: f32,
    /// Shift of the cycle, in cycles (`0.5` starts halfway).
    #[serde(default)]
    pub phase: f32,
    /// Played once from the launch of the clip, then held at its last
    /// value, instead of looping.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub once: bool,
}

impl Animation {
    /// A sine wave from `from` to `to` over a bar of four beats.
    pub fn new(from: f32, to: f32) -> Self {
        Self {
            shape: Shape::Sine,
            period: Period::Beats(4.0),
            from,
            to,
            phase: 0.0,
            once: false,
        }
    }

    /// Value at moment `at`.
    pub fn value_at(&self, at: Moment) -> f32 {
        let (position, launch, length) = match self.period {
            Period::Beats(length) => (at.beat, at.launch_beat, length),
            Period::Seconds(length) => (at.seconds, at.launch_seconds, length),
        };
        let length = if length.is_nan() {
            1.0
        } else {
            f64::from(length.max(MIN_PERIOD))
        };
        let (seed, t) = if self.once {
            // One cycle from the launch, then the value it ends on. A new
            // random value at each launch.
            let t = ((position - launch) / length + f64::from(self.phase)).clamp(0.0, 1.0);
            (launch.to_bits(), t)
        } else {
            let cycles = position / length + f64::from(self.phase);
            let cycle = cycles.floor();
            (cycle as i64 as u64, cycles - cycle)
        };
        let level = match self.shape {
            Shape::Sine => 0.5 - 0.5 * (std::f64::consts::TAU * t).cos(),
            Shape::Triangle => 1.0 - (1.0 - 2.0 * t).abs(),
            Shape::Ramp => t,
            Shape::Square => {
                if t < 0.5 {
                    1.0
                } else {
                    0.0
                }
            }
            Shape::Random => random(seed),
        };
        self.from + (self.to - self.from) * level as f32
    }
}

/// A value in `0.0..1.0`, always the same for the same seed.
fn random(seed: u64) -> f64 {
    // SplitMix64.
    let mut z = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 11) as f64 / (1u64 << 53) as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-5
    }

    #[test]
    fn shapes_over_one_cycle() {
        let at = |shape, t: f64| {
            let animation = Animation {
                shape,
                period: Period::Beats(1.0),
                ..Animation::new(10.0, 20.0)
            };
            animation.value_at(Moment::new(t, 0.0))
        };
        assert!(close(at(Shape::Sine, 0.0), 10.0) && close(at(Shape::Sine, 0.5), 20.0));
        assert!(close(at(Shape::Triangle, 0.25), 15.0) && close(at(Shape::Triangle, 0.5), 20.0));
        assert!(close(at(Shape::Ramp, 0.75), 17.5));
        assert!(close(at(Shape::Square, 0.4), 20.0) && close(at(Shape::Square, 0.6), 10.0));
        // Random: held during a cycle, within the range, changing between.
        let held = at(Shape::Random, 3.1);
        assert!(close(held, at(Shape::Random, 3.9)));
        assert!((10.0..=20.0).contains(&held));
        assert!((0..20).any(|cycle| !close(at(Shape::Random, f64::from(cycle) + 0.5), held)));
    }

    #[test]
    fn periods_in_beats_or_seconds_and_phase() {
        let mut animation = Animation::new(0.0, 1.0);
        animation.shape = Shape::Ramp;
        // Four beats: beat 2 is halfway, whatever the time.
        assert!(close(animation.value_at(Moment::new(2.0, 99.0)), 0.5));
        animation.period = Period::Seconds(2.0);
        assert!(close(animation.value_at(Moment::new(99.0, 0.5)), 0.25));
        animation.phase = 0.5;
        assert!(close(animation.value_at(Moment::new(99.0, 0.5)), 0.75));
        // A period too short or NaN stays usable.
        animation.period = Period::Beats(0.0);
        assert!(animation.value_at(Moment::new(1.0, 0.0)).is_finite());
        animation.period = Period::Seconds(f32::NAN);
        assert!(animation.value_at(Moment::new(0.0, 1.0)).is_finite());
    }

    #[test]
    fn beat_animations_stay_on_the_beat() {
        // A square wave of one beat, after ten minutes at 128 BPM: still
        // high on the first half of each beat, low on the second.
        let animation = Animation {
            shape: Shape::Square,
            period: Period::Beats(1.0),
            ..Animation::new(0.0, 1.0)
        };
        for beat in [1280.1, 1280.4, 4_000_000.2] {
            assert_eq!(
                animation.value_at(Moment::new(beat, 0.0)),
                1.0,
                "beat {beat}"
            );
            assert_eq!(
                animation.value_at(Moment::new(beat + 0.5, 0.0)),
                0.0,
                "beat {beat}"
            );
        }
    }

    #[test]
    fn envelopes_play_once_from_the_launch() {
        let envelope = Animation {
            shape: Shape::Ramp,
            period: Period::Beats(2.0),
            once: true,
            ..Animation::new(0.0, 1.0)
        };
        let at = |beat| Moment::new(beat, 0.0).launched_at(10.0, 0.0);
        // Before the launch (a resync can move the beat back): the start.
        assert!(close(envelope.value_at(at(9.0)), 0.0));
        assert!(close(envelope.value_at(at(10.0)), 0.0));
        assert!(close(envelope.value_at(at(11.0)), 0.5));
        // Then held at the end, however long after.
        assert!(close(envelope.value_at(at(12.0)), 1.0));
        assert!(close(envelope.value_at(at(500.0)), 1.0));
        // Shapes end where their cycle ends: a triangle goes back down.
        let triangle = Animation {
            shape: Shape::Triangle,
            ..envelope
        };
        assert!(close(triangle.value_at(at(11.0)), 1.0));
        assert!(close(triangle.value_at(at(14.0)), 0.0));
        // In seconds, from the launch in seconds.
        let seconds = Animation {
            period: Period::Seconds(4.0),
            ..envelope
        };
        let moment = Moment::new(0.0, 7.0).launched_at(0.0, 6.0);
        assert!(close(seconds.value_at(moment), 0.25));
        // Random: one value per launch, held.
        let random = Animation {
            shape: Shape::Random,
            ..envelope
        };
        let value = random.value_at(at(11.0));
        assert!(close(value, random.value_at(at(300.0))));
        assert!((1..20).any(|launch| {
            let moment = Moment::new(100.0, 0.0).launched_at(f64::from(launch), 0.0);
            !close(random.value_at(moment), value)
        }));
    }

    #[test]
    fn animations_are_saved_only_when_present() {
        let mut effect = Effect::new("wave");
        effect
            .animations
            .insert("amplitude".into(), Animation::new(0.0, 0.1));
        let json = serde_json::to_string(&effect).unwrap();
        let read: Effect = serde_json::from_str(&json).unwrap();
        assert_eq!(read, effect);
        assert!(json.contains(r#""period":{"beats":4.0}"#), "{json}");
        assert!(!json.contains("once"), "{json}");
        effect.animations.get_mut("amplitude").unwrap().once = true;
        let json = serde_json::to_string(&effect).unwrap();
        assert_eq!(serde_json::from_str::<Effect>(&json).unwrap(), effect);
    }

    #[test]
    fn parameters_are_saved_by_name() {
        let mut effect = Effect::new("blur");
        assert_eq!(effect.param("radius"), None);
        effect.set_param("radius", 4.0);
        effect.set_param("radius", f32::NAN);
        assert_eq!(effect.param("radius"), Some(4.0));

        let json = serde_json::to_string(&effect).unwrap();
        assert_eq!(
            json,
            r#"{"kind":"blur","params":{"radius":4.0},"bypass":false}"#
        );
        let read: Effect = serde_json::from_str(r#"{"kind":"future"}"#).unwrap();
        assert_eq!(read, Effect::new("future"));
    }
}
