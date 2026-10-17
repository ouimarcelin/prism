//! Presentation of the program image to a window.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use prism_core::ScreenId;

use prism_engine::{Output, OutputImages, ProgramOutput};
use prism_render::{Gpu, wgpu};

use crate::presenter::{Fit, Presenter};

/// Error raised while preparing a window for presentation.
#[derive(Debug, thiserror::Error)]
pub enum OutputError {
    /// The window surface cannot be created.
    #[error("cannot create the window surface")]
    Surface(#[from] wgpu::CreateSurfaceError),
    /// The GPU cannot present to this window.
    #[error("the GPU cannot present to this window")]
    Unsupported,
}

/// Window state written by the thread handling window events and read by
/// the render thread before each presentation.
#[derive(Debug)]
pub struct OutputShared {
    size: AtomicU64,
    visible: AtomicBool,
    /// Screen of the stage shown, plus one; 0 for the program.
    screen: AtomicU64,
}

impl OutputShared {
    fn new(width: u32, height: u32) -> Self {
        Self {
            size: AtomicU64::new(pack(width, height)),
            visible: AtomicBool::new(true),
            screen: AtomicU64::new(0),
        }
    }

    /// Shows the screen `screen` of the stage, or the program with `None`.
    pub fn set_screen(&self, screen: Option<ScreenId>) {
        let value = screen.map_or(0, |id| u64::from(id.0) + 1);
        self.screen.store(value, Ordering::Relaxed);
    }

    /// The screen of the stage shown; `None` for the program.
    pub fn screen(&self) -> Option<ScreenId> {
        let value = self.screen.load(Ordering::Relaxed);
        (value > 0).then(|| ScreenId((value - 1) as u32))
    }

    /// Records a new window size, in physical pixels.
    pub fn set_size(&self, width: u32, height: u32) {
        self.size.store(pack(width, height), Ordering::Relaxed);
    }

    /// Window size, in physical pixels.
    pub fn size(&self) -> (u32, u32) {
        let packed = self.size.load(Ordering::Relaxed);
        ((packed >> 32) as u32, packed as u32)
    }

    /// Records whether the window can be seen. Hidden windows are skipped:
    /// presenting to them could make the render thread wait.
    pub fn set_visible(&self, visible: bool) {
        self.visible.store(visible, Ordering::Relaxed);
    }

    /// Whether the window can be seen.
    pub fn is_visible(&self) -> bool {
        self.visible.load(Ordering::Relaxed)
    }
}

fn pack(width: u32, height: u32) -> u64 {
    (u64::from(width) << 32) | u64::from(height)
}

/// A window showing the program image, presented by the render thread.
pub struct OutputSurface {
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    view_format: wgpu::TextureFormat,
    presenter: Presenter,
    shared: Arc<OutputShared>,
    fit: Fit,
    reconfigure: bool,
}

impl OutputSurface {
    /// Prepares `surface`, of `width` × `height` physical pixels, with
    /// vertical synchronisation.
    pub fn new(
        gpu: &Gpu,
        surface: wgpu::Surface<'static>,
        width: u32,
        height: u32,
        fit: Fit,
    ) -> Result<Self, OutputError> {
        let mut config = surface
            .get_default_config(&gpu.adapter, width.max(1), height.max(1))
            .ok_or(OutputError::Unsupported)?;
        // The program holds display-space values: write them unchanged.
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
        Ok(Self {
            surface,
            config,
            view_format,
            presenter: Presenter::new(&gpu.device, view_format),
            shared: Arc::new(OutputShared::new(width, height)),
            fit,
            reconfigure: false,
        })
    }

    /// State to update from window events.
    pub fn shared(&self) -> Arc<OutputShared> {
        Arc::clone(&self.shared)
    }

    fn configure(&mut self, gpu: &Gpu) {
        self.surface.configure(&gpu.device, &self.config);
        self.reconfigure = false;
    }
}

impl Output for OutputSurface {
    fn present_images(&mut self, gpu: &Gpu, images: &OutputImages<'_>) {
        // A screen not drawn (yet) shows the program.
        let image = self
            .shared
            .screen()
            .and_then(|id| images.screen(id))
            .unwrap_or(images.program());
        self.present(gpu, image);
    }

    fn present(&mut self, gpu: &Gpu, program: &ProgramOutput) {
        if !self.shared.is_visible() {
            return;
        }
        let (width, height) = self.shared.size();
        if width == 0 || height == 0 {
            return;
        }
        if (width, height) != (self.config.width, self.config.height) {
            self.config.width = width;
            self.config.height = height;
            self.reconfigure = true;
        }
        if self.reconfigure {
            self.configure(gpu);
        }

        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) => frame,
            wgpu::CurrentSurfaceTexture::Suboptimal(frame) => {
                self.reconfigure = true;
                frame
            }
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.reconfigure = true;
                return;
            }
            wgpu::CurrentSurfaceTexture::Timeout
            | wgpu::CurrentSurfaceTexture::Occluded
            | wgpu::CurrentSurfaceTexture::Validation => return,
        };
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(self.view_format),
            ..Default::default()
        });
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("present"),
            });
        let resolution = program.resolution();
        self.presenter.encode(
            &gpu.device,
            &mut encoder,
            program.view(),
            (resolution.width, resolution.height),
            &view,
            (width, height),
            self.fit,
        );
        gpu.queue.submit([encoder.finish()]);
        gpu.queue.present(frame);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_state_round_trips() {
        let shared = OutputShared::new(1920, 1080);
        assert_eq!(shared.size(), (1920, 1080));
        shared.set_size(u32::MAX, 7);
        assert_eq!(shared.size(), (u32::MAX, 7));
        assert!(shared.is_visible());
        shared.set_visible(false);
        assert!(!shared.is_visible());
        assert_eq!(shared.screen(), None);
        shared.set_screen(Some(ScreenId(0)));
        assert_eq!(shared.screen(), Some(ScreenId(0)));
        shared.set_screen(None);
        assert_eq!(shared.screen(), None);
    }
}
