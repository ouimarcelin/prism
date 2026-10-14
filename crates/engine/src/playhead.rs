//! Playhead: the position of a clip in its media, advanced in real time.
//!
//! Pure logic, without threads or GPU: direction, speed, loop modes, in and
//! out points, seeking and holding for scratching.

use prism_core::{Direction, LoopMode, Playback};
use prism_media::MediaInfo;

use crate::random::Random;

/// Timing of the media a playhead runs over.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MediaTiming {
    /// Total duration, in seconds.
    pub duration: f64,
    /// Average duration of one frame, in seconds.
    pub frame_duration: f64,
}

impl MediaTiming {
    /// Timing of a media.
    pub fn from_info(info: &MediaInfo) -> Self {
        let duration = info.duration_seconds();
        let frames = info.frame_count();
        Self {
            duration,
            frame_duration: if frames > 0 {
                duration / frames as f64
            } else {
                0.0
            },
        }
    }
}

/// Played range in seconds: the in and out points, clamped to the media.
pub fn play_range(playback: &Playback, media: MediaTiming) -> (f64, f64) {
    let duration = media.duration.max(0.0);
    let clamp = |value: f64| {
        if value.is_nan() {
            0.0
        } else {
            value.clamp(0.0, duration)
        }
    };
    let start = clamp(playback.in_point);
    let end = clamp(playback.out_point.unwrap_or(duration)).max(start);
    (start, end)
}

/// Position of a clip in its media and the state needed to move it.
#[derive(Clone, Debug, PartialEq)]
pub struct Playhead {
    position: f64,
    /// `1.0` when moving forward, `-1.0` backward. Ping-pong flips it.
    travel: f64,
    finished: bool,
    random_elapsed: f64,
    held: bool,
}

impl Playhead {
    /// A playhead at the start of the played range, or at its end when the
    /// clip plays backward.
    pub fn new(playback: &Playback, media: MediaTiming) -> Self {
        let (start, end) = play_range(playback, media);
        let backward = playback.direction == Direction::Backward;
        Self {
            position: if backward { end } else { start },
            travel: if backward { -1.0 } else { 1.0 },
            finished: false,
            random_elapsed: 0.0,
            held: false,
        }
    }

    /// Position in seconds from the start of the media.
    pub fn position(&self) -> f64 {
        self.position
    }

    /// Current direction of travel: `1.0` forward, `-1.0` backward.
    #[cfg(test)]
    pub fn travel(&self) -> f64 {
        self.travel
    }

    /// Whether a clip played once has reached its end.
    pub fn is_finished(&self) -> bool {
        self.finished
    }

    /// Whether the playhead is held in place (scratching).
    pub fn is_held(&self) -> bool {
        self.held
    }

    /// Holds the playhead in place, or releases it.
    pub fn set_held(&mut self, held: bool) {
        self.held = held;
    }

    /// Moves to `position`, clamped to the played range.
    pub fn seek(&mut self, position: f64, playback: &Playback, media: MediaTiming) {
        let (start, end) = play_range(playback, media);
        self.position = if position.is_nan() {
            start
        } else {
            position.clamp(start, end)
        };
        self.finished = false;
    }

    /// Advances by `elapsed` seconds of real time.
    pub fn advance(
        &mut self,
        playback: &Playback,
        media: MediaTiming,
        elapsed: f64,
        random: &mut Random,
    ) {
        let (start, end) = play_range(playback, media);
        // The in or out point may have moved since the last step.
        self.position = self.position.clamp(start, end);
        let length = end - start;
        if self.held || elapsed.is_nan() || elapsed <= 0.0 || length <= 0.0 {
            return;
        }
        let distance = elapsed * f64::from(playback.effective_speed());

        let base = match playback.direction {
            Direction::Paused => return,
            Direction::Random => {
                self.advance_random(start, length, media, distance, random);
                return;
            }
            Direction::Forward => 1.0,
            Direction::Backward => -1.0,
        };
        if playback.loop_mode != LoopMode::PingPong {
            self.travel = base;
        }

        match playback.loop_mode {
            LoopMode::Loop => {
                self.finished = false;
                let offset = self.position - start + self.travel * distance;
                self.position = start + offset.rem_euclid(length);
            }
            LoopMode::Once => {
                let target = self.position + self.travel * distance;
                self.finished = false;
                if target >= end {
                    self.position = end;
                    self.finished = true;
                } else if target <= start {
                    self.position = start;
                    self.finished = true;
                } else {
                    self.position = target;
                }
            }
            LoopMode::PingPong => {
                self.finished = false;
                // A full round trip returns to the same state.
                let mut remaining = distance % (2.0 * length);
                while remaining > 0.0 {
                    let room = if self.travel > 0.0 {
                        end - self.position
                    } else {
                        self.position - start
                    };
                    if remaining <= room {
                        self.position += self.travel * remaining;
                        break;
                    }
                    self.position = if self.travel > 0.0 { end } else { start };
                    remaining -= room;
                    self.travel = -self.travel;
                }
            }
        }
    }

