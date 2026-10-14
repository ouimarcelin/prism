//! State of the engine, published for the interface after every frame.

use prism_core::{LayerId, Slot};

/// Playback state of the clip of a layer.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ClipState {
    /// No clip on the layer.
    #[default]
    Idle,
    /// The file is opening or the first frame is decoding.
    Loading,
    /// Frames are on screen.
    Playing,
    /// A clip played once reached its end and holds its last frame.
    Finished,
    /// The clip cannot play.
    Error(String),
}

/// State of one layer.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LayerStatus {
    /// Layer described.
    pub layer: Option<LayerId>,
    /// Slot of the clip playing, if any.
    pub slot: Option<Slot>,
    /// Playback state of that clip.
    pub state: ClipState,
    /// Playhead position, in seconds from the start of the media.
    pub position: f64,
    /// Media duration, in seconds.
    pub duration: f64,
    /// Frame on screen.
    pub frame: Option<usize>,
    /// Number of frames of the media.
    pub frame_count: usize,
    /// Whether the playhead is held (scratching).
    pub held: bool,
    /// Progress of a fade between two clips, if one is running.
    pub transition: Option<f32>,
    /// Frames shown late since the clip started: the wanted frame was not
    /// decoded in time and the previous one stayed on screen.
    pub late_frames: u64,
    /// Frames that failed to decode since the clip started.
    pub frame_errors: u64,
}

/// Time the render thread spent on each step of one frame, in milliseconds.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FrameSteps {
    /// Collecting decoded frames and moving the playheads.
    pub playback_ms: f64,
    /// Copying decoded frames to the GPU and recording their conversion.
    pub upload_ms: f64,
    /// Recording the mix and submitting the frame to the GPU.
    pub mix_ms: f64,
    /// Presenting the program to the outputs.
    pub present_ms: f64,
}

impl FrameSteps {
    /// Total time of the frame.
    pub fn total_ms(&self) -> f64 {
        self.playback_ms + self.upload_ms + self.mix_ms + self.present_ms
    }

    /// Longest time of each step between `self` and `other`.
    pub fn max(self, other: Self) -> Self {
        Self {
            playback_ms: self.playback_ms.max(other.playback_ms),
            upload_ms: self.upload_ms.max(other.upload_ms),
            mix_ms: self.mix_ms.max(other.mix_ms),
            present_ms: self.present_ms.max(other.present_ms),
        }
    }
}

/// A stutter: a frame shown more than one and a half frame periods after
/// the previous one.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Hitch {
    /// Number of the late frame, counted from the start.
    pub frame: u64,
    /// Time since the previous frame, in milliseconds.
    pub interval_ms: f64,
    /// Steps of the previous frame. A long step there delayed this frame;
    /// short steps everywhere mean the thread woke up late.
    pub previous: FrameSteps,
}

/// State of the engine after its last frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EngineStatus {
    /// Frames rendered since the start.
    pub frames: u64,
    /// Frames rendered per second, over the last second.
    pub fps: f64,
    /// Average time spent preparing a frame, in milliseconds.
    pub frame_time_ms: f64,
    /// Frames started more than one frame period late since the start.
    pub late_ticks: u64,
    /// Longest time between two frames, in milliseconds, after the first
    /// two seconds.
    pub worst_interval_ms: f64,
    /// Times between two frames longer than one and a half frame periods,
    /// after the first two seconds: visible stutters.
    pub hitches: u64,
    /// Longest time spent preparing one frame, in milliseconds, after the
    /// first two seconds.
    pub worst_frame_time_ms: f64,
    /// Longest time of each step of a frame, after the first two seconds.
    pub worst_steps: FrameSteps,
    /// Last stutter, if any.
    pub last_hitch: Option<Hitch>,
    /// Whether the render thread runs with real-time scheduling.
    pub real_time: bool,
    /// Layers, bottom to top.
    pub layers: Vec<LayerStatus>,
}
