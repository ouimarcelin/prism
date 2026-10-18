//! Transparent window showing the application logo while the engine and
//! the control window start.
//!
//! The GPU opened for this window is handed over to the application.

use std::error::Error;
use std::io::Cursor;
use std::sync::Arc;
use std::time::{Duration, Instant};

use prism_render::{Gpu, wgpu};
use winit::dpi::{LogicalSize, PhysicalPosition};
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowId, WindowLevel};

/// The splash image, 2048 × 2048 RGBA, rendered from `assets/splash.svg`:
/// sharp on a Retina screen.
const LOGO: &[u8] = include_bytes!("../../../assets/splash-2048.png");

/// Side of the window, in logical pixels: large enough for the thin lines
/// of the image.
const SIDE: f64 = 640.0;

/// Time left to the window server to show the painted window before the
/// start blocks the event loop.
const SETTLE: Duration = Duration::from_millis(50);

/// Longest wait for the first paint, after which the start goes on anyway.
const PAINT_TIMEOUT: Duration = Duration::from_secs(1);

pub struct Splash {
    window: Arc<Window>,
    gpu: Arc<Gpu>,
    surface: wgpu::Surface<'static>,
    opened: Instant,
    painted: Option<Instant>,
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
            surface,
            opened: Instant::now(),
            painted: None,
        })
    }

    pub fn id(&self) -> WindowId {
        self.window.id()
    }

    /// The GPU, for the rest of the application.
    pub fn gpu(&self) -> Arc<Gpu> {
        Arc::clone(&self.gpu)
    }

    /// Copies the logo, scaled to the window, into the window surface.
    pub fn paint(&mut self) -> Result<(), Box<dyn Error>> {
        let size = self.window.inner_size();
        let side = size.width.min(size.height);
        if side == 0 {
            return Ok(());
        }
        let capabilities = self.surface.get_capabilities(&self.gpu.adapter);
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
        let mut config = self
            .surface
            .get_default_config(&self.gpu.adapter, side, side)
            .ok_or("le GPU ne peut pas afficher dans la fenêtre de démarrage")?;
        config.usage = wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_DST;
        config.format = format;
        config.alpha_mode = alpha_mode;
        config.view_formats.clear();
        self.surface.configure(&self.gpu.device, &config);
        let logo = decode_logo()?;
        let premultiplied = alpha_mode == wgpu::CompositeAlphaMode::PreMultiplied;
        let mut bytes = Vec::with_capacity((side * side * 4) as usize);
        for [r, g, b, a] in scale(&logo, side) {
            let [r, g, b] = if premultiplied {
                [r, g, b].map(|channel| ((channel as u32 * a as u32 + 127) / 255) as u8)
            } else {
                [r, g, b]
            };
            match format {
                wgpu::TextureFormat::Bgra8Unorm => bytes.extend_from_slice(&[b, g, r, a]),
                _ => bytes.extend_from_slice(&[r, g, b, a]),
            }
        }
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            // Not shown yet: next redraw.
            _ => {
                self.window.request_redraw();
                return Ok(());
            }
        };
        self.gpu.queue.write_texture(
            frame.texture.as_image_copy(),
            &bytes,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(side * 4),
                rows_per_image: Some(side),
            },
            frame.texture.size(),
        );
        self.gpu.queue.submit([]);
        self.gpu.queue.present(frame);
        self.painted.get_or_insert_with(Instant::now);
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

/// The logo as RGBA pixels, row by row.
struct Logo {
    side: u32,
    pixels: Vec<u8>,
}

fn decode_logo() -> Result<Logo, Box<dyn Error>> {
    let mut decoder = png::Decoder::new(Cursor::new(LOGO));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info()?;
    let size = reader.output_buffer_size().ok_or("logo trop grand")?;
    let mut pixels = vec![0; size];
    let info = reader.next_frame(&mut pixels)?;
    if info.color_type != png::ColorType::Rgba || info.width != info.height {
        return Err("logo : image RGBA carrée attendue".into());
    }
    Ok(Logo {
        side: info.width,
        pixels,
    })
}

/// The logo averaged down (or sampled up) to `side` × `side` pixels, with
/// colors weighted by their alpha.
fn scale(logo: &Logo, side: u32) -> Vec<[u8; 4]> {
    let ratio = logo.side as f64 / side as f64;
    let span = |index: u32| {
        let start = (index as f64 * ratio) as u32;
        let end = (((index + 1) as f64 * ratio) as u32).clamp(start + 1, logo.side);
        start..end
    };
    let mut result = Vec::with_capacity((side * side) as usize);
    for y in 0..side {
        let rows = span(y);
        for x in 0..side {
            let mut sum = [0u64; 4];
            let mut count = 0u64;
            for sy in rows.clone() {
                for sx in span(x) {
                    let i = ((sy * logo.side + sx) * 4) as usize;
                    let alpha = logo.pixels[i + 3] as u64;
                    for (total, value) in sum.iter_mut().zip(&logo.pixels[i..i + 3]) {
                        *total += *value as u64 * alpha;
                    }
                    sum[3] += alpha;
                    count += 1;
                }
            }
            let color = |channel: usize| sum[channel].checked_div(sum[3]).unwrap_or(0) as u8;
            result.push([color(0), color(1), color(2), (sum[3] / count) as u8]);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_logo_decodes_as_a_square() {
        let logo = decode_logo().unwrap();
        assert_eq!(logo.pixels.len(), (logo.side * logo.side * 4) as usize);
    }

    #[test]
    fn scaling_averages_by_alpha() {
        // Opaque red next to transparent green: the color stays red.
        let logo = Logo {
            side: 2,
            pixels: vec![255, 0, 0, 255, 0, 255, 0, 0, 255, 0, 0, 255, 0, 255, 0, 0],
        };
        assert_eq!(scale(&logo, 1), vec![[255, 0, 0, 127]]);
        assert_eq!(scale(&logo, 4)[1], [255, 0, 0, 255]);
    }
}
