//! The control window: the egui interface drawn on the main thread, with the
//! program image shown as a texture shared with the engine.

use std::error::Error;
use std::sync::Arc;

use prism_core::Resolution;
use prism_engine::ProgramOutput;
use prism_render::{Gpu, wgpu};
use winit::event::WindowEvent;
use winit::window::Window;

/// Background behind the interface.
const BACKGROUND: wgpu::Color = wgpu::Color {
    r: 0.08,
    g: 0.08,
    b: 0.09,
    a: 1.0,
};

pub(crate) struct ControlWindow {
    pub(crate) window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    view_format: wgpu::TextureFormat,
    egui: egui::Context,
    state: egui_winit::State,
    renderer: egui_wgpu::Renderer,
    /// Engine images registered with the renderer.
    textures: Vec<(Arc<ProgramOutput>, egui::TextureId)>,
}

impl ControlWindow {
    pub(crate) fn new(
        gpu: &Gpu,
        window: Arc<Window>,
        surface: wgpu::Surface<'static>,
    ) -> Result<Self, Box<dyn Error>> {
        let size = window.inner_size();
        let mut config = surface
            .get_default_config(&gpu.adapter, size.width.max(1), size.height.max(1))
            .ok_or("le GPU ne peut pas afficher dans cette fenêtre")?;
        // Display-space values, as in the outputs: a non-sRGB view.
        let capabilities = surface.get_capabilities(&gpu.adapter);
        config.format = capabilities
            .formats
            .iter()
            .copied()
            .find(|format| !format.is_srgb())
            .unwrap_or(config.format);
        let view_format = config.format.remove_srgb_suffix();
        if view_format != config.format {
            config.view_formats.push(view_format);
        }
        config.present_mode = wgpu::PresentMode::AutoVsync;
        surface.configure(&gpu.device, &config);

        let egui = egui::Context::default();
        let state = egui_winit::State::new(
            egui.clone(),
            egui::ViewportId::ROOT,
            &window,
            Some(window.scale_factor() as f32),
            window.theme(),
            Some(gpu.device.limits().max_texture_dimension_2d as usize),
        );
        let renderer = egui_wgpu::Renderer::new(
            &gpu.device,
            view_format,
            egui_wgpu::RendererOptions {
                msaa_samples: 1,
                depth_stencil_format: None,
                dithering: false,
                predictable_texture_filtering: false,
            },
        );
        Ok(Self {
            window,
            surface,
            config,
            view_format,
            egui,
            state,
            renderer,
            textures: Vec::new(),
        })
    }

    /// Passes a window event to egui. Returns whether egui wants a new frame.
    pub(crate) fn on_event(&mut self, event: &WindowEvent) -> bool {
        self.state.on_window_event(&self.window, event).repaint
    }

    /// Draws one frame of the interface; `run` builds it, with the textures
    /// of `images` and their sizes, in the same order.
    pub(crate) fn draw(
        &mut self,
        gpu: &Gpu,
        images: &[Arc<ProgramOutput>],
        run: impl FnMut(&mut egui::Ui, &[(egui::TextureId, Resolution)]),
    ) {
        let mut run = run;
        let size = self.window.inner_size();
        if size.width == 0 || size.height == 0 {
            return;
        }
        if (size.width, size.height) != (self.config.width, self.config.height) {
            self.config.width = size.width;
            self.config.height = size.height;
            self.surface.configure(&gpu.device, &self.config);
        }

        // Images are replaced when their size changes: registered again.
        let mut kept = Vec::with_capacity(images.len());
        for image in images {
            let found = self
                .textures
                .iter()
                .position(|(registered, _)| Arc::ptr_eq(registered, image));
            let entry = match found {
                Some(index) => self.textures.swap_remove(index),
                None => {
                    let id = self.renderer.register_native_texture(
                        &gpu.device,
                        image.view(),
                        wgpu::FilterMode::Linear,
                    );
                    (Arc::clone(image), id)
                }
            };
            kept.push(entry);
        }
        for (_, id) in std::mem::replace(&mut self.textures, kept) {
            self.renderer.free_texture(&id);
        }
        let textures: Vec<(egui::TextureId, Resolution)> = self
            .textures
            .iter()
            .map(|(image, id)| (*id, image.resolution()))
            .collect();

        let input = self.state.take_egui_input(&self.window);
        let output = self.egui.run_ui(input, |ui| run(ui, &textures));
        self.state
            .handle_platform_output(&self.window, output.platform_output);
        let paint_jobs = self.egui.tessellate(output.shapes, output.pixels_per_point);
        let mut textures = output.textures_delta;
        for (id, deltas) in textures.set.drain() {
            for delta in deltas {
                self.renderer
                    .update_texture(&gpu.device, &gpu.queue, id, &delta);
            }
        }

        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => Some(frame),
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&gpu.device, &self.config);
                None
            }
            wgpu::CurrentSurfaceTexture::Timeout
            | wgpu::CurrentSurfaceTexture::Occluded
            | wgpu::CurrentSurfaceTexture::Validation => None,
        };
        if let Some(frame) = frame {
            let view = frame.texture.create_view(&wgpu::TextureViewDescriptor {
                format: Some(self.view_format),
                ..Default::default()
            });
            let screen = egui_wgpu::ScreenDescriptor {
                size_in_pixels: [self.config.width, self.config.height],
                pixels_per_point: output.pixels_per_point,
            };
            let mut encoder = gpu
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("interface"),
                });
            let buffers = self.renderer.update_buffers(
                &gpu.device,
                &gpu.queue,
                &mut encoder,
                &paint_jobs,
                &screen,
            );
            let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("interface"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(BACKGROUND),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            self.renderer
                .render(&mut pass.forget_lifetime(), &paint_jobs, &screen);
            gpu.queue
                .submit(buffers.into_iter().chain([encoder.finish()]));
            gpu.queue.present(frame);
        }
        for id in textures.free.drain() {
            self.renderer.free_texture(&id);
        }
    }
}
