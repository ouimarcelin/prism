//! The splash image in motion: `assets/splash.svg`, with some of its
//! vertices drifting slowly, rendered on the processor for each frame.
//!
//! The vertices that move are the corners where the facets of the frame
//! meet, on its inner grid and on its outer edge; the central band, the
//! logo and the four outer corners stay still.

use std::fmt::Write as _;

use resvg::{tiny_skia, usvg};

/// The image, drawn by the author of Prism.
const SVG: &str = include_str!("../../../assets/splash.svg");

/// Start of the outer outline in the image: a path with rounded corners,
/// replaced by a polygon whose corners can move.
const OUTLINE_START: &str = "<path class=\"cls-3\" d=\"M131.76";

/// The inner vertices that move, as written in the image.
const MOVING: [(f32, f32); 20] = [
    (128.33, 9.0),
    (216.01, 21.2),
    (307.29, 9.0),
    (382.95, 33.14),
    (9.79, 104.67),
    (146.56, 116.85),
    (216.01, 104.67),
    (299.48, 104.67),
    (371.82, 91.23),
    (506.24, 104.67),
    (9.79, 362.06),
    (142.53, 362.06),
    (211.98, 349.88),
    (295.45, 349.88),
    (396.97, 367.26),
    (506.24, 362.06),
    (132.54, 448.28),
    (218.99, 428.93),
    (299.48, 438.55),
    (382.95, 448.28),
];

/// The outer outline: each corner, and the inner vertex it follows.
const OUTLINE: [((f32, f32), (f32, f32)); 16] = [
    ((0.5, 103.5), (9.79, 104.67)),
    ((43.0, 13.5), (49.07, 21.2)),
    ((127.5, 0.5), (128.33, 9.0)),
    ((215.24, 12.6), (216.01, 21.2)),
    ((306.5, 0.5), (307.29, 9.0)),
    ((382.76, 24.45), (382.95, 33.14)),
    ((469.5, 13.5), (466.42, 21.2)),
    ((514.0, 103.5), (506.24, 104.67)),
    ((514.0, 363.5), (506.24, 362.06)),
    ((472.0, 444.5), (466.42, 438.55)),
    ((382.5, 456.8), (382.95, 448.28)),
    ((298.6, 447.0), (299.48, 438.55)),
    ((219.5, 437.6), (218.99, 428.93)),
    ((131.8, 456.8), (132.54, 448.28)),
    ((43.5, 444.5), (49.07, 438.55)),
    ((0.5, 363.5), (9.79, 362.06)),
];

/// Largest drift of a vertex, in units of the image (514 wide).
const AMPLITUDE: f32 = 4.0;
/// Time over which the drift grows from nothing, in seconds.
const EASE_IN: f32 = 0.8;

/// A piece of the image: text as it is, or a vertex to place.
enum Piece {
    Text(String),
    /// The vertex `index` of [`MOVING`], at rest at `(x, y)`.
    Vertex {
        index: Option<usize>,
        x: f32,
        y: f32,
    },
}

/// The image cut into still text and vertices.
pub struct Art {
    pieces: Vec<Piece>,
    width: f32,
    height: f32,
}

impl Art {
    pub fn new() -> Result<Self, String> {
        let start = SVG
            .find(OUTLINE_START)
            .ok_or("image de démarrage : contour introuvable")?;
        let end = start
            + SVG[start..]
                .find("/>")
                .ok_or("image de démarrage : contour mal formé")?
            + 2;
        let mut outline = String::from("<polygon class=\"cls-3\" points=\"");
        for ((x, y), _) in OUTLINE {
            let _ = write!(outline, "{x} {y} ");
        }
        outline.push_str("\"/>");
        let svg = format!("{}{outline}{}", &SVG[..start], &SVG[end..]);

        let mut pieces = Vec::new();
        let mut rest = svg.as_str();
        while let Some(found) = rest.find("points=\"") {
            let open = found + "points=\"".len();
            pieces.push(Piece::Text(rest[..open].to_owned()));
            let close = open
                + rest[open..]
                    .find('"')
                    .ok_or("image de démarrage : points mal formés")?;
            let numbers: Vec<f32> = rest[open..close]
                .split_whitespace()
                .map(str::parse)
                .collect::<Result<_, _>>()
                .map_err(|_| "image de démarrage : nombre illisible")?;
            for &[x, y] in numbers.as_chunks::<2>().0 {
                pieces.push(Piece::Vertex {
                    index: follower(x, y),
                    x,
                    y,
                });
            }
            rest = &rest[close..];
        }
        pieces.push(Piece::Text(rest.to_owned()));

        let tree = usvg::Tree::from_str(&svg, &usvg::Options::default())
            .map_err(|error| format!("image de démarrage : {error}"))?;
        let size = tree.size();
        Ok(Self {
            pieces,
            width: size.width(),
            height: size.height(),
        })
    }

