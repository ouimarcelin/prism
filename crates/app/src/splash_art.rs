//! The splash image in motion: a square grid that unfolds, in
//! [`DURATION`], into `assets/splash.svg`, rendered on the processor for
//! each frame.
//!
//! At the start, the vertices of the frame's facets sit on a regular grid
//! and its outline is a square; they ease into their places in the image.
//! The central band and the logo stay still.

use std::fmt::Write as _;

use resvg::{tiny_skia, usvg};

/// The image, drawn by the author of Prism.
const SVG: &str = include_str!("../../../assets/splash.svg");

/// Length of the animation, in seconds.
pub const DURATION: f32 = 3.0;

/// Start of the outer outline in the image: a path with rounded corners,
/// replaced by a polygon whose corners can move.
const OUTLINE_START: &str = "<path class=\"cls-3\" d=\"M131.76";

/// The square grid at the start: left and right edges of the inner frame,
/// as in the image, and top and bottom edges as far apart, so that the
/// square with its outline fills the window.
const LEFT: f32 = 9.79;
const RIGHT: f32 = 506.24;
const TOP: f32 = -19.5;
const BOTTOM: f32 = TOP + (RIGHT - LEFT);
/// Rows of the grid between the edges and the central band.
const UPPER: f32 = (TOP + 160.81) / 2.0;
const LOWER: f32 = (300.36 + BOTTOM) / 2.0;
/// Width of the band between the inner frame and the outline: left, right,
/// top and bottom.
const BAND: [f32; 4] = [9.29, 7.76, 8.5, 8.5];

/// Where a vertex of the image starts.
#[derive(Clone, Copy)]
enum Start {
    /// On the grid: column (0 to 5) and row.
    Grid(u8, f32),
    /// Hidden under the central band: in its column, at its own height.
    Column(u8),
}

