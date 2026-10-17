//! Linear timecode (LTC): SMPTE timecode carried in an audio signal.
//!
//! Each frame is 80 bits in biphase mark code: the signal flips at every
//! bit boundary, and once more in the middle of a `1`. The last 16 bits
//! are the sync word `0011 1111 1111 1101`; the others carry the time in
//! binary-coded decimal, least significant bit first.

use crate::{Timecode, TimecodeRate};

/// Bits of an LTC frame.
pub const LTC_BITS: usize = 80;
/// Sync word, in the order its bits are sent (bit 64 first).
const SYNC: [bool; 16] = [
    false, false, true, true, true, true, true, true, true, true, true, true, true, true, false,
    true,
];

/// Reads timecodes from audio samples.
#[derive(Clone, Debug)]
pub struct LtcDecoder {
    sample_rate: f64,
    /// Sign of the signal after the last crossing.
    high: bool,
    /// Samples since the last crossing.
    run: f64,
    /// Samples per bit, estimated from the signal.
    bit: f64,
    /// A half bit read, waiting for its second half.
    half: bool,
    /// Last bits read, oldest first.
    bits: Vec<bool>,
}

impl LtcDecoder {
    /// A decoder for samples at `sample_rate` per second.
    pub fn new(sample_rate: u32) -> Self {
        let sample_rate = f64::from(sample_rate.max(1));
        Self {
            sample_rate,
            high: false,
            run: 0.0,
            // 25 frames per second until the signal says otherwise.
            bit: sample_rate / (25.0 * LTC_BITS as f64),
            half: false,
            bits: Vec::with_capacity(LTC_BITS + 1),
        }
    }

    /// Reads `samples` (one channel, -1 to 1); returns the timecode of the
    /// last frame they complete, as it stands at the end of that frame.
    pub fn push(&mut self, samples: &[f32]) -> Option<Timecode> {
        let mut found = None;
        for &sample in samples {
            self.run += 1.0;
            // A little hysteresis against noise around zero.
            let flipped = if self.high {
                sample < -0.02
            } else {
                sample > 0.02
            };
            if !flipped {
                continue;
            }
            self.high = !self.high;
            let run = std::mem::take(&mut self.run);
            if let Some(timecode) = self.interval(run) {
                found = Some(timecode);
            }
        }
        found
    }

    /// One interval between crossings, of `run` samples.
    fn interval(&mut self, run: f64) -> Option<Timecode> {
        // Far off the bit length: noise or silence, start over.
        if run < self.bit * 0.25 || run > self.bit * 1.6 {
            self.half = false;
            if run > self.bit * 1.6 && run < self.sample_rate / (20.0 * LTC_BITS as f64) * 1.6 {
                // Slower than expected: a lower frame rate.
                self.bit = run;
            }
            return None;
        }
        if run < self.bit * 0.75 {
            self.bit = self.bit * 0.95 + run * 2.0 * 0.05;
            if self.half {
                self.half = false;
                return self.bit_read(true);
            }
            self.half = true;
            None
        } else {
            self.bit = self.bit * 0.95 + run * 0.05;
            self.half = false;
            self.bit_read(false)
        }
    }

    fn bit_read(&mut self, bit: bool) -> Option<Timecode> {
        self.bits.push(bit);
        if self.bits.len() > LTC_BITS {
            self.bits.remove(0);
        }
        if self.bits.len() < LTC_BITS || self.bits[64..] != SYNC {
            return None;
        }
        let field = |start: usize, count: usize| {
            (0..count).fold(0u8, |value, index| {
                value | (u8::from(self.bits[start + index]) << index)
            })
        };
        let frames = field(0, 4) + 10 * field(8, 2);
        let drop = self.bits[10];
        let seconds = field(16, 4) + 10 * field(24, 3);
        let minutes = field(32, 4) + 10 * field(40, 3);
        let hours = field(48, 4) + 10 * field(56, 2);
        let fps = self.sample_rate / (self.bit * LTC_BITS as f64);
        let rate = if drop {
            TimecodeRate::Fps2997Drop
        } else if fps < 24.5 {
            TimecodeRate::Fps24
        } else if fps < 27.5 {
            TimecodeRate::Fps25
        } else {
            TimecodeRate::Fps30
        };
        self.bits.clear();
        let read = Timecode {
            hours,
            minutes,
            seconds,
            frames,
            rate,
        };
        if u32::from(frames) >= rate.nominal() || seconds > 59 || minutes > 59 || hours > 23 {
            return None;
        }
        // The frame's bits are all in: its time is now one frame later.
        Some(Timecode::from_seconds(
            read.seconds() + 1.0 / rate.real(),
            rate,
        ))
    }
}

