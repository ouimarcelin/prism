//! Cues on a timecode: what to launch when the timecode received passes
//! a given time.

use serde::{Deserialize, Serialize};

use crate::{ColumnId, Slot};

/// What a cue launches.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CueTarget {
    /// Every clip of a column.
    Column {
        /// The column.
        column: ColumnId,
    },
    /// One clip.
    Clip {
        /// Where the clip is.
        slot: Slot,
    },
    /// Stops every layer.
    StopAll,
}

/// A time of the timecode and what to launch then.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cue {
    /// Seconds of timecode.
    pub at: f64,
    /// What to launch.
    pub target: CueTarget,
}

/// Longest step of the timecode between two looks still taken as playing
/// on: a longer step, or a step back, is a jump, which launches nothing.
pub const MAX_TIMECODE_STEP: f64 = 1.0;

/// Cues launched by the timecode.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Timeline {
    /// Whether the cues follow the timecode received.
    pub enabled: bool,
    /// The cues, in any order.
    pub cues: Vec<Cue>,
}

impl Timeline {
    /// Whether there is nothing to keep.
    pub fn is_unused(&self) -> bool {
        !self.enabled && self.cues.is_empty()
    }

    /// The cues passed when the timecode moves from `from` to `to`, in
    /// time order: those after `from` up to `to` included, when the
    /// timecode plays on; none on a jump.
    pub fn passed(&self, from: f64, to: f64) -> Vec<Cue> {
        if to <= from || to - from > MAX_TIMECODE_STEP {
            return Vec::new();
        }
        let mut passed: Vec<Cue> = self
            .cues
            .iter()
            .filter(|cue| cue.at > from && cue.at <= to)
            .copied()
            .collect();
        passed.sort_by(|a, b| a.at.total_cmp(&b.at));
        passed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cues_fire_once_when_passed_and_not_on_jumps() {
        let timeline = Timeline {
            enabled: true,
            cues: vec![
                Cue {
                    at: 10.0,
                    target: CueTarget::StopAll,
                },
                Cue {
                    at: 9.5,
                    target: CueTarget::Column {
                        column: ColumnId(1),
                    },
                },
            ],
        };
        assert!(timeline.passed(9.0, 9.4).is_empty());
        let passed = timeline.passed(9.4, 10.0);
        assert_eq!(passed.len(), 2);
        assert_eq!(passed[0].at, 9.5);
        assert!(timeline.passed(10.0, 10.04).is_empty());
        // Jumps: back, or far ahead.
        assert!(timeline.passed(11.0, 9.6).is_empty());
        assert!(timeline.passed(0.0, 60.0).is_empty());
    }
}
