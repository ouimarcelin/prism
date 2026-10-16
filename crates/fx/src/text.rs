//! Text drawn by the text generator: laid out and rasterized on the CPU
//! into a coverage mask, which the generator's shader colors and places.

use ab_glyph_rasterizer::{Point, Rasterizer, point};
use prism_core::Effect;
use prism_render::{pixels_texture, wgpu};
use skrifa::MetadataProvider;
use skrifa::instance::{LocationRef, Size};
use skrifa::outline::{DrawSettings, OutlinePen};

/// Kind of the text generator.
pub const TEXT_KIND: &str = "text";

/// Fonts the text generator offers, in the order of its `font` setting.
const FONTS: [&[u8]; 2] = [
    epaint_default_fonts::UBUNTU_LIGHT,
    epaint_default_fonts::HACK_REGULAR,
];

/// Smallest and largest line height a mask is rasterized at, in pixels.
const MIN_LINE: u32 = 16;
const MAX_LINE: u32 = 512;

/// Horizontal placement of the lines of a text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Align {
    Left,
    Center,
    Right,
}

/// What a mask depends on.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TextKey {
    pub text: String,
    /// Index in the fonts of the generator.
    pub font: usize,
    pub align: Align,
    /// Height of one line, in pixels.
    pub line: u32,
}

/// Coverage of a text: white, premultiplied, transparent around the glyphs.
#[derive(Clone, Debug, PartialEq)]
pub struct TextMask {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<[f32; 4]>,
}

/// Line height to rasterize at, for lines of `wanted` pixels on screen: the
/// power of two at or above it, so the mask is only shrunk, by at most half,
/// and a size that moves does not rasterize the text at every frame.
pub fn line_height_for(wanted: f32) -> u32 {
    let wanted = if wanted.is_finite() { wanted } else { 0.0 };
    (wanted.ceil().max(0.0) as u32)
        .clamp(MIN_LINE, MAX_LINE)
        .next_power_of_two()
}

/// Lays out and rasterizes `key.text`. `None` for a text with nothing to
/// draw.
pub fn rasterize(key: &TextKey) -> Option<TextMask> {
    let font = skrifa::FontRef::new(FONTS[key.font.min(FONTS.len() - 1)]).ok()?;
    let line = key.line.clamp(MIN_LINE, MAX_LINE);
    // A size whose ascent to descent spans the line height.
    let unscaled = font.metrics(Size::unscaled(), LocationRef::default());
    let em = unscaled.ascent - unscaled.descent;
    let size = Size::new(line as f32 * f32::from(unscaled.units_per_em) / em.max(1.0));
    let ascent = font.metrics(size, LocationRef::default()).ascent;
    let advances = font.glyph_metrics(size, LocationRef::default());
    let charmap = font.charmap();
    let outlines = font.outline_glyphs();

    let lines: Vec<Vec<(skrifa::GlyphId, f32)>> = key
        .text
        .lines()
        .map(|text| {
            text.chars()
                .map(|c| {
                    let glyph = charmap.map(c).unwrap_or_default();
                    (glyph, advances.advance_width(glyph).unwrap_or(0.0))
                })
                .collect()
        })
        .collect();
    let drawn = |c: char| !c.is_whitespace();
    if !key.text.chars().any(drawn) {
        return None;
    }
    let widths: Vec<f32> = lines
        .iter()
        .map(|glyphs| glyphs.iter().map(|(_, advance)| advance).sum())
        .collect();
    let box_width = widths.iter().copied().fold(0.0, f32::max).ceil();
    let box_height = (line * lines.len() as u32) as f32;

    // Pen position of each glyph in the layout box: lines one under the
    // other, placed by the alignment.
    let mut placed = Vec::new();
    for (index, (glyphs, text_width)) in lines.iter().zip(&widths).enumerate() {
        let mut x = match key.align {
            Align::Left => 0.0,
            Align::Center => (box_width - text_width) / 2.0,
            Align::Right => box_width - text_width,
        };
        let baseline = (index as u32 * line) as f32 + ascent;
        for (glyph, advance) in glyphs {
            placed.push((*glyph, x, baseline));
            x += advance;
        }
    }
    // Ink can reach out of the box (accents, descenders): the mask grows by
    // as much on both sides, so the box stays at its center.
    let (mut pad_x, mut pad_y) = (0.0f32, 0.0f32);
    for (glyph, x, baseline) in &placed {
        if let Some(bounds) = advances.bounds(*glyph) {
            pad_x = pad_x
                .max(-(x + bounds.x_min))
                .max(x + bounds.x_max - box_width);
            pad_y = pad_y
                .max(-(baseline - bounds.y_max))
                .max(baseline - bounds.y_min - box_height);
        }
    }
    let (pad_x, pad_y) = (pad_x.ceil() as u32 + 1, pad_y.ceil() as u32 + 1);
    let width = box_width as u32 + 2 * pad_x;
    let height = box_height as u32 + 2 * pad_y;
    let mut rasterizer = Rasterizer::new(width as usize, height as usize);
    for (glyph, x, baseline) in placed {
        if let Some(outline) = outlines.get(glyph) {
            let mut pen = Pen {
                rasterizer: &mut rasterizer,
                origin: (x + pad_x as f32, baseline + pad_y as f32),
                start: point(0.0, 0.0),
                last: point(0.0, 0.0),
            };
            let _ = outline.draw(
                DrawSettings::unhinted(size, LocationRef::default()),
                &mut pen,
            );
        }
    }
    let mut pixels = vec![[0.0; 4]; (width * height) as usize];
    rasterizer.for_each_pixel(|index, coverage| {
        // Rounding leaves faint coverage where there is none.
        let a = if coverage < 1.0 / 1024.0 {
            0.0
        } else {
            coverage.min(1.0)
        };
        pixels[index] = [a, a, a, a];
    });
    Some(TextMask {
        width,
        height,
        pixels,
    })
}

