//! Window showing the application logo while the GPU and the engine start.
//!
//! The window is drawn on the CPU, so it appears before any GPU work.

use std::error::Error;
use std::io::Cursor;
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::{Duration, Instant};

use winit::dpi::{LogicalSize, PhysicalPosition};
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowId, WindowLevel};

/// The logo, 1024 × 1024 RGBA.
const LOGO: &[u8] = include_bytes!("../../../assets/icon-1024.png");

/// Side of the window, in logical pixels.
const SIDE: f64 = 320.0;

/// Background behind the logo, 0RGB.
const BACKGROUND: [u8; 3] = [0x12, 0x13, 0x16];

/// Time left to the window server to show the painted window before the
/// start blocks the event loop.
const SETTLE: Duration = Duration::from_millis(50);

/// Longest wait for the first paint, after which the start goes on anyway.
const PAINT_TIMEOUT: Duration = Duration::from_secs(1);

pub struct Splash {
    window: Arc<Window>,
    surface: softbuffer::Surface<Arc<Window>, Arc<Window>>,
    opened: Instant,
    painted: Option<Instant>,
}

impl Splash {
    pub fn open(event_loop: &ActiveEventLoop) -> Result<Self, Box<dyn Error>> {
        let mut attributes = Window::default_attributes()
            .with_title("Prism")
            .with_decorations(false)
            .with_resizable(false)
            .with_window_level(WindowLevel::AlwaysOnTop)
            .with_inner_size(LogicalSize::new(SIDE, SIDE));
        if let Some(monitor) = event_loop.primary_monitor() {
            let scale = monitor.scale_factor();
            let side = (SIDE * scale) as i32;
            let size = monitor.size();
            let origin = monitor.position();
            attributes = attributes.with_position(PhysicalPosition::new(
                origin.x + (size.width as i32 - side) / 2,
                origin.y + (size.height as i32 - side) / 2,
            ));
        }
        let window = Arc::new(event_loop.create_window(attributes)?);
        let context = softbuffer::Context::new(Arc::clone(&window))?;
        let surface = softbuffer::Surface::new(&context, Arc::clone(&window))?;
        window.request_redraw();
        Ok(Self {
            window,
            surface,
            opened: Instant::now(),
            painted: None,
        })
    }

    pub fn id(&self) -> WindowId {
        self.window.id()
    }

    /// Draws the logo, scaled to the window, over the background.
    pub fn paint(&mut self) -> Result<(), Box<dyn Error>> {
        let size = self.window.inner_size();
        let (Some(width), Some(height)) =
            (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
        else {
            return Ok(());
        };
        self.surface.resize(width, height)?;
        let logo = decode_logo()?;
        let mut buffer = self.surface.buffer_mut()?;
        let side = size.width.min(size.height);
        let pixels = scale(&logo, side);
        let left = (size.width - side) / 2;
        let top = (size.height - side) / 2;
        let background = pack(BACKGROUND);
        buffer.fill(background);
        for y in 0..side {
            let row = ((top + y) * size.width + left) as usize;
            for x in 0..side {
                let [r, g, b, a] = pixels[(y * side + x) as usize];
                let over = |channel: u8, under: u8| {
                    ((channel as u32 * a as u32 + under as u32 * (255 - a as u32)) / 255) as u8
                };
                buffer[row + x as usize] = pack([
                    over(r, BACKGROUND[0]),
                    over(g, BACKGROUND[1]),
                    over(b, BACKGROUND[2]),
                ]);
            }
        }
        buffer.present()?;
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

fn pack([r, g, b]: [u8; 3]) -> u32 {
    (r as u32) << 16 | (g as u32) << 8 | b as u32
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
