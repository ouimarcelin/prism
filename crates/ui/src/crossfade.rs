//! Automatic crossfader movement over a set time.

/// A crossfader travelling from one position to another.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct AutoFade {
    from: f32,
    to: f32,
    elapsed: f32,
    duration: f32,
}

impl AutoFade {
    /// Starts a move from `from` to `to` lasting `duration` seconds.
    pub(crate) fn new(from: f32, to: f32, duration: f32) -> Self {
        Self {
            from,
            to,
            elapsed: 0.0,
            duration: duration.max(0.0),
        }
    }

    /// Advances by `dt` seconds. Returns the new position and whether the
    /// move is over.
    pub(crate) fn step(&mut self, dt: f32) -> (f32, bool) {
        self.elapsed += dt.max(0.0);
        if self.duration <= 0.0 || self.elapsed >= self.duration {
            return (self.to, true);
        }
        let t = self.elapsed / self.duration;
        (self.from + (self.to - self.from) * t, false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moves_linearly_then_stops_on_target() {
        let mut fade = AutoFade::new(0.2, 1.0, 2.0);
        let (position, done) = fade.step(0.5);
        assert!((position - 0.4).abs() < 1e-6 && !done);
        let (position, done) = fade.step(1.0);
        assert!((position - 0.8).abs() < 1e-6 && !done);
        assert_eq!(fade.step(5.0), (1.0, true));
    }

    #[test]
    fn zero_duration_jumps() {
        assert_eq!(AutoFade::new(1.0, 0.0, 0.0).step(0.0), (0.0, true));
    }
}