    /// The image `seconds` after the start, `opacity` from 0 to 1, as
    /// premultiplied RGBA pixels fitted in a square of `side` pixels.
    pub fn render(&self, seconds: f32, opacity: f32, side: u32) -> Option<tiny_skia::Pixmap> {
        let svg = self.svg(seconds, opacity);
        let tree = usvg::Tree::from_str(&svg, &usvg::Options::default()).ok()?;
        let mut pixmap = tiny_skia::Pixmap::new(side, side)?;
        let scale = side as f32 / self.width.max(self.height);
        let transform = tiny_skia::Transform::from_scale(scale, scale).post_translate(
            (side as f32 - self.width * scale) / 2.0,
            (side as f32 - self.height * scale) / 2.0,
        );
        resvg::render(&tree, transform, &mut pixmap.as_mut());
        Some(pixmap)
    }

    /// The image with every moving vertex at its place at `seconds`.
    fn svg(&self, seconds: f32, opacity: f32) -> String {
        let grow = smoothstep((seconds / EASE_IN).clamp(0.0, 1.0));
        let mut svg = String::with_capacity(8192);
        for piece in &self.pieces {
            match piece {
                Piece::Text(text) => svg.push_str(text),
                Piece::Vertex { index, x, y } => {
                    let (dx, dy) = index.map_or((0.0, 0.0), |index| drift(index, seconds));
                    let _ = write!(svg, "{:.2} {:.2} ", x + dx * grow, y + dy * grow);
                }
            }
        }
        if opacity < 1.0 {
            // The whole drawing, faded.
            if let (Some(open), Some(close)) = (svg.find("</defs>"), svg.rfind("</svg>")) {
                svg.insert_str(close, "</g>");
                svg.insert_str(
                    open + "</defs>".len(),
                    &format!("<g opacity=\"{opacity:.3}\">"),
                );
            }
        }
        svg
    }
}

/// Which moving vertex the point `(x, y)` follows: itself, or for an
/// outline corner, its inner vertex.
fn follower(x: f32, y: f32) -> Option<usize> {
    let same = |a: (f32, f32)| (a.0 - x).abs() < 0.01 && (a.1 - y).abs() < 0.01;
    let inner = OUTLINE
        .iter()
        .find(|(corner, _)| same(*corner))
        .map_or((x, y), |(_, inner)| *inner);
    MOVING
        .iter()
        .position(|&vertex| (vertex.0 - inner.0).abs() < 0.01 && (vertex.1 - inner.1).abs() < 0.01)
}

/// Offset of the moving vertex `index` at `seconds`: two slow waves of
/// its own speed and phase, so that the vertices never move together.
fn drift(index: usize, seconds: f32) -> (f32, f32) {
    let seed = index as f32 * 2.399_963; // golden angle
    let speed_x = 1.1 + 0.6 * (seed * 1.7).sin().abs();
    let speed_y = 0.9 + 0.6 * (seed * 2.3).cos().abs();
    (
        AMPLITUDE * (seconds * speed_x + seed).sin(),
        AMPLITUDE * (seconds * speed_y + seed * 1.3).cos(),
    )
}

fn smoothstep(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_moving_vertex_is_in_the_image() {
        let art = Art::new().unwrap();
        for (index, vertex) in MOVING.iter().enumerate() {
            let found = art
                .pieces
                .iter()
                .any(|piece| matches!(piece, Piece::Vertex { index: Some(i), .. } if *i == index));
            assert!(found, "vertex {vertex:?} not in the image");
        }
        // Every corner of the outline is found, moving or not.
        assert!(OUTLINE.iter().all(|(_, inner)| {
            follower(inner.0, inner.1).is_some()
                || [
                    (49.07, 21.2),
                    (466.42, 21.2),
                    (466.42, 438.55),
                    (49.07, 438.55),
                ]
                .contains(inner)
        }));
    }

    #[test]
    fn the_image_moves_and_stays_drawn() {
        let art = Art::new().unwrap();
        let still = art.render(0.0, 1.0, 128).unwrap();
        let later = art.render(2.0, 1.0, 128).unwrap();
        let drawn = still
            .pixels()
            .iter()
            .filter(|pixel| pixel.alpha() > 0)
            .count();
        assert!(drawn > 128 * 64, "the drawing covers most of the square");
        assert_ne!(still.data(), later.data());
        let faded = art.render(2.0, 0.5, 128).unwrap();
        let max_alpha = faded.pixels().iter().map(|p| p.alpha()).max().unwrap();
        assert!(max_alpha < 200);
    }
}
