//! Levels of the bands of a sound, from its spectrum.

use std::sync::Arc;

use prism_core::{BAND_COUNT, Band};
use rustfft::num_complex::Complex;
use rustfft::{Fft, FftPlanner};

/// Samples per analysis: 43 ms at 48 kHz, 23 Hz between frequencies.
pub const WINDOW: usize = 2048;

/// Samples between two analyses: about a hundred per second.
const HOP: usize = 512;

/// Levels shown from 0 to 1 cover this many decibels below full scale.
pub const RANGE_DB: f32 = 50.0;

/// Seconds a level takes to fall from 1 to 0 when the sound stops. A level
/// rises at once.
const RELEASE: f32 = 0.25;

/// Frequency range of each band, in hertz, in the order of [`Band::ALL`].
fn range(band: Band) -> (f32, f32) {
    match band {
        Band::Level => (20.0, 20_000.0),
        Band::Bass => (20.0, 250.0),
        Band::Mid => (250.0, 2_000.0),
        Band::High => (2_000.0, 16_000.0),
    }
}

/// Measures the bands of a sound fed sample by sample. Allocates only when
/// made: it runs in the audio callback.
pub struct Analyzer {
    sample_rate: f32,
    fft: Arc<dyn Fft<f32>>,
    /// Hann window, and the sum of its squares.
    window: Vec<f32>,
    window_power: f32,
    /// The last [`WINDOW`] samples, as a ring.
    history: Vec<f32>,
    next: usize,
    since_analysis: usize,
    spectrum: Vec<Complex<f32>>,
    scratch: Vec<Complex<f32>>,
    /// First and last frequency index of each band.
    bins: [(usize, usize); BAND_COUNT],
    levels: [f32; BAND_COUNT],
}

impl Analyzer {
    /// An analyzer for a sound at `sample_rate` hertz.
    pub fn new(sample_rate: u32) -> Self {
        let sample_rate = sample_rate.max(1) as f32;
        let fft = FftPlanner::new().plan_fft_forward(WINDOW);
        let window: Vec<f32> = (0..WINDOW)
            .map(|n| {
                let phase = std::f32::consts::TAU * n as f32 / WINDOW as f32;
                0.5 - 0.5 * phase.cos()
            })
            .collect();
        let window_power = window.iter().map(|w| w * w).sum();
        let resolution = sample_rate / WINDOW as f32;
        let bins = Band::ALL.map(|band| {
            let (low, high) = range(band);
            let last = WINDOW / 2 - 1;
            let first = ((low / resolution).round() as usize).clamp(1, last);
            (
                (first),
                ((high / resolution).round() as usize).clamp(first, last),
            )
        });
        Self {
            sample_rate,
            scratch: vec![Complex::default(); fft.get_inplace_scratch_len()],
            fft,
            window,
            window_power,
            history: vec![0.0; WINDOW],
            next: 0,
            since_analysis: 0,
            spectrum: vec![Complex::default(); WINDOW],
            bins,
            levels: [0.0; BAND_COUNT],
        }
    }

    /// Adds one sample, from -1 to 1. Returns whether the levels were
    /// measured again. `gain_db` raises the sound before measuring.
    pub fn push(&mut self, sample: f32, gain_db: f32) -> bool {
        self.history[self.next] = if sample.is_finite() { sample } else { 0.0 };
        self.next = (self.next + 1) % WINDOW;
        self.since_analysis += 1;
        if self.since_analysis < HOP {
            return false;
        }
        self.since_analysis = 0;
        self.analyze(gain_db);
        true
    }

    /// Current level of each band, from 0 to 1, in the order of
    /// [`Band::ALL`].
    pub fn levels(&self) -> [f32; BAND_COUNT] {
        self.levels
    }

