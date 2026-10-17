//! Prism control interface, drawn with egui.
//!
//! The [`ControlPanel`] owns the [`Document`] being edited: the clip grid
//! (layers × columns), layer and clip properties, effects, the tempo, the
//! crossfader, project
//! files and media import. Each frame it returns [`Action`]s the application
//! carries out: commands for the engine, opening or closing the output,
//! quitting. The application sends the composition to the engine whenever
//! [`Document::revision`] changes.

mod crossfade;
mod dialogs;
mod document;
mod effects;
mod grid;
mod inspector;
mod labels;
mod learn;
mod panel;
mod preferences;
mod presets;
mod shortcuts;
mod stage;

pub use document::{Document, IMAGE_EXTENSIONS, VIDEO_EXTENSIONS, is_image, is_video};
pub use panel::{Action, AudioStatus, ControlPanel, FrameInfo, MidiStatus, Screen};
pub use preferences::{MAX_RECENT, Preferences};
pub use shortcuts::{SHORTCUT_HELP, Shortcut};
pub use stage::StageFrame;

/// Message of an error and of all its causes.
pub(crate) fn describe(error: &dyn std::error::Error) -> String {
    let mut message = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        message.push_str(" : ");
        message.push_str(&cause.to_string());
        source = cause.source();
    }
    message
}
