//! GPU compositing of Prism layers.
//!
//! [`plan_layers`] selects, from a [`prism_core::Composition`], the layers to
//! draw and their effective opacity. The [`Compositor`] then mixes their
//! current frames on the GPU, bottom to top, with the layer blend modes.
//!
//! The crate re-exports [`wgpu`] so that every Prism crate uses the same
//! version.

mod compositor;
mod gpu;
mod plan;
pub mod reference;

pub use compositor::{Compositor, LayerInput};
pub use gpu::{Gpu, GpuError, ReadbackError, WORKING_FORMAT, read_texture, solid_color_texture};
pub use plan::{LayerPlan, plan_layers};
pub use wgpu;