    fn analyze(&mut self, gain_db: f32) {
        // Oldest sample first.
        for (index, value) in self.spectrum.iter_mut().enumerate() {
            let sample = self.history[(self.next + index) % WINDOW];
            *value = Complex::new(sample * self.window[index], 0.0);
        }
        self.fft
            .process_with_scratch(&mut self.spectrum, &mut self.scratch);
        let fall = HOP as f32 / self.sample_rate / RELEASE;
        for (level, (first, last)) in self.levels.iter_mut().zip(self.bins) {
            let energy: f32 = self.spectrum[first..=last]
                .iter()
                .map(Complex::norm_sqr)
                .sum();
            // Parseval: the root mean square of the band, as the peak of a
            // sine (a full-scale sine reads 0 dB).
            let peak = (4.0 * energy / (WINDOW as f32 * self.window_power)).sqrt();
            let db = 20.0 * peak.max(1e-9).log10() + gain_db;
            let measured = ((db + RANGE_DB) / RANGE_DB).clamp(0.0, 1.0);
            *level = measured.max(*level - fall);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 48_000;

    /// Levels after a second of a sine of `frequency` hertz and `amplitude`.
    fn after_sine(frequency: f32, amplitude: f32) -> [f32; BAND_COUNT] {
        let mut analyzer = Analyzer::new(RATE);
        for n in 0..RATE {
            let phase = std::f32::consts::TAU * frequency * n as f32 / RATE as f32;
            analyzer.push(amplitude * phase.sin(), 0.0);
        }
        analyzer.levels()
    }

    #[test]
    fn a_full_scale_sine_reads_full_in_its_band_only() {
        let [level, bass, mid, high] = after_sine(80.0, 1.0);
        assert!(bass > 0.97 && level > 0.97, "{bass} {level}");
        assert!(mid < 0.3 && high < 0.1, "{mid} {high}");

        let [_, bass, mid, high] = after_sine(800.0, 1.0);
        assert!(mid > 0.97, "{mid}");
        assert!(bass < 0.3 && high < 0.3, "{bass} {high}");

        let [_, bass, mid, high] = after_sine(6_000.0, 1.0);
        assert!(high > 0.97, "{high}");
        assert!(bass < 0.1 && mid < 0.3, "{bass} {mid}");
    }

    #[test]
    fn levels_follow_loudness_in_decibels() {
        // 20 dB quieter: 20 / RANGE_DB lower.
        let loud = after_sine(80.0, 1.0)[Band::Bass.index()];
        let quiet = after_sine(80.0, 0.1)[Band::Bass.index()];
        assert!(
            ((loud - quiet) - 20.0 / RANGE_DB).abs() < 0.03,
            "{loud} {quiet}"
        );
        // Silence.
        assert_eq!(after_sine(80.0, 0.0), [0.0; BAND_COUNT]);
    }

    #[test]
    fn levels_rise_at_once_and_fall_over_the_release() {
        let mut analyzer = Analyzer::new(RATE);
        let mut n = 0u32;
        let mut feed = |analyzer: &mut Analyzer, seconds: f32, amplitude: f32, gain: f32| {
            for _ in 0..(seconds * RATE as f32) as u32 {
                let phase = std::f32::consts::TAU * 80.0 * n as f32 / RATE as f32;
                analyzer.push(amplitude * phase.sin(), gain);
                n += 1;
            }
        };
        feed(&mut analyzer, 0.1, 1.0, 0.0);
        assert!(analyzer.levels()[1] > 0.97);
        // Half the release after silence fills the window: about half.
        feed(
            &mut analyzer,
            WINDOW as f32 / RATE as f32 + RELEASE / 2.0,
            0.0,
            0.0,
        );
        let falling = analyzer.levels()[1];
        assert!((0.3..0.7).contains(&falling), "{falling}");
        feed(&mut analyzer, RELEASE, 0.0, 0.0);
        assert_eq!(analyzer.levels()[1], 0.0);
        // A gain lifts a quiet sound.
        feed(&mut analyzer, 0.1, 0.1, 20.0);
        assert!(analyzer.levels()[1] > 0.95, "{:?}", analyzer.levels());
    }
}
