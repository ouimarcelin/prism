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
    /// Layers, bottom to top.
    pub layers: Vec<LayerStatus>,
}