/// Draws a glyph outline, in font space (up is positive), into the mask
/// (down is positive) at `origin` on the baseline.
struct Pen<'a> {
    rasterizer: &'a mut Rasterizer,
    origin: (f32, f32),
    start: Point,
    last: Point,
}

impl Pen<'_> {
    fn at(&self, x: f32, y: f32) -> Point {
        point(self.origin.0 + x, self.origin.1 - y)
    }
}

impl OutlinePen for Pen<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        self.start = self.at(x, y);
        self.last = self.start;
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let to = self.at(x, y);
        self.rasterizer.draw_line(self.last, to);
        self.last = to;
    }

    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        let to = self.at(x, y);
        self.rasterizer.draw_quad(self.last, self.at(cx0, cy0), to);
        self.last = to;
    }

    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        let to = self.at(x, y);
        let (c0, c1) = (self.at(cx0, cy0), self.at(cx1, cy1));
        self.rasterizer.draw_cubic(self.last, c0, c1, to);
        self.last = to;
    }

    fn close(&mut self) {
        if self.last != self.start {
            self.rasterizer.draw_line(self.last, self.start);
        }
        self.last = self.start;
    }
}

/// The mask of the text generator of a clip, kept on the GPU while its
/// text, font, alignment and line height stay the same.
#[derive(Default)]
pub struct TextCache {
    current: Option<(TextKey, Option<wgpu::TextureView>)>,
}

