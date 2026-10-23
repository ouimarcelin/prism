//! The image of the cell selected in the interface, drawn apart from the
//! program: the clip with its own effects, before it is launched.

use std::sync::Arc;

use prism_core::{BlendMode, Composition, Effect, Resolution, Slot};
use prism_fx::{EffectRenderer, FrameTime};
use prism_render::{Compositor, FrameUploader, Gpu, LayerInput, wgpu};

use crate::engine::ProgramOutput;
use crate::random::Random;
use crate::stream::ClipStream;

/// The preview of one cell: its own stream of the clip (separate from the
/// one its layer may be playing), and the image it is drawn into, at half
/// the composition size.
pub(crate) struct SlotPreview {
    slot: Slot,
    stream: Option<ClipStream>,
    image: Arc<ProgramOutput>,
    compositor: Compositor,
}

/// Size of the preview for a composition of `resolution`.
fn preview_size(resolution: Resolution) -> Resolution {
    Resolution {
        width: (resolution.width / 2).max(1),
        height: (resolution.height / 2).max(1),
    }
}

impl SlotPreview {
    pub(crate) fn new(device: &wgpu::Device, slot: Slot, resolution: Resolution) -> Self {
        let size = preview_size(resolution);
        Self {
            slot,
            stream: None,
            image: Arc::new(ProgramOutput::new(device, size)),
            compositor: Compositor::new(device, size).over_transparent(),
        }
    }

    pub(crate) fn slot(&self) -> Slot {
        self.slot
    }

    /// The image, once the clip has shown a frame.
    pub(crate) fn image(&self) -> Arc<ProgramOutput> {
        Arc::clone(&self.image)
    }

    /// Opens, keeps or drops the stream so that it plays the clip now in
    /// the cell; resizes the image with the composition.
    pub(crate) fn sync(
        &mut self,
        device: &wgpu::Device,
        composition: &Composition,
        launch: (f64, f64),
    ) {
        let size = preview_size(composition.resolution());
        if size != self.image.resolution() {
            self.image = Arc::new(ProgramOutput::new(device, size));
            self.compositor.resize(size);
        }
        let Some(clip) = composition.clip(self.slot) else {
            self.stream = None;
            return;
        };
        match &mut self.stream {
            Some(stream) if stream.plays(self.slot, &clip.source) => {
                stream.set_playback(&clip.playback)
            }
            _ => {
                self.stream = Some(ClipStream::open(
                    self.slot,
                    clip.source.clone(),
                    clip.playback.clone(),
                    launch,
                ));
            }
        }
    }

    /// Moves the playhead on by `elapsed` seconds.
    pub(crate) fn advance(&mut self, elapsed: f64, tick: f64, random: &mut Random) {
        if let Some(stream) = &mut self.stream {
            stream.receive();
            stream.advance(elapsed, tick, random);
        }
    }

    /// Records the upload of the frame to show.
    pub(crate) fn upload(
        &mut self,
        gpu: &Gpu,
        uploader: &mut FrameUploader,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        if let Some(stream) = &mut self.stream {
            stream.upload(gpu, uploader, encoder);
        }
    }

    /// Records the clip with its generator and effects, stretched into the
    /// image.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn draw(
        &mut self,
        renderer: &EffectRenderer,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        composition: &Composition,
        time: FrameTime,
        blank: &wgpu::TextureView,
    ) {
        let Some(stream) = &mut self.stream else {
            return;
        };
        let Some(clip) = composition.clip(self.slot) else {
            return;
        };
        let mut time = time;
        time.dashboard = clip.dashboard.map(prism_core::Unit::get);
        (time.launch_beat, time.launch_seconds) = stream.launched();
        match clip.source.generator() {
            Some(generator) if stream.is_generator() => {
                let effects: Vec<&Effect> =
                    std::iter::once(generator).chain(&clip.effects).collect();
                let size = self.image.resolution();
                stream.generate(
                    renderer,
                    gpu,
                    encoder,
                    &effects,
                    time,
                    blank,
                    (size.width, size.height),
                );
            }
            _ => {
                let effects: Vec<&Effect> = clip.effects.iter().collect();
                stream.apply_effects(renderer, gpu, encoder, &effects, time);
            }
        }
        if let Some(view) = stream.view() {
            self.compositor.encode(
                &gpu.device,
                &gpu.queue,
                encoder,
                &[LayerInput::new(view, 1.0, BlendMode::Normal)],
                self.image.view(),
            );
        }
    }
}
