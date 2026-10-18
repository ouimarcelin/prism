//! Effects of Prism: image treatments run on the GPU.
//!
//! The [`CATALOG`] lists the effects this build knows, with their settings,
//! and [`GENERATORS`] the effects that draw an image of their own: the
//! first pass of a generated clip.
//! An [`EffectChain`] applies a list of [`prism_core::Effect`]s to an image
//! of a clip or of the composition, one full-screen pass after the other,
//! with the pipelines of an [`EffectRenderer`] shared by every chain.
//!
//! Each effect is a WGSL fragment shader after a common prelude
//! (`shaders/prelude.wgsl`): it reads the image so far and its settings,
//! and writes the new image. Colors stay premultiplied, in display space.

mod catalog;
mod chain;
mod isf;
mod plugins;
mod text;

pub use catalog::{
    CATALOG, EffectInfo, GENERATORS, MAX_PARAMS, ParamInfo, find, find_generator, info_for,
};
pub use chain::{EffectChain, EffectRenderer, FrameTime, is_active};
pub use isf::{ISF_KIND, IsfError, IsfShader, compile as compile_isf, load as load_isf, shader_of};
pub use plugins::{PLUGIN_EXTENSIONS, Plugin, scan_plugins};
pub use text::TEXT_KIND;