impl TextCache {
    /// Mask of `generator`, for lines of `line_fraction` of an image
    /// `height` pixels high, with its line height in pixels: rasterized
    /// again only when what it depends on changed. `None` when there is
    /// nothing to draw.
    #[allow(clippy::too_many_arguments)]
    pub fn view(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        generator: &Effect,
        line_fraction: f32,
        height: u32,
        font: usize,
        align: Align,
    ) -> Option<(&wgpu::TextureView, u32)> {
        let line = line_height_for(line_fraction * height as f32);
        // Compared field by field: no new string at every frame.
        let unchanged = self.current.as_ref().is_some_and(|(old, _)| {
            old.text == generator.text && old.font == font && old.align == align && old.line == line
        });
        if !unchanged {
            let key = TextKey {
                text: generator.text.clone(),
                font,
                align,
                line,
            };
            let view = rasterize(&key).map(|mask| {
                pixels_texture(device, queue, mask.width, mask.height, &mask.pixels)
                    .create_view(&wgpu::TextureViewDescriptor::default())
            });
            self.current = Some((key, view));
        }
        self.current
            .as_ref()
            .and_then(|(key, view)| view.as_ref().map(|view| (view, key.line)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(text: &str, align: Align) -> TextKey {
        TextKey {
            text: text.into(),
            font: 0,
            align,
            line: 64,
        }
    }

    fn covered_columns(mask: &TextMask) -> (u32, u32) {
        let column_covered =
            |x: u32| (0..mask.height).any(|y| mask.pixels[(y * mask.width + x) as usize][3] > 0.5);
        let first = (0..mask.width).find(|x| column_covered(*x)).unwrap();
        let last = (0..mask.width).rev().find(|x| column_covered(*x)).unwrap();
        (first, last)
    }

    #[test]
    fn text_is_rasterized_line_by_line() {
        let one = rasterize(&key("Prism", Align::Left)).unwrap();
        let two = rasterize(&key("Prism\nPrism", Align::Left)).unwrap();
        assert_eq!(two.width, one.width);
        assert_eq!(two.height - one.height, 64);
        // The layout box is centered: as much room above as below.
        assert!(one.pixels.iter().any(|pixel| pixel[3] > 0.99));
        assert!(
            one.pixels
                .iter()
                .all(|pixel| (0.0..=1.0).contains(&pixel[3]))
        );
        // Premultiplied white.
        assert!(one.pixels.iter().all(|pixel| pixel[0] == pixel[3]));
        // A longer text is wider.
        let long = rasterize(&key("Prism Prism", Align::Left)).unwrap();
        assert!(long.width > one.width * 3 / 2);
        // Accents, signs and characters missing from the font: drawn, and
        // the top and bottom rows stay clear.
        for font in 0..FONTS.len() {
            let mask = rasterize(&TextKey {
                font,
                ..key("ÉÀÇ€ jÅ 😀 Œuvre", Align::Left)
            })
            .unwrap();
            let row_clear = |y: u32| {
                (0..mask.width).all(|x| mask.pixels[(y * mask.width + x) as usize][3] < 0.004)
            };
            assert!(row_clear(0) && row_clear(mask.height - 1), "font {font}");
        }
        // Nothing to draw.
        assert_eq!(rasterize(&key("", Align::Left)), None);
        assert_eq!(rasterize(&key(" \n ", Align::Left)), None);
    }

    #[test]
    fn short_lines_follow_the_alignment() {
        let text = "Prism Prism\nP";
        let left = rasterize(&key(text, Align::Left)).unwrap();
        let right = rasterize(&key(text, Align::Right)).unwrap();
        let center = rasterize(&key(text, Align::Center)).unwrap();
        // Only the second line, below the first.
        let second = |mask: &TextMask| {
            let top = (mask.height - 2 * 64) / 2 + 64;
            TextMask {
                width: mask.width,
                height: mask.height - top,
                pixels: mask.pixels[(mask.width * top) as usize..].to_vec(),
            }
        };
        let (left_start, _) = covered_columns(&second(&left));
        let (center_start, _) = covered_columns(&second(&center));
        let (right_start, _) = covered_columns(&second(&right));
        assert!(left_start < center_start && center_start < right_start);
    }

    #[test]
    fn line_heights_go_by_powers_of_two() {
        assert_eq!(line_height_for(0.0), 16);
        assert_eq!(line_height_for(f32::NAN), 16);
        assert_eq!(line_height_for(100.0), 128);
        assert_eq!(line_height_for(128.0), 128);
        assert_eq!(line_height_for(129.0), 256);
        assert_eq!(line_height_for(5000.0), 512);
    }
}
