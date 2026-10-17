//! SMPTE timecode: hours, minutes, seconds and frames at a frame rate, and
//! its reading from MIDI timecode (MTC).
//!
//! MTC sends the timecode in eight quarter-frame messages (`0xF1`, then a
//! byte whose high nibble names the piece and whose low nibble carries it),
//! two frames' worth of time for a full timecode, or at once in a full
//! frame message (`F0 7F 7F 01 01 hh mm ss ff F7`) when the sender jumps.
//! The hours byte carries the frame rate in its bits 5 and 6.

use std::fmt;

/// Frame rate of a timecode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TimecodeRate {
    /// 24 frames per second (film).
    Fps24,
    /// 25 frames per second (PAL).
    #[default]
    Fps25,
    /// 29.97 frames per second, drop frame (NTSC).
    Fps2997Drop,
    /// 30 frames per second.
    Fps30,
}

impl TimecodeRate {
    /// The rate coded on two bits in MTC.
    pub fn from_mtc(bits: u8) -> Self {
        match bits & 0b11 {
            0 => Self::Fps24,
            1 => Self::Fps25,
            2 => Self::Fps2997Drop,
            _ => Self::Fps30,
        }
    }

    /// Frames counted per second (30 for 29.97 drop frame).
    pub fn nominal(self) -> u32 {
        match self {
            Self::Fps24 => 24,
            Self::Fps25 => 25,
            Self::Fps2997Drop | Self::Fps30 => 30,
        }
    }

    /// Frames per second of real time.
    pub fn real(self) -> f64 {
        match self {
            Self::Fps2997Drop => 30_000.0 / 1001.0,
            rate => f64::from(rate.nominal()),
        }
    }
}

/// A SMPTE timecode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Timecode {
    /// Hours, 0 to 23.
    pub hours: u8,
    /// Minutes, 0 to 59.
    pub minutes: u8,
    /// Seconds, 0 to 59.
    pub seconds: u8,
    /// Frames, from 0.
    pub frames: u8,
    /// Frame rate.
    pub rate: TimecodeRate,
}

impl Timecode {
    /// Frames counted since 00:00:00:00 (drop-frame numbers skipped).
    pub fn frame_count(&self) -> u64 {
        let nominal = u64::from(self.rate.nominal());
        let minutes = u64::from(self.hours) * 60 + u64::from(self.minutes);
        let mut frames =
            (minutes * 60 + u64::from(self.seconds)) * nominal + u64::from(self.frames);
        if self.rate == TimecodeRate::Fps2997Drop {
            // Two frame numbers dropped every minute, except every tenth.
            frames -= 2 * (minutes - minutes / 10);
        }
        frames
    }

    /// Seconds of real time since 00:00:00:00.
    pub fn seconds(&self) -> f64 {
        self.frame_count() as f64 / self.rate.real()
    }

    /// The timecode at `seconds` of real time, at `rate`.
    pub fn from_seconds(seconds: f64, rate: TimecodeRate) -> Self {
        let mut frames = (seconds.max(0.0) * rate.real()).floor() as u64;
        let nominal = u64::from(rate.nominal());
        if rate == TimecodeRate::Fps2997Drop {
            // Back to frame numbers: add the dropped ones.
            let per_ten_minutes = 17_982;
            let per_minute = 1798;
            let tens = frames / per_ten_minutes;
            let rest = frames % per_ten_minutes;
            frames += 18 * tens;
            if rest > 2 {
                frames += 2 * ((rest - 2) / per_minute);
            }
        }
        let total_seconds = frames / nominal;
        Self {
            hours: ((total_seconds / 3600) % 24) as u8,
            minutes: ((total_seconds / 60) % 60) as u8,
            seconds: (total_seconds % 60) as u8,
            frames: (frames % nominal) as u8,
            rate,
        }
    }

    /// Reads `hh:mm:ss:ff` (or `;` before the frames); `None` if it is not
    /// a timecode at `rate`.
    pub fn parse(text: &str, rate: TimecodeRate) -> Option<Self> {
        let parts: Vec<&str> = text.trim().split([':', ';', '.']).collect();
        let [hours, minutes, seconds, frames] = parts.as_slice() else {
            return None;
        };
        let number = |text: &str| text.trim().parse::<u8>().ok();
        let timecode = Self {
            hours: number(hours)?,
            minutes: number(minutes)?,
            seconds: number(seconds)?,
            frames: number(frames)?,
            rate,
        };
        (timecode.hours < 24
            && timecode.minutes < 60
            && timecode.seconds < 60
            && u32::from(timecode.frames) < rate.nominal())
        .then_some(timecode)
    }
}

impl fmt::Display for Timecode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let separator = if self.rate == TimecodeRate::Fps2997Drop {
            ';'
        } else {
            ':'
        };
        write!(
            formatter,
            "{:02}:{:02}:{:02}{separator}{:02}",
            self.hours, self.minutes, self.seconds, self.frames
        )
    }
}

/// Assembles MTC messages into timecodes.
#[derive(Clone, Debug, Default)]
pub struct MtcReader {
    /// Nibbles of the quarter frames received, by piece.
    pieces: [Option<u8>; 8],
    last_piece: Option<u8>,
}

