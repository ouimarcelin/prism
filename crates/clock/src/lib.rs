//! Musical time of Prism.
//!
//! A [`BeatClock`] gives the beat position at any instant for a tempo. The
//! position is computed from one anchor, an instant and the beat at that
//! instant, never accumulated frame after frame: it does not drift, however
//! long it runs, and skipped frames do not shift it. [`TapTempo`] turns taps
//! on a key or a button into a tempo, [`MidiClock`] the pulses of a MIDI
//! clock. [`Timecode`] is SMPTE time, read from MIDI timecode by
//! [`MtcReader`].
//!
//! Beats are counted from `0.0` at the start of a bar; a bar has
//! [`BEATS_PER_BAR`] beats.

#![forbid(unsafe_code)]

mod ltc;
mod timecode;

pub use ltc::{LTC_BITS, LtcDecoder, ltc_bits};
pub use timecode::{MtcReader, Timecode, TimecodeRate};

use std::time::{Duration, Instant};

/// Beats in a bar.
pub const BEATS_PER_BAR: u32 = 4;

/// Beat position over time at a tempo.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BeatClock {
    bpm: f64,
    anchor_time: Instant,
    anchor_beat: f64,
}

/// Seconds from `from` to `to`, negative when `to` comes first.
fn seconds_between(from: Instant, to: Instant) -> f64 {
    if to >= from {
        (to - from).as_secs_f64()
    } else {
        -(from - to).as_secs_f64()
    }
}

impl BeatClock {
    /// A clock at `bpm` beats per minute whose first bar starts at `start`.
    pub fn new(bpm: f64, start: Instant) -> Self {
        Self {
            bpm: positive(bpm),
            anchor_time: start,
            anchor_beat: 0.0,
        }
    }

    /// Tempo in beats per minute.
    pub fn bpm(&self) -> f64 {
        self.bpm
    }

    /// Beat position at `time`. Before the anchor, the position keeps going
    /// down: it is negative before the first bar.
    pub fn beat_at(&self, time: Instant) -> f64 {
        self.anchor_beat + seconds_between(self.anchor_time, time) * self.bpm / 60.0
    }

    /// Changes the tempo from `time` on, without moving the beat position at
    /// that instant.
    pub fn set_bpm(&mut self, bpm: f64, time: Instant) {
        self.anchor_beat = self.beat_at(time);
        self.anchor_time = time;
        self.bpm = positive(bpm);
    }

    /// Starts a bar at `time`: the beat position there becomes `0.0`.
    pub fn restart_bar(&mut self, time: Instant) {
        self.anchor_time = time;
        self.anchor_beat = 0.0;
    }

    /// Puts a beat at `time`: the position there becomes the nearest whole
    /// beat, so the bar count stays where it was.
    pub fn align_beat(&mut self, time: Instant) {
        self.anchor_beat = self.beat_at(time).round();
        self.anchor_time = time;
    }

    /// Duration of `beats` beats at the current tempo.
    pub fn duration_of(&self, beats: f64) -> Duration {
        Duration::from_secs_f64((beats * 60.0 / self.bpm).max(0.0))
    }
}

/// A tempo always above zero.
fn positive(bpm: f64) -> f64 {
    if bpm.is_finite() && bpm > 0.0 {
        bpm
    } else {
        1.0
    }
}

/// Position of a beat within its bar, from `0` to [`BEATS_PER_BAR`] − 1.
pub fn beat_in_bar(beat: f64) -> u32 {
    (beat.floor() as i64).rem_euclid(i64::from(BEATS_PER_BAR)) as u32
}

/// Taps further apart than this start a new measure.
pub const TAP_TIMEOUT: Duration = Duration::from_secs(2);

/// MIDI clock pulses per beat (MIDI 1.0: 24 per quarter note).
pub const CLOCK_PULSES_PER_BEAT: usize = 24;

/// Pulses averaged for the tempo: two beats.
const CLOCK_WINDOW: usize = 2 * CLOCK_PULSES_PER_BEAT;

/// Longest gap between pulses, in microseconds, before the clock is taken
/// as stopped.
const CLOCK_TIMEOUT_MICROS: u64 = 500_000;

/// Tempo from a MIDI clock: the average interval between its last pulses,
/// timed by the MIDI driver.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MidiClock {
    /// Times of the last pulses, in microseconds.
    pulses: Vec<u64>,
    /// Times the sender started, counted.
    starts: u64,
}