    /// Random playback: jumps to a random position once per media frame,
    /// scaled by the speed.
    fn advance_random(
        &mut self,
        start: f64,
        length: f64,
        media: MediaTiming,
        distance: f64,
        random: &mut Random,
    ) {
        let step = if media.frame_duration > 0.0 {
            media.frame_duration
        } else {
            length
        };
        self.random_elapsed += distance;
        if self.random_elapsed >= step {
            self.random_elapsed %= step;
            self.position = start + random.next_f64() * length;
        }
    }

    /// Positions reached on each of the next `ticks` steps of `tick`
    /// seconds, if nothing changes. Empty for random playback, which cannot
    /// be predicted.
    pub fn predict(
        &self,
        playback: &Playback,
        media: MediaTiming,
        tick: f64,
        ticks: usize,
    ) -> Vec<f64> {
        if playback.direction == Direction::Random || self.held {
            return Vec::new();
        }
        let mut ghost = self.clone();
        let mut random = Random::new(1);
        (0..ticks)
            .map(|_| {
                ghost.advance(playback, media, tick, &mut random);
                ghost.position
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 10 seconds at 10 frames per second.
    const MEDIA: MediaTiming = MediaTiming {
        duration: 10.0,
        frame_duration: 0.1,
    };

    fn playback(direction: Direction, loop_mode: LoopMode) -> Playback {
        Playback {
            direction,
            loop_mode,
            ..Playback::default()
        }
    }

    fn run(playhead: &mut Playhead, playback: &Playback, seconds: f64) {
        let mut random = Random::new(7);
        playhead.advance(playback, MEDIA, seconds, &mut random);
    }

    fn assert_near(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 1e-9,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn forward_and_backward_start_at_opposite_ends() {
        let forward = Playhead::new(&Playback::default(), MEDIA);
        assert_near(forward.position(), 0.0);
        let backward = Playhead::new(&playback(Direction::Backward, LoopMode::Loop), MEDIA);
        assert_near(backward.position(), 10.0);
    }

    #[test]
    fn speed_scales_the_advance() {
        let settings = Playback {
            speed: 2.0,
            ..Playback::default()
        };
        let mut playhead = Playhead::new(&settings, MEDIA);
        run(&mut playhead, &settings, 1.5);
        assert_near(playhead.position(), 3.0);
    }

    #[test]
    fn loop_wraps_in_both_directions() {
        let forward = Playback::default();
        let mut playhead = Playhead::new(&forward, MEDIA);
        run(&mut playhead, &forward, 12.5);
        assert_near(playhead.position(), 2.5);

        let backward = playback(Direction::Backward, LoopMode::Loop);
        let mut playhead = Playhead::new(&backward, MEDIA);
        run(&mut playhead, &backward, 11.0);
        assert_near(playhead.position(), 9.0);
    }

    #[test]
    fn once_stops_at_the_end() {
        let settings = playback(Direction::Forward, LoopMode::Once);
        let mut playhead = Playhead::new(&settings, MEDIA);
        run(&mut playhead, &settings, 4.0);
        assert!(!playhead.is_finished());
        run(&mut playhead, &settings, 20.0);
        assert_near(playhead.position(), 10.0);
        assert!(playhead.is_finished());
        run(&mut playhead, &settings, 1.0);
        assert_near(playhead.position(), 10.0);

        // Reversing the direction plays it back.
        let reversed = playback(Direction::Backward, LoopMode::Once);
        run(&mut playhead, &reversed, 3.0);
        assert_near(playhead.position(), 7.0);
        assert!(!playhead.is_finished());
    }

    #[test]
    fn ping_pong_reflects_at_both_ends() {
        let settings = playback(Direction::Forward, LoopMode::PingPong);
        let mut playhead = Playhead::new(&settings, MEDIA);
        run(&mut playhead, &settings, 13.0);
        assert_near(playhead.position(), 7.0);
        assert_eq!(playhead.travel(), -1.0);
        run(&mut playhead, &settings, 9.0);
        assert_near(playhead.position(), 2.0);
        assert_eq!(playhead.travel(), 1.0);
        // Several round trips in one step.
        run(&mut playhead, &settings, 41.0);
        assert_near(playhead.position(), 3.0);
    }

    #[test]
    fn in_and_out_points_bound_the_range() {
        let mut settings = Playback {
            in_point: 2.0,
            out_point: Some(4.0),
            ..Playback::default()
        };
        let mut playhead = Playhead::new(&settings, MEDIA);
        assert_near(playhead.position(), 2.0);
        run(&mut playhead, &settings, 3.0);
        assert_near(playhead.position(), 3.0);

        // Moving the out point before the playhead pulls it back in.
        settings.out_point = Some(2.5);
        run(&mut playhead, &settings, 0.0);
        assert_near(playhead.position(), 2.5);
    }

    #[test]
    fn inconsistent_points_are_clamped() {
        let mut settings = Playback {
            in_point: 8.0,
            out_point: Some(3.0),
            ..Playback::default()
        };
        assert_eq!(play_range(&settings, MEDIA), (8.0, 8.0));
        settings.in_point = -5.0;
        settings.out_point = Some(50.0);
        assert_eq!(play_range(&settings, MEDIA), (0.0, 10.0));
        settings.in_point = f64::NAN;
        assert_eq!(play_range(&settings, MEDIA).0, 0.0);

        // An empty range holds the playhead.
        settings.in_point = 4.0;
        settings.out_point = Some(4.0);
        let mut playhead = Playhead::new(&settings, MEDIA);
        run(&mut playhead, &settings, 3.0);
        assert_near(playhead.position(), 4.0);
    }

    #[test]
    fn paused_and_held_playheads_do_not_move() {
        let paused = playback(Direction::Paused, LoopMode::Loop);
        let mut playhead = Playhead::new(&paused, MEDIA);
        run(&mut playhead, &paused, 5.0);
        assert_near(playhead.position(), 0.0);

        let settings = Playback::default();
        let mut playhead = Playhead::new(&settings, MEDIA);
        playhead.seek(6.0, &settings, MEDIA);
        playhead.set_held(true);
        run(&mut playhead, &settings, 2.0);
        assert_near(playhead.position(), 6.0);
        playhead.set_held(false);
        run(&mut playhead, &settings, 2.0);
        assert_near(playhead.position(), 8.0);
    }

    #[test]
    fn seek_is_clamped_to_the_range() {
        let settings = Playback::default();
        let mut playhead = Playhead::new(&settings, MEDIA);
        playhead.seek(25.0, &settings, MEDIA);
        assert_near(playhead.position(), 10.0);
        playhead.seek(f64::NAN, &settings, MEDIA);
        assert_near(playhead.position(), 0.0);
    }

    #[test]
    fn random_jumps_once_per_frame_inside_the_range() {
        let mut settings = playback(Direction::Random, LoopMode::Loop);
        settings.in_point = 2.0;
        settings.out_point = Some(3.0);
        let mut playhead = Playhead::new(&settings, MEDIA);
        let mut random = Random::new(3);
        // Less than a frame: no jump yet.
        playhead.advance(&settings, MEDIA, 0.05, &mut random);
        assert_near(playhead.position(), 2.0);
        let mut positions = Vec::new();
        for _ in 0..20 {
            playhead.advance(&settings, MEDIA, 0.1, &mut random);
            assert!((2.0..=3.0).contains(&playhead.position()));
            positions.push(playhead.position());
        }
        positions.dedup();
        assert!(positions.len() > 10, "random playback barely moved");
    }

    #[test]
    fn prediction_follows_the_playhead_without_moving_it() {
        let settings = playback(Direction::Forward, LoopMode::PingPong);
        let mut playhead = Playhead::new(&settings, MEDIA);
        playhead.seek(9.9, &settings, MEDIA);
        let predicted = playhead.predict(&settings, MEDIA, 0.05, 4);
        assert_eq!(predicted.len(), 4);
        assert_near(predicted[0], 9.95);
        assert_near(predicted[1], 10.0);
        assert_near(predicted[2], 9.95);
        assert_near(playhead.position(), 9.9);

        let random = playback(Direction::Random, LoopMode::Loop);
        assert!(playhead.predict(&random, MEDIA, 0.05, 4).is_empty());
    }
}
