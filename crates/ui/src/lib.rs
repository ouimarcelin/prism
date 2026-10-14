//! Prism control interface, drawn with egui.
//!
//! The [`ControlPanel`] owns the [`Document`] being edited: the clip grid
//! (layers × columns), layer and clip properties, the crossfader, project
//! files and media import. Each frame it returns [`Action`]s the application
//! carries out: commands for the engine, opening or closing the output,
//! quitting. The application sends the composition to the engine whenever
//! [`Document::revision`] changes.

mod dialogs;
mod document;
mod grid;
mod inspector;
mod labels;
mod panel;
mod shortcuts;

pub use document::{Document, IMAGE_EXTENSIONS, VIDEO_EXTENSIONS, is_image, is_video};
pub use panel::{Action, ControlPanel, FrameInfo, Screen};
pub use shortcuts::{SHORTCUT_HELP, Shortcut};

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