impl MtcReader {
    /// Reads one MIDI message; returns the timecode it completes, if any.
    pub fn read(&mut self, bytes: &[u8]) -> Option<Timecode> {
        match bytes {
            [0xF1, data, ..] => self.quarter_frame(*data),
            [
                0xF0,
                0x7F,
                _,
                0x01,
                0x01,
                hours,
                minutes,
                seconds,
                frames,
                ..,
            ] => {
                self.pieces = [None; 8];
                self.last_piece = None;
                Some(Timecode {
                    hours: hours & 0x1F,
                    minutes: minutes & 0x3F,
                    seconds: seconds & 0x3F,
                    frames: frames & 0x1F,
                    rate: TimecodeRate::from_mtc(hours >> 5),
                })
            }
            _ => None,
        }
    }

    fn quarter_frame(&mut self, data: u8) -> Option<Timecode> {
        let piece = (data >> 4) & 0x07;
        // Pieces must come in order; a gap starts over.
        if piece != 0 && self.last_piece != Some(piece - 1) {
            self.pieces = [None; 8];
            self.last_piece = None;
            return None;
        }
        self.pieces[usize::from(piece)] = Some(data & 0x0F);
        self.last_piece = Some(piece);
        if piece != 7 {
            return None;
        }
        let nibble = |index: usize| self.pieces[index];
        let frames = nibble(0)? | (nibble(1)? & 0x01) << 4;
        let seconds = nibble(2)? | (nibble(3)? & 0x03) << 4;
        let minutes = nibble(4)? | (nibble(5)? & 0x03) << 4;
        let hours = nibble(6)? | (nibble(7)? & 0x01) << 4;
        let rate = TimecodeRate::from_mtc(nibble(7)? >> 1);
        self.pieces = [None; 8];
        self.last_piece = None;
        // The eight pieces took two frames to arrive: the timecode they
        // carry is two frames old.
        let timecode = Timecode {
            hours,
            minutes,
            seconds,
            frames,
            rate,
        };
        Some(Timecode::from_seconds(
            timecode.seconds() + 2.0 / rate.real(),
            rate,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The eight quarter frames of `timecode`.
    fn quarter_frames(timecode: Timecode) -> Vec<[u8; 2]> {
        let rate = match timecode.rate {
            TimecodeRate::Fps24 => 0,
            TimecodeRate::Fps25 => 1,
            TimecodeRate::Fps2997Drop => 2,
            TimecodeRate::Fps30 => 3,
        };
        let values = [
            timecode.frames & 0x0F,
            timecode.frames >> 4,
            timecode.seconds & 0x0F,
            timecode.seconds >> 4,
            timecode.minutes & 0x0F,
            timecode.minutes >> 4,
            timecode.hours & 0x0F,
            (timecode.hours >> 4) | (rate << 1),
        ];
        values
            .iter()
            .enumerate()
            .map(|(piece, value)| [0xF1, ((piece as u8) << 4) | value])
            .collect()
    }

    #[test]
    fn timecodes_count_real_time() {
        let timecode = Timecode {
            hours: 1,
            minutes: 2,
            seconds: 3,
            frames: 12,
            rate: TimecodeRate::Fps25,
        };
        assert_eq!(timecode.seconds(), 3723.48);
        assert_eq!(
            Timecode::from_seconds(3723.48, TimecodeRate::Fps25),
            timecode
        );
        assert_eq!(timecode.to_string(), "01:02:03:12");
        assert_eq!(
            Timecode::parse("01:02:03:12", TimecodeRate::Fps25),
            Some(timecode)
        );
        assert_eq!(Timecode::parse("01:02:03:25", TimecodeRate::Fps25), None);
        assert_eq!(Timecode::parse("douze", TimecodeRate::Fps25), None);
    }

    #[test]
    fn drop_frame_skips_two_numbers_a_minute() {
        let rate = TimecodeRate::Fps2997Drop;
        // 00:01:00;02 is the first frame of minute 1: frame 1800.
        let first = Timecode::parse("00:01:00;02", rate).unwrap();
        assert_eq!(first.frame_count(), 1800);
        // Minute 10 drops nothing.
        let tenth = Timecode::parse("00:10:00;00", rate).unwrap();
        assert_eq!(tenth.frame_count(), 17_982);
        for timecode in [first, tenth] {
            assert_eq!(
                Timecode::from_seconds(timecode.seconds() + 1e-6, rate),
                timecode
            );
        }
    }

    #[test]
    fn quarter_frames_and_full_frames_give_the_timecode() {
        let timecode = Timecode {
            hours: 10,
            minutes: 20,
            seconds: 30,
            frames: 20,
            rate: TimecodeRate::Fps25,
        };
        let mut reader = MtcReader::default();
        let mut read = None;
        for message in quarter_frames(timecode) {
            read = reader.read(&message);
        }
        // Two frames later than the pieces say.
        assert_eq!(read.map(|read| read.frames), Some(22));
        // Out of order: nothing.
        assert_eq!(reader.read(&[0xF1, 0x30]), None);
        let full = [0xF0, 0x7F, 0x7F, 0x01, 0x01, 0x20 | 10, 20, 30, 20, 0xF7];
        assert_eq!(reader.read(&full), Some(timecode));
    }
}