/// The 80 bits of the LTC frame of `timecode`, in the order they are sent.
pub fn ltc_bits(timecode: Timecode) -> [bool; LTC_BITS] {
    let mut bits = [false; LTC_BITS];
    let mut set = |start: usize, count: usize, value: u8| {
        for index in 0..count {
            bits[start + index] = (value >> index) & 1 == 1;
        }
    };
    set(0, 4, timecode.frames % 10);
    set(8, 2, timecode.frames / 10);
    set(16, 4, timecode.seconds % 10);
    set(24, 3, timecode.seconds / 10);
    set(32, 4, timecode.minutes % 10);
    set(40, 3, timecode.minutes / 10);
    set(48, 4, timecode.hours % 10);
    set(56, 2, timecode.hours / 10);
    bits[10] = timecode.rate == TimecodeRate::Fps2997Drop;
    bits[64..].copy_from_slice(&SYNC);
    bits
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The audio of `frames` LTC frames from `start`, at `sample_rate`.
    fn signal(start: Timecode, frames: u32, sample_rate: u32) -> Vec<f32> {
        let rate = start.rate;
        let per_bit = f64::from(sample_rate) / (rate.real() * LTC_BITS as f64);
        let mut level = 0.5f32;
        let mut samples = Vec::new();
        let mut position = 0.0f64;
        for frame in 0..frames {
            let timecode = Timecode::from_seconds(
                start.seconds() + f64::from(frame) / rate.real() + 1e-6,
                rate,
            );
            for bit in ltc_bits(timecode) {
                // A flip at the start of every bit, one more in the middle
                // of a one.
                level = -level;
                let half = per_bit / 2.0;
                let end = position + half;
                while (samples.len() as f64) < end {
                    samples.push(level);
                }
                if bit {
                    level = -level;
                }
                position += per_bit;
                while (samples.len() as f64) < position {
                    samples.push(level);
                }
            }
        }
        // The flip starting the next frame ends the last bit.
        samples.extend(std::iter::repeat_n(-level, 4));
        samples
    }

    #[test]
    fn ltc_audio_gives_the_timecode() {
        for rate in [
            TimecodeRate::Fps25,
            TimecodeRate::Fps30,
            TimecodeRate::Fps24,
        ] {
            let start = Timecode::parse("10:59:58:00", rate).unwrap();
            let audio = signal(start, 10, 48_000);
            let mut decoder = LtcDecoder::new(48_000);
            let mut last = None;
            // In chunks, as an audio callback gives them.
            for chunk in audio.chunks(256) {
                if let Some(timecode) = decoder.push(chunk) {
                    last = Some(timecode);
                }
            }
            // Ten frames from 10:59:58:00: the last one ends at :10.
            let last = last.unwrap_or_else(|| panic!("nothing read at {rate:?}"));
            assert_eq!(last.rate, rate);
            assert_eq!(last.frames, 10, "{rate:?}: {last}");
            assert_eq!((last.hours, last.minutes, last.seconds), (10, 59, 58));
        }
    }

    #[test]
    fn noise_reads_nothing() {
        let noise: Vec<f32> = (0..48_000)
            .map(|index| ((index * 7919 % 101) as f32 / 50.0) - 1.0)
            .collect();
        let mut decoder = LtcDecoder::new(48_000);
        assert_eq!(decoder.push(&noise), None);
    }
}
