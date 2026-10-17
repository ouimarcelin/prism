//! Clips: a media source and its playback settings.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{Effect, Unit};

/// Number of cue points available on each clip.
pub const CUE_POINT_COUNT: usize = 8;

/// Highest playback speed multiplier.
pub const MAX_SPEED: f32 = 10.0;

/// Sliders on the dashboard of a clip.
pub const DASHBOARD_SIZE: usize = 8;

/// Frame rate given to a new image sequence.
pub const DEFAULT_SEQUENCE_RATE: f64 = 30.0;

/// Where the frames of a clip come from.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MediaSource {
    /// A video or image file. Absolute in memory; stored relative to the
    /// project file when it sits in the project folder.
    File {
        /// Location of the file.
        path: PathBuf,
    },
    /// A folder of PNG images, played in name order.
    ImageSequence {
        /// Location of the folder.
        path: PathBuf,
        /// Images shown per second.
        frame_rate: f64,
    },
    /// An image drawn by the GPU, frame after frame, with no media file:
    /// a solid color, a gradient, lines… Its kind, settings and animations
    /// are those of an effect without input.
    Generator {
        /// What to draw and how.
        generator: Effect,
    },
    /// A camera of the computer (or a capture device seen as one), live.
    Camera {
        /// Identifier of the device, stable across restarts.
        id: String,
        /// Name of the device when it was chosen.
        name: String,
    },
}

impl MediaSource {
    /// Path of the file or folder behind this source.
    pub fn path(&self) -> Option<&Path> {
        match self {
            Self::File { path } | Self::ImageSequence { path, .. } => Some(path),
            Self::Generator { .. } | Self::Camera { .. } => None,
        }
    }

    /// Mutable path of the file or folder behind this source.
    pub fn path_mut(&mut self) -> Option<&mut PathBuf> {
        match self {
            Self::File { path } | Self::ImageSequence { path, .. } => Some(path),
            Self::Generator { .. } | Self::Camera { .. } => None,
        }
    }

    /// Whether the frames come live from a device, without position or
    /// duration.
    pub fn is_live(&self) -> bool {
        matches!(self, Self::Camera { .. })
    }

    /// The generator, for a clip drawn by the GPU.
    pub fn generator(&self) -> Option<&Effect> {
        match self {
            Self::Generator { generator } => Some(generator),
            _ => None,
        }
    }

    /// The generator, mutable.
    pub fn generator_mut(&mut self) -> Option<&mut Effect> {
        match self {
            Self::Generator { generator } => Some(generator),
            _ => None,
        }
    }

    /// Whether both sources play the same thing: the same file or folder,
    /// or generators of the same kind, whatever their settings.
    pub fn same_media(&self, other: &Self) -> bool {
        match (self.generator(), other.generator()) {
            (Some(a), Some(b)) => a.kind == b.kind,
            (None, None) => self == other,
            _ => false,
        }
    }
}

/// Playback direction of a clip.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    /// Plays forward.
    #[default]
    Forward,
    /// Plays backward.
    Backward,
    /// Jumps to a random frame on each step.
    Random,
    /// Holds the current frame.
    Paused,
}

/// What happens when playback reaches the in or out point.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoopMode {
    /// Jumps back to the other end and continues.
    #[default]
    Loop,
    /// Reverses direction.
    PingPong,
    /// Stops on the last frame.
    Once,
}

/// Playback settings of a clip. Times are in seconds from the media start.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Playback {
    /// Playback direction.
    pub direction: Direction,
    /// Speed multiplier; see [`Playback::effective_speed`].
    pub speed: f32,
    /// Behavior at the in and out points.
    pub loop_mode: LoopMode,
    /// Start of the played range.
    pub in_point: f64,
    /// End of the played range; `None` means the end of the media.
    pub out_point: Option<f64>,
    /// Cue points, each one set or empty.
    pub cue_points: [Option<f64>; CUE_POINT_COUNT],
    /// Volume of the clip's sound; the opacity of its layer scales it too.
    pub volume: Unit,
}

impl Default for Playback {
    fn default() -> Self {
        Self {
            direction: Direction::default(),
            speed: 1.0,
            loop_mode: LoopMode::default(),
            in_point: 0.0,
            out_point: None,
            cue_points: [None; CUE_POINT_COUNT],
            volume: Unit::ONE,
        }
    }
}

impl Playback {
    /// Speed clamped to `0.0..=MAX_SPEED`. NaN gives the normal speed `1.0`.
    pub fn effective_speed(&self) -> f32 {
        if self.speed.is_nan() {
            1.0
        } else {
            self.speed.clamp(0.0, MAX_SPEED)
        }
    }
}