/// The vertices that move, as written in the image, and where they start.
const VERTICES: [((f32, f32), Start); 36] = [
    // Top edge.
    ((49.07, 21.2), Start::Grid(0, TOP)),
    ((128.33, 9.0), Start::Grid(1, TOP)),
    ((216.01, 21.2), Start::Grid(2, TOP)),
    ((307.29, 9.0), Start::Grid(3, TOP)),
    ((382.95, 33.14), Start::Grid(4, TOP)),
    ((466.42, 21.2), Start::Grid(5, TOP)),
    // Upper row.
    ((9.79, 104.67), Start::Grid(0, UPPER)),
    ((146.56, 116.85), Start::Grid(1, UPPER)),
    ((216.01, 104.67), Start::Grid(2, UPPER)),
    ((299.48, 104.67), Start::Grid(3, UPPER)),
    ((371.82, 91.23), Start::Grid(4, UPPER)),
    ((506.24, 104.67), Start::Grid(5, UPPER)),
    // Under the band, top half.
    ((132.74, 187.14), Start::Column(1)),
    ((132.54, 188.14), Start::Column(1)),
    ((230.03, 200.32), Start::Column(2)),
    ((313.5, 200.32), Start::Column(3)),
    ((401.77, 188.14), Start::Column(4)),
    ((401.77, 187.14), Start::Column(4)),
    // Under the band, bottom half.
    ((119.48, 274.03), Start::Column(1)),
    ((113.83, 254.59), Start::Column(1)),
    ((197.3, 254.59), Start::Column(2)),
    ((299.48, 271.61), Start::Column(3)),
    ((382.95, 271.61), Start::Column(4)),
    ((382.95, 274.03), Start::Column(4)),
    // Lower row.
    ((9.79, 362.06), Start::Grid(0, LOWER)),
    ((142.53, 362.06), Start::Grid(1, LOWER)),
    ((211.98, 349.88), Start::Grid(2, LOWER)),
    ((295.45, 349.88), Start::Grid(3, LOWER)),
    ((396.97, 367.26), Start::Grid(4, LOWER)),
    ((506.24, 362.06), Start::Grid(5, LOWER)),
    // Bottom edge.
    ((49.07, 438.55), Start::Grid(0, BOTTOM)),
    ((132.54, 448.28), Start::Grid(1, BOTTOM)),
    ((218.99, 428.93), Start::Grid(2, BOTTOM)),
    ((299.48, 438.55), Start::Grid(3, BOTTOM)),
    ((382.95, 448.28), Start::Grid(4, BOTTOM)),
    ((466.42, 438.55), Start::Grid(5, BOTTOM)),
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

/// A piece of the image: text as it is, or a vertex to place.
enum Piece {
    Text(String),
    /// A vertex going from `start` to `end`, its place in the image.
    Vertex {
        start: (f32, f32),
        end: (f32, f32),
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
                    start: start_of(x, y),
                    end: (x, y),
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

    /// The image with every vertex at its place at `seconds`.
    fn svg(&self, seconds: f32, opacity: f32) -> String {
        let t = ease_in_out((seconds / DURATION).clamp(0.0, 1.0));
        let mut svg = String::with_capacity(8192);
        for piece in &self.pieces {
            match piece {
                Piece::Text(text) => svg.push_str(text),
                Piece::Vertex { start, end } => {
                    let x = start.0 + (end.0 - start.0) * t;
                    let y = start.1 + (end.1 - start.1) * t;
                    let _ = write!(svg, "{x:.2} {y:.2} ");
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

fn same(a: (f32, f32), x: f32, y: f32) -> bool {
    (a.0 - x).abs() < 0.01 && (a.1 - y).abs() < 0.01
}

/// Where the point `(x, y)` of the image starts: on the square grid for a
/// vertex, on the square outline for a corner of the outline, where it is
/// for the rest.
fn start_of(x: f32, y: f32) -> (f32, f32) {
    if let Some((_, inner)) = OUTLINE.iter().find(|(corner, _)| same(*corner, x, y)) {
        let (sx, sy) = start_of(inner.0, inner.1);
        let dx = if sx <= LEFT {
            -BAND[0]
        } else if sx >= RIGHT {
            BAND[1]
        } else {
            0.0
        };
        let dy = if sy <= TOP {
            -BAND[2]
        } else if sy >= BOTTOM {
            BAND[3]
        } else {
            0.0
        };
        return (sx + dx, sy + dy);
    }
    let column = |index: u8| LEFT + (RIGHT - LEFT) * f32::from(index) / 5.0;
    match VERTICES.iter().find(|(vertex, _)| same(*vertex, x, y)) {
        Some((_, Start::Grid(index, row))) => (column(*index), *row),
        Some((_, Start::Column(index))) => (column(*index), y),
        None => (x, y),
    }
}

/// Slow at both ends: cubic ease-in-out.
fn ease_in_out(t: f32) -> f32 {
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (2.0 - 2.0 * t).powi(3) / 2.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_vertex_is_in_the_image() {
        let art = Art::new().unwrap();
        for (vertex, _) in VERTICES {
            let found = art.pieces.iter().any(
                |piece| matches!(piece, Piece::Vertex { end, .. } if same(*end, vertex.0, vertex.1)),
            );
            assert!(found, "vertex {vertex:?} not in the image");
        }
    }

    #[test]
    fn the_outline_starts_square() {
        let (left, right) = (LEFT - BAND[0], RIGHT + BAND[1]);
        let (top, bottom) = (TOP - BAND[2], BOTTOM + BAND[3]);
        for ((x, y), _) in OUTLINE {
            let (x, y) = start_of(x, y);
            let on_edge = (x - left).abs() < 0.01
                || (x - right).abs() < 0.01
                || (y - top).abs() < 0.01
                || (y - bottom).abs() < 0.01;
            assert!(on_edge, "corner ({x}, {y}) off the square");
        }
        assert!(((right - left) - (bottom - top)).abs() < 1.0);
    }

    #[test]
    fn the_image_unfolds_into_the_drawing() {
        let art = Art::new().unwrap();
        let first = art.render(0.0, 1.0, 128).unwrap();
        let middle = art.render(DURATION / 2.0, 1.0, 128).unwrap();
        let last = art.render(DURATION, 1.0, 128).unwrap();
        let after = art.render(DURATION + 1.0, 1.0, 128).unwrap();
        assert_ne!(first.data(), middle.data());
        assert_ne!(middle.data(), last.data());
        assert_eq!(last.data(), after.data());
        assert!((ease_in_out(0.5) - 0.5).abs() < 1e-6);
    }
}
