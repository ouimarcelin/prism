//! Prism outputs.
//!
//! An [`OutputSurface`] shows the program image in a window, fitted with
//! black bars or stretched. The render thread presents it after every frame
//! (it implements [`prism_engine::Output`]); the thread receiving window
//! events only updates its [`OutputShared`] state (size, visibility).

mod presenter;
mod surface;

pub use presenter::{Fit, Presenter, Viewport, fit_viewport};
pub use surface::{OutputError, OutputShared, OutputSurface};
