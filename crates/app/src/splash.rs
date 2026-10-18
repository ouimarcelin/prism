//! Transparent window showing the splash image, in motion, while the
//! engine and the control window start.
//!
//! The first frame is painted on the main thread; the next ones on a
//! thread of their own, since the start blocks the event loop. The GPU
//! opened for this window is handed over to the application.

use std::error::Error;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use prism_render::{Gpu, wgpu};
use resvg::tiny_skia::Pixmap;
use winit::dpi::{LogicalSize, PhysicalPosition};
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowId, WindowLevel};

use crate::splash_art::Art;

/// Side of the window, in logical pixels.
const SIDE: f64 = 640.0;

/// Time left to the window server to show the painted window before the
/// start blocks the event loop.
const SETTLE: Duration = Duration::from_millis(50);

/// Longest wait for the first paint, after which the start goes on anyway.
const PAINT_TIMEOUT: Duration = Duration::from_secs(1);

/// Shortest time the image shows, from its first frame, so that its motion
/// is seen even when the start is quick.
const SHORTEST: Duration = Duration::from_millis(1600);

/// Fade in and fade out, in seconds.
const FADE: f32 = 0.3;

pub struct Splash {
    window: Arc<Window>,
    gpu: Arc<Gpu>,
    /// Handed to the animation thread after the first frame.
    surface: Option<wgpu::Surface<'static>>,
    opened: Instant,
    painted: Option<Instant>,
    animation: Option<Animation>,
}

/// The thread drawing the frames after the first.
struct Animation {
    stop: Arc<AtomicBool>,
    thread: JoinHandle<()>,
}

/// How frames reach the window surface.
#[derive(Clone, Copy)]
struct Target {
    side: u32,
    bgra: bool,
    premultiplied: bool,
}

impl Splash {
    pub fn open(event_loop: &ActiveEventLoop) -> Result<Self, Box<dyn Error>> {
        let mut attributes = Window::default_attributes()
            .with_title("Prism")
            .with_decorations(false)
            .with_transparent(true)
            .with_resizable(false)
            .with_window_level(WindowLevel::AlwaysOnTop)
            .with_inner_size(LogicalSize::new(SIDE, SIDE));
        if let Some(monitor) = event_loop.primary_monitor() {
            let side = (SIDE * monitor.scale_factor()) as i32;
            let size = monitor.size();
            let origin = monitor.position();
            attributes = attributes.with_position(PhysicalPosition::new(
                origin.x + (size.width as i32 - side) / 2,
                origin.y + (size.height as i32 - side) / 2,
            ));
        }
        let window = Arc::new(event_loop.create_window(attributes)?);
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..wgpu::InstanceDescriptor::new_with_display_handle(Box::new(
                event_loop.owned_display_handle(),
            ))
        });
        let surface = instance.create_surface(Arc::clone(&window))?;
        let gpu = Arc::new(pollster::block_on(Gpu::from_instance(
            &instance,
            Some(&surface),
        ))?);
        window.request_redraw();
        Ok(Self {
            window,
            gpu,
            opened: Instant::now(),
            painted: None,
            surface: Some(surface),
            animation: None,
        })
    }

    pub fn id(&self) -> WindowId {
        self.window.id()
    }

    /// The GPU, for the rest of the application.
    pub fn gpu(&self) -> Arc<Gpu> {
        Arc::clone(&self.gpu)
    }

    /// Paints the first frame, then starts the animation thread; later
    /// calls do nothing.
    pub fn paint(&mut self) -> Result<(), Box<dyn Error>> {
        if self.painted.is_some() {
            return Ok(());
        }
        let Some(surface) = &self.surface else {
            return Ok(());
        };
        let size = self.window.inner_size();
        let side = size.width.min(size.height);
        if side == 0 {
            return Ok(());
        }
        let capabilities = surface.get_capabilities(&self.gpu.adapter);
        if !capabilities.usages.contains(wgpu::TextureUsages::COPY_DST) {
            return Err("la fenêtre de démarrage n'accepte pas de copie".into());
        }
        let format = [
            wgpu::TextureFormat::Bgra8Unorm,
            wgpu::TextureFormat::Rgba8Unorm,
        ]
        .into_iter()
        .find(|format| capabilities.formats.contains(format))
        .ok_or("format de fenêtre non pris en charge")?;
        let alpha_mode = [
            wgpu::CompositeAlphaMode::PreMultiplied,
            wgpu::CompositeAlphaMode::PostMultiplied,
        ]
        .into_iter()
        .find(|mode| capabilities.alpha_modes.contains(mode))
        .ok_or("transparence non prise en charge")?;
        let mut config = surface
            .get_default_config(&self.gpu.adapter, side, side)
            .ok_or("le GPU ne peut pas afficher dans la fenêtre de démarrage")?;
        config.usage = wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_DST;
        config.format = format;
        config.alpha_mode = alpha_mode;
        config.view_formats.clear();
        surface.configure(&self.gpu.device, &config);

        let target = Target {
            side,
            bgra: format == wgpu::TextureFormat::Bgra8Unorm,
            premultiplied: alpha_mode == wgpu::CompositeAlphaMode::PreMultiplied,
        };
        let art = Art::new()?;
        let first = art.render(0.0, 0.0, side).ok_or("image de démarrage")?;
        if !present(&self.gpu, surface, &first, target) {
            // Not shown yet: next redraw.
            self.window.request_redraw();
            return Ok(());
        }
        let painted = Instant::now();
        self.painted = Some(painted);

        let stop = Arc::new(AtomicBool::new(false));
        let (gpu, flag) = (Arc::clone(&self.gpu), Arc::clone(&stop));
        let surface = self.surface.take().ok_or("surface")?;
        let thread = std::thread::Builder::new()
            .name("prism-splash".into())
            .spawn(move || animate(&art, &gpu, &surface, target, painted, &flag))?;
        self.animation = Some(Animation { stop, thread });
        Ok(())
    }

    /// Time at which the start may block the event loop.
    pub fn ready_at(&self) -> Instant {
        match self.painted {
            Some(painted) => painted + SETTLE,
            None => self.opened + PAINT_TIMEOUT,
        }
    }
}

