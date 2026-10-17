//! GPU compositing of Prism layers.
//!
//! [`plan_layers`] selects, from a [`prism_core::Composition`], the layers to
//! draw and their effective opacity. The [`Compositor`] then mixes their
//! current frames on the GPU, bottom to top, with the layer blend modes.
//!
//! [`FrameUploader`] turns decoded HAP frames into layer textures without
//! decompressing them on the CPU.
//!
//! The crate re-exports [`wgpu`] so that every Prism crate uses the same
//! version.

mod compositor;
mod gpu;
mod plan;
pub mod reference;
mod stage;
mod video;

pub use compositor::{Compositor, LayerInput, Transition};
pub use gpu::{
    Gpu, GpuError, ReadbackError, WORKING_FORMAT, pixels_texture, read_texture, solid_color_texture,
};
pub use plan::{LayerPlan, plan_layers};
pub use stage::{ScreenTarget, SliceVertex, StageRenderer, perspective_weights, slice_vertices};
pub use video::{FrameUploader, UploadError, VideoTexture};
pub use wgpu;
