//! Draws a patch: each node of the graph in turn, generators and effects
//! by their own chain, mixes by a compositor of their own.

use std::collections::HashMap;

use prism_core::{BlendMode, Effect, NodeId, NodeKind, Patch, Resolution};
use prism_fx::{EffectChain, EffectRenderer, FrameTime};
use prism_render::{Compositor, Gpu, LayerInput, WORKING_FORMAT, wgpu};

/// What a mix node draws with.
struct Mixer {
    compositor: Compositor,
    view: wgpu::TextureView,
    size: (u32, u32),
}

/// The images of the nodes of one patch, kept from frame to frame (so that
/// generators with memory keep theirs).
#[derive(Default)]
pub(crate) struct PatchRender {
    chains: HashMap<NodeId, EffectChain>,
    mixers: HashMap<NodeId, Mixer>,
    output: Option<wgpu::TextureView>,
}

impl PatchRender {
    /// The image of the output, after the last [`PatchRender::render`].
    pub(crate) fn output(&self) -> Option<&wgpu::TextureView> {
        self.output.as_ref()
    }

    /// Records the nodes leading to the output, at `size` pixels. `blank`
    /// stands for an input wired to nothing.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn render(
        &mut self,
        renderer: &EffectRenderer,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        patch: &Patch,
        time: FrameTime,
        blank: &wgpu::TextureView,
        size: (u32, u32),
    ) {
        // Nodes removed from the graph let go of their images.
        self.chains.retain(|id, _| patch.node(*id).is_some());
        self.mixers.retain(|id, _| patch.node(*id).is_some());
        let mut images: HashMap<NodeId, wgpu::TextureView> = HashMap::new();
        let input = |images: &HashMap<NodeId, wgpu::TextureView>, node: NodeId, index: u8| {
            patch
                .source_of(node, index)
                .and_then(|source| images.get(&source).cloned())
        };
        self.output = None;
        for id in patch.order() {
            let Some(node) = patch.node(id) else {
                continue;
            };
            let image = match &node.kind {
                NodeKind::Generator { generator } => {
                    let chain = self.chains.entry(id).or_default();
                    draw(chain, renderer, gpu, encoder, generator, blank, size, time)
                }
                NodeKind::Effect { effect } => {
                    let source = input(&images, id, 0);
                    let chain = self.chains.entry(id).or_default();
                    match &source {
                        Some(source) => {
                            draw(chain, renderer, gpu, encoder, effect, source, size, time)
                                .or(Some(source.clone()))
                        }
                        None => None,
                    }
                }
                NodeKind::Mix { blend, amount } => {
                    let first = input(&images, id, 0);
                    let second = input(&images, id, 1);
                    let mixer = self
                        .mixers
                        .entry(id)
                        .or_insert_with(|| Mixer::new(&gpu.device, size));
                    if mixer.size != size {
                        *mixer = Mixer::new(&gpu.device, size);
                    }
                    let mut layers = Vec::new();
                    if let Some(first) = &first {
                        layers.push(LayerInput::new(first, 1.0, BlendMode::Normal));
                    }
                    if let Some(second) = &second {
                        layers.push(LayerInput::new(second, amount.get(), *blend));
                    }
                    mixer
                        .compositor
                        .encode(&gpu.device, &gpu.queue, encoder, &layers, &mixer.view);
                    Some(mixer.view.clone())
                }
                NodeKind::Output => {
                    self.output = input(&images, id, 0);
                    None
                }
            };
            if let Some(image) = image {
                images.insert(id, image);
            }
        }
    }
}

/// Runs `effect` alone on `input` through `chain`; its image, if it drew.
#[allow(clippy::too_many_arguments)]
fn draw(
    chain: &mut EffectChain,
    renderer: &EffectRenderer,
    gpu: &Gpu,
    encoder: &mut wgpu::CommandEncoder,
    effect: &Effect,
    input: &wgpu::TextureView,
    size: (u32, u32),
    time: FrameTime,
) -> Option<wgpu::TextureView> {
    chain
        .apply(
            renderer,
            &gpu.device,
            &gpu.queue,
            encoder,
            &[effect],
            input,
            size,
            time,
        )
        .cloned()
}

impl Mixer {
    fn new(device: &wgpu::Device, size: (u32, u32)) -> Self {
        let resolution = Resolution {
            width: size.0.max(1),
            height: size.1.max(1),
        };
        let view = device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("patch mix"),
                size: wgpu::Extent3d {
                    width: resolution.width,
                    height: resolution.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: WORKING_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            compositor: Compositor::new(device, resolution).over_transparent(),
            view,
            size,
        }
    }
}