impl MidiClock {
    /// Records a pulse at `micros`; returns the tempo once a beat of pulses
    /// came in.
    pub fn pulse(&mut self, micros: u64) -> Option<f64> {
        if let Some(&last) = self.pulses.last()
            && (micros <= last || micros - last > CLOCK_TIMEOUT_MICROS)
        {
            self.pulses.clear();
        }
        self.pulses.push(micros);
        if self.pulses.len() > CLOCK_WINDOW + 1 {
            self.pulses.remove(0);
        }
        self.bpm()
    }

    /// Tempo of the last pulses, in beats per minute, once a beat of them
    /// came in.
    pub fn bpm(&self) -> Option<f64> {
        let intervals = self.pulses.len().checked_sub(1)?;
        if intervals < CLOCK_PULSES_PER_BEAT {
            return None;
        }
        let span = (self.pulses.last()? - self.pulses.first()?) as f64 / 1e6;
        (span > 0.0).then(|| 60.0 * intervals as f64 / (span * CLOCK_PULSES_PER_BEAT as f64))
    }

    /// Whether pulses still come in at `micros`.
    pub fn is_running(&self, micros: u64) -> bool {
        self.pulses
            .last()
            .is_some_and(|&last| micros.saturating_sub(last) <= CLOCK_TIMEOUT_MICROS)
    }

    /// The sender started (MIDI Start): the bar begins now.
    pub fn start(&mut self) {
        self.starts += 1;
        self.pulses.clear();
    }

    /// Times the sender started, counted from the first.
    pub fn starts(&self) -> u64 {
        self.starts
    }
}

/// Taps averaged for the tempo.
pub const TAP_WINDOW: usize = 8;

/// Tempo from taps: the average interval between the last taps.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TapTempo {
    taps: Vec<Instant>,
}

impl TapTempo {
    /// Records a tap at `time`. Returns the tempo in beats per minute from
    /// the second tap of a measure on.
    pub fn tap(&mut self, time: Instant) -> Option<f64> {
        if let Some(&last) = self.taps.last()
            && (time <= last || time - last > TAP_TIMEOUT)
        {
            self.taps.clear();
        }
        self.taps.push(time);
        if self.taps.len() > TAP_WINDOW {
            self.taps.remove(0);
        }
        let (&first, &last) = (self.taps.first()?, self.taps.last()?);
        let intervals = self.taps.len() - 1;
        if intervals == 0 {
            return None;
        }
        let seconds = (last - first).as_secs_f64() / intervals as f64;
        Some(60.0 / seconds)
    }