impl Drop for Splash {
    /// Leaves the image its shortest time on screen, then fades it out.
    fn drop(&mut self) {
        let Some(animation) = self.animation.take() else {
            return;
        };
        if let Some(painted) = self.painted {
            std::thread::sleep((painted + SHORTEST).saturating_duration_since(Instant::now()));
        }
        animation.stop.store(true, Ordering::Relaxed);
        let _ = animation.thread.join();
    }
}

/// Draws frames until `stop`, then fades the image out.
fn animate(
    art: &Art,
    gpu: &Gpu,
    surface: &wgpu::Surface<'static>,
    target: Target,
    start: Instant,
    stop: &AtomicBool,
) {
    let mut stopped: Option<Instant> = None;
    loop {
        let seconds = start.elapsed().as_secs_f32();
        let fade_in = (seconds / FADE).min(1.0);
        if stopped.is_none() && stop.load(Ordering::Relaxed) {
            stopped = Some(Instant::now());
        }
        let fade_out = stopped.map_or(1.0, |at| 1.0 - at.elapsed().as_secs_f32() / FADE);
        let opacity = fade_in.min(fade_out).max(0.0);
        let shown = art
            .render(seconds, opacity, target.side)
            .is_some_and(|frame| present(gpu, surface, &frame, target));
        if opacity <= 0.0 && stopped.is_some() {
            return;
        }
        if !shown {
            std::thread::sleep(Duration::from_millis(16));
        }
    }
}

/// Copies `frame` into the window; false when the window takes no frame now.
fn present(gpu: &Gpu, surface: &wgpu::Surface<'static>, frame: &Pixmap, target: Target) -> bool {
    let texture = match surface.get_current_texture() {
        wgpu::CurrentSurfaceTexture::Success(texture)
        | wgpu::CurrentSurfaceTexture::Suboptimal(texture) => texture,
        _ => return false,
    };
    let bytes = surface_bytes(frame.data(), target);
    gpu.queue.write_texture(
        texture.texture.as_image_copy(),
        &bytes,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(target.side * 4),
            rows_per_image: Some(target.side),
        },
        texture.texture.size(),
    );
    gpu.queue.submit([]);
    gpu.queue.present(texture);
    true
}

/// Premultiplied RGBA pixels in the order and alpha mode of the window.
fn surface_bytes(rgba: &[u8], target: Target) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(rgba.len());
    for &[mut r, mut g, mut b, a] in rgba.as_chunks::<4>().0 {
        if !target.premultiplied && a > 0 {
            let unmultiply =
                |channel: u8| ((channel as u32 * 255 + a as u32 / 2) / a as u32).min(255) as u8;
            [r, g, b] = [unmultiply(r), unmultiply(g), unmultiply(b)];
        }
        if target.bgra {
            bytes.extend_from_slice(&[b, g, r, a]);
        } else {
            bytes.extend_from_slice(&[r, g, b, a]);
        }
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pixels_follow_the_window_format() {
        let pixel = [100, 50, 0, 128];
        let target = Target {
            side: 1,
            bgra: true,
            premultiplied: true,
        };
        assert_eq!(surface_bytes(&pixel, target), vec![0, 50, 100, 128]);
        let target = Target {
            bgra: false,
            premultiplied: false,
            ..target
        };
        assert_eq!(surface_bytes(&pixel, target), vec![199, 100, 0, 128]);
    }
}