/// A clip placed in a slot of the grid.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Clip {
    /// Name shown in the grid.
    pub name: String,
    /// Media played by the clip.
    pub source: MediaSource,
    /// Playback settings.
    #[serde(default)]
    pub playback: Playback,
    /// Effects applied to the clip, first to last.
    #[serde(default)]
    pub effects: Vec<Effect>,
    /// Sliders the settings of the clip's effects, and of its layer's, can
    /// follow: each from `0.0` to `1.0`.
    #[serde(default)]
    pub dashboard: [Unit; DASHBOARD_SIZE],
}

impl Clip {
    /// Creates a clip for a media file, named after the file.
    pub fn from_file(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let name = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_default();
        Self {
            name,
            source: MediaSource::File { path },
            playback: Playback::default(),
            effects: Vec::new(),
            dashboard: [Unit::ZERO; DASHBOARD_SIZE],
        }
    }

    /// Creates a clip showing the camera `id`, named `name`.
    pub fn from_camera(id: impl Into<String>, name: impl Into<String>) -> Self {
        let name = name.into();
        Self {
            name: name.clone(),
            source: MediaSource::Camera {
                id: id.into(),
                name,
            },
            playback: Playback::default(),
            effects: Vec::new(),
            dashboard: [Unit::ZERO; DASHBOARD_SIZE],
        }
    }

    /// Creates a clip drawn by a generator of `kind`, every setting at its
    /// default.
    pub fn from_generator(name: impl Into<String>, kind: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            source: MediaSource::Generator {
                generator: Effect::new(kind),
            },
            playback: Playback::default(),
            effects: Vec::new(),
            dashboard: [Unit::ZERO; DASHBOARD_SIZE],
        }
    }

    /// Creates a clip for a folder of PNG images, named after the folder.
    pub fn from_image_sequence(folder: impl Into<PathBuf>, frame_rate: f64) -> Self {
        let path = folder.into();
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        Self {
            name,
            source: MediaSource::ImageSequence { path, frame_rate },
            playback: Playback::default(),
            effects: Vec::new(),
            dashboard: [Unit::ZERO; DASHBOARD_SIZE],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generators_have_no_file_and_keep_their_settings() {
        let mut clip = Clip::from_generator("Dégradé", "gradient");
        assert_eq!(clip.source.path(), None);
        clip.source
            .generator_mut()
            .unwrap()
            .set_param("angle", 45.0);
        let json = serde_json::to_string(&clip).unwrap();
        let read: Clip = serde_json::from_str(&json).unwrap();
        assert_eq!(read, clip);

        // Settings changed, same generator; another kind, another one.
        let other = Clip::from_generator("Dégradé", "gradient");
        assert!(clip.source.same_media(&other.source));
        let lines = Clip::from_generator("Lignes", "lines");
        assert!(!clip.source.same_media(&lines.source));
        assert!(!clip.source.same_media(&Clip::from_file("/a.mov").source));
    }

    #[test]
    fn clip_is_named_after_its_file() {
        let clip = Clip::from_file("/media/loops/tunnel_blue.mov");
        assert_eq!(clip.name, "tunnel_blue");
        assert_eq!(
            clip.source.path(),
            Some(Path::new("/media/loops/tunnel_blue.mov"))
        );
    }

    #[test]
    fn image_sequences_are_saved_with_their_rate() {
        let clip = Clip::from_image_sequence("/media/smoke", 24.0);
        assert_eq!(clip.name, "smoke");
        let json = serde_json::to_value(&clip.source).unwrap();
        assert_eq!(json["kind"], "image_sequence");
        assert_eq!(json["frame_rate"], 24.0);
        let back: MediaSource = serde_json::from_value(json).unwrap();
        assert_eq!(back, clip.source);
        assert_eq!(back.path(), Some(Path::new("/media/smoke")));
    }

    #[test]
    fn effective_speed_is_bounded() {
        let mut playback = Playback::default();
        assert_eq!(playback.effective_speed(), 1.0);
        playback.speed = -2.0;
        assert_eq!(playback.effective_speed(), 0.0);
        playback.speed = 100.0;
        assert_eq!(playback.effective_speed(), MAX_SPEED);
        playback.speed = f32::NAN;
        assert_eq!(playback.effective_speed(), 1.0);
    }

    #[test]
    fn missing_playback_fields_take_defaults() {
        let clip: Clip = serde_json::from_str(
            r#"{"name":"a","source":{"kind":"file","path":"a.mov"},"playback":{"speed":2.0}}"#,
        )
        .unwrap();
        assert_eq!(clip.playback.speed, 2.0);
        assert_eq!(clip.playback.loop_mode, LoopMode::Loop);
        assert_eq!(clip.playback.cue_points, [None; CUE_POINT_COUNT]);
    }
}