    /// Taps counted in the current measure.
    pub fn count(&self) -> usize {
        self.taps.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn midi_clock_pulses_give_the_tempo() {
        let mut clock = MidiClock::default();
        // 125 BPM: 20 000 µs between pulses.
        let mut time = 1_000_000;
        let mut tempo = None;
        for pulse in 0..60 {
            tempo = clock.pulse(time);
            if pulse < CLOCK_PULSES_PER_BEAT {
                assert_eq!(tempo, None, "pulse {pulse}");
            }
            // A little jitter, as from a real driver.
            time += if pulse % 2 == 0 { 20_150 } else { 19_850 };
        }
        let tempo = tempo.unwrap();
        assert!((tempo - 125.0).abs() < 0.2, "{tempo}");
        assert!(clock.is_running(time));
        assert!(!clock.is_running(time + 600_000));
        // A long gap starts over.
        assert_eq!(clock.pulse(time + 600_000), None);
        clock.start();
        assert_eq!(clock.starts(), 1);
        assert_eq!(clock.bpm(), None);
    }

    fn at(start: Instant, seconds: f64) -> Instant {
        if seconds >= 0.0 {
            start + Duration::from_secs_f64(seconds)
        } else {
            start - Duration::from_secs_f64(-seconds)
        }
    }

    #[test]
    fn beats_follow_the_tempo_without_drift() {
        let start = Instant::now();
        let clock = BeatClock::new(128.0, start);
        assert_eq!(clock.beat_at(start), 0.0);
        assert!((clock.beat_at(at(start, 0.5)) - 128.0 / 120.0).abs() < 1e-12);
        // Ten minutes of frames at 60 per second: every position is exact,
        // the last one included.
        let worst = (1..=36_000)
            .map(|frame| {
                let seconds = f64::from(frame) / 60.0;
                (clock.beat_at(at(start, seconds)) - seconds * 128.0 / 60.0).abs()
            })
            .fold(0.0, f64::max);
        assert!(worst < 1e-9, "{worst}");
        assert!((clock.beat_at(at(start, 600.0)) - 1280.0).abs() < 1e-9);
    }

    #[test]
    fn positions_before_the_start_are_negative() {
        let start = Instant::now() + Duration::from_secs(10);
        let clock = BeatClock::new(60.0, start);
        assert!((clock.beat_at(at(start, -1.5)) + 1.5).abs() < 1e-12);
        assert_eq!(beat_in_bar(-0.25), 3);
    }

    #[test]
    fn tempo_changes_keep_the_position() {
        let start = Instant::now();
        let mut clock = BeatClock::new(120.0, start);
        let change = at(start, 3.0);
        clock.set_bpm(60.0, change);
        assert!((clock.beat_at(change) - 6.0).abs() < 1e-12);
        assert!((clock.beat_at(at(start, 5.0)) - 8.0).abs() < 1e-12);
        assert_eq!(clock.bpm(), 60.0);
    }

    #[test]
    fn bars_restart_and_beats_align() {
        let start = Instant::now();
        let mut clock = BeatClock::new(120.0, start);
        clock.restart_bar(at(start, 1.3));
        assert_eq!(clock.beat_at(at(start, 1.3)), 0.0);
        assert!((clock.beat_at(at(start, 2.3)) - 2.0).abs() < 1e-12);

        // 2.6 beats after the restart: the beat is pulled to 3.
        clock.align_beat(at(start, 2.6));
        assert_eq!(clock.beat_at(at(start, 2.6)), 3.0);
        assert_eq!(beat_in_bar(clock.beat_at(at(start, 2.6))), 3);
        assert_eq!(beat_in_bar(clock.beat_at(at(start, 3.1))), 0);
    }

    #[test]
    fn durations_follow_the_tempo() {
        let clock = BeatClock::new(120.0, Instant::now());
        assert_eq!(clock.duration_of(4.0), Duration::from_secs(2));
        assert_eq!(clock.duration_of(-1.0), Duration::ZERO);
    }

    #[test]
    fn invalid_tempos_are_replaced() {
        let start = Instant::now();
        assert_eq!(BeatClock::new(f64::NAN, start).bpm(), 1.0);
        assert_eq!(BeatClock::new(-5.0, start).bpm(), 1.0);
    }

    #[test]
    fn taps_give_the_average_tempo() {
        let start = Instant::now();
        let mut taps = TapTempo::default();
        assert_eq!(taps.tap(start), None);
        // Half a second apart, with a little jitter: 120 BPM.
        let bpm = [0.51, 0.99, 1.5, 2.0]
            .iter()
            .map(|&seconds| taps.tap(at(start, seconds)))
            .last()
            .flatten()
            .unwrap();
        assert!((bpm - 120.0).abs() < 1e-9, "{bpm}");
        assert_eq!(taps.count(), 5);
    }

    #[test]
    fn a_pause_starts_a_new_measure() {
        let start = Instant::now();
        let mut taps = TapTempo::default();
        taps.tap(start);
        taps.tap(at(start, 0.5));
        assert_eq!(taps.tap(at(start, 3.0)), None);
        assert_eq!(taps.count(), 1);
        let bpm = taps.tap(at(start, 3.4)).unwrap();
        assert!((bpm - 150.0).abs() < 1e-9, "{bpm}");
    }

    #[test]
    fn only_the_last_taps_count() {
        let start = Instant::now();
        let mut taps = TapTempo::default();
        // Ten taps at 100 BPM, then a whole window at 150 BPM.
        let mut time = 0.0;
        for _ in 0..10 {
            taps.tap(at(start, time));
            time += 0.6;
        }
        let mut bpm = None;
        for _ in 0..TAP_WINDOW {
            bpm = taps.tap(at(start, time));
            time += 0.4;
        }
        let bpm = bpm.unwrap();
        assert!((bpm - 150.0).abs() < 1e-6, "{bpm}");
        assert_eq!(taps.count(), TAP_WINDOW);
    }
}
