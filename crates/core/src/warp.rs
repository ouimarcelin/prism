//! Warping of a slice by a grid of points: the slice's image is stretched
//! so each point of a regular grid over it lands where the user put it on
//! the screen. Between points, the image follows straight lines, smooth
//! curves that pass through every point, or Bézier curves whose tangent
//! handles are set by hand.

use serde::{Deserialize, Serialize};

use crate::{Point, Quad};

/// Most columns or rows of a warp grid.
pub const MAX_WARP_CELLS: u8 = 32;

/// How the image runs between the points of a warp grid.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WarpCurve {
    /// Straight between neighbouring points.
    #[default]
    Linear,
    /// Smooth curves through every point (Catmull-Rom splines).
    Smooth,
    /// Bézier curves through every point, bent by a pair of handles on
    /// each point (bicubic Bézier patches).
    Bezier,
}

/// Tangent handles of a point of a Bézier warp: offsets on the screen
/// from the point to its handles towards the next column and the next row.
/// The handles towards the previous column and row are their opposites.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Handles {
    /// Towards the next column (a third of the tangent across).
    pub across: Point,
    /// Towards the next row (a third of the tangent down).
    pub down: Point,
}

/// A grid of points over a slice, on its screen.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Warp {
    columns: u8,
    rows: u8,
    /// How the image runs between the points.
    #[serde(default)]
    pub curve: WarpCurve,
    /// `(columns + 1) × (rows + 1)` points on the screen, row by row from
    /// the top left.
    points: Vec<Point>,
    /// One pair of handles per point, for [`WarpCurve::Bezier`]; empty
    /// until the grid is first bent that way.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    handles: Vec<Handles>,
}

/// The projective mapping of the unit square onto `quad`: the slice's
/// image in perspective, as its four corners place it.
#[derive(Clone, Copy, Debug)]
struct Projection {
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    e: f64,
    f: f64,
    g: f64,
    h: f64,
}

impl Projection {
    fn new(quad: &Quad) -> Self {
        let [p0, p1, p2, p3] = quad.0.map(|point| (f64::from(point.x), f64::from(point.y)));
        let sx = p0.0 - p1.0 + p2.0 - p3.0;
        let sy = p0.1 - p1.1 + p2.1 - p3.1;
        let (g, h) = if sx.abs() < 1e-12 && sy.abs() < 1e-12 {
            (0.0, 0.0)
        } else {
            let (dx1, dx2) = (p1.0 - p2.0, p3.0 - p2.0);
            let (dy1, dy2) = (p1.1 - p2.1, p3.1 - p2.1);
            let denominator = dx1 * dy2 - dx2 * dy1;
            if denominator.abs() < 1e-12 {
                (0.0, 0.0)
            } else {
                (
                    (sx * dy2 - dx2 * sy) / denominator,
                    (dx1 * sy - sx * dy1) / denominator,
                )
            }
        };
        Self {
            a: p1.0 - p0.0 + g * p1.0,
            b: p3.0 - p0.0 + h * p3.0,
            c: p0.0,
            d: p1.1 - p0.1 + g * p1.1,
            e: p3.1 - p0.1 + h * p3.1,
            f: p0.1,
            g,
            h,
        }
    }

    fn map(&self, u: f64, v: f64) -> Point {
        let w = self.g * u + self.h * v + 1.0;
        let w = if w.abs() < 1e-12 { 1e-12 } else { w };
        Point::new(
            ((self.a * u + self.b * v + self.c) / w) as f32,
            ((self.d * u + self.e * v + self.f) / w) as f32,
        )
    }
}

/// Catmull-Rom interpolation between `p1` and `p2` at `t`.
fn catmull_rom(p0: Point, p1: Point, p2: Point, p3: Point, t: f32) -> Point {
    let curve = |a: f32, b: f32, c: f32, d: f32| {
        0.5 * (2.0 * b
            + (c - a) * t
            + (2.0 * a - 5.0 * b + 4.0 * c - d) * t * t
            + (3.0 * b - a - 3.0 * c + d) * t * t * t)
    };
    Point::new(curve(p0.x, p1.x, p2.x, p3.x), curve(p0.y, p1.y, p2.y, p3.y))
}

/// Cubic Bézier curve through `p0` and `p3`, bent by `p1` and `p2`, at `t`.
fn bezier(p: [Point; 4], t: f32) -> Point {
    let u = 1.0 - t;
    let w = [u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t];
    Point::new(
        p.iter().zip(w).map(|(p, w)| p.x * w).sum(),
        p.iter().zip(w).map(|(p, w)| p.y * w).sum(),
    )
}

fn add(a: Point, b: Point) -> Point {
    Point::new(a.x + b.x, a.y + b.y)
}

fn sub(a: Point, b: Point) -> Point {
    Point::new(a.x - b.x, a.y - b.y)
}

fn scale(a: Point, factor: f32) -> Point {
    Point::new(a.x * factor, a.y * factor)
}

fn lerp(a: Point, b: Point, t: f32) -> Point {
    Point::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)
}

/// Point `index` of `line`, extended in a straight line past both ends.
fn extended(line: &[Point], index: isize) -> Point {
    let last = line.len() as isize - 1;
    if index < 0 {
        lerp(line[0], line[1], index as f32)
    } else if index > last {
        let (a, b) = (line[last as usize - 1], line[last as usize]);
        lerp(b, a, -(index - last) as f32)
    } else {
        line[index as usize]
    }
}

/// Cell holding `position` (0 to 1) in `count` cells, and the position in it.
fn cell(position: f32, count: u8) -> (usize, f32) {
    let scaled = position.clamp(0.0, 1.0) * f32::from(count);
    let index = (scaled.floor() as usize).min(usize::from(count) - 1);
    (index, scaled - index as f32)
}

impl Warp {
    /// A grid of `columns` × `rows` cells following `quad`, in perspective:
    /// the slice looks as it did without the warp.
    pub fn from_quad(quad: &Quad, columns: u8, rows: u8) -> Self {
        let (columns, rows) = (
            columns.clamp(1, MAX_WARP_CELLS),
            rows.clamp(1, MAX_WARP_CELLS),
        );
        let projection = Projection::new(quad);
        let points = (0..=rows)
            .flat_map(|row| {
                (0..=columns).map(move |column| {
                    projection.map(
                        f64::from(column) / f64::from(columns),
                        f64::from(row) / f64::from(rows),
                    )
                })
            })
            .collect();
        Self {
            columns,
            rows,
            curve: WarpCurve::Linear,
            points,
            handles: Vec::new(),
        }
    }

    /// Number of cells across and down.
    pub fn cells(&self) -> (u8, u8) {
        (self.columns, self.rows)
    }

    /// The points, row by row from the top left.
    pub fn points(&self) -> &[Point] {
        &self.points
    }

    /// The points, to move them.
    pub fn points_mut(&mut self) -> &mut [Point] {
        &mut self.points
    }

    /// Tangent handles of the points, row by row from the top left; empty
    /// until [`Warp::ensure_handles`].
    pub fn handles(&self) -> &[Handles] {
        &self.handles
    }

    /// The handles, to move them.
    pub fn handles_mut(&mut self) -> &mut [Handles] {
        &mut self.handles
    }

    /// Gives every point handles, if they are missing, taken on the current
    /// shape: switched to Bézier, the image does not move.
    pub fn ensure_handles(&mut self) {
        if self.handles.len() == self.points.len() {
            return;
        }
        let shape = Self {
            handles: Vec::new(),
            ..self.clone()
        };
        self.handles = shape.handles_from_shape(self.columns, self.rows);
    }

    /// Handles that follow the shape of the grid as it is drawn, for the
    /// points of a grid of `columns` × `rows` cells: a third of its tangent
    /// across one of those cells, at each point.
    fn handles_from_shape(&self, columns: u8, rows: u8) -> Vec<Handles> {
        let (count_across, count_down) = (columns, rows);
        let (columns, rows) = (f32::from(columns), f32::from(rows));
        let step = 1e-3;
        let tangent = |u: f32, v: f32, across: bool| {
            let at = |d: f32| {
                if across {
                    self.evaluate((u + d).clamp(0.0, 1.0), v)
                } else {
                    self.evaluate(u, (v + d).clamp(0.0, 1.0))
                }
            };
            let (before, after) = (at(-step), at(step));
            let span = if across {
                ((u + step).min(1.0) - (u - step).max(0.0)) * columns
            } else {
                ((v + step).min(1.0) - (v - step).max(0.0)) * rows
            };
            scale(sub(after, before), 1.0 / (3.0 * span))
        };
        (0..=count_down)
            .flat_map(|row| (0..=count_across).map(move |column| (column, row)))
            .map(|(column, row)| {
                let (u, v) = (f32::from(column) / columns, f32::from(row) / rows);
                Handles {
                    across: tangent(u, v, true),
                    down: tangent(u, v, false),
                }
            })
            .collect()
    }

    /// Whether the grid has as many points as cells call for.
    pub fn is_valid(&self) -> bool {
        (1..=MAX_WARP_CELLS).contains(&self.columns)
            && (1..=MAX_WARP_CELLS).contains(&self.rows)
            && self.points.len() == (usize::from(self.columns) + 1) * (usize::from(self.rows) + 1)
            && (self.handles.is_empty() || self.handles.len() == self.points.len())
    }

    fn row(&self, row: usize) -> &[Point] {
        let width = usize::from(self.columns) + 1;
        &self.points[row * width..(row + 1) * width]
    }

    /// Where the point at `u`, `v` of the slice's image (fractions across
    /// and down) lands on the screen.
    pub fn evaluate(&self, u: f32, v: f32) -> Point {
        if !self.is_valid() {
            return Point::new(u, v);
        }
        let (column, s) = cell(u, self.columns);
        let (row, t) = cell(v, self.rows);
        let curve = match self.curve {
            // Without handles for every point, the smooth curves stand in.
            WarpCurve::Bezier if self.handles.len() != self.points.len() => WarpCurve::Smooth,
            curve => curve,
        };
        match curve {
            WarpCurve::Bezier => {
                let width = usize::from(self.columns) + 1;
                let index = |c: usize, r: usize| r * width + c;
                // The 4 × 4 control points of the cell, from its corners
                // and their handles; the inner ones without twist.
                let mut control = [[Point::default(); 4]; 4];
                for (dc, dr) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let at = index(column + dc, row + dr);
                    let (point, handles) = (self.points[at], self.handles[at]);
                    let across = if dc == 0 {
                        handles.across
                    } else {
                        scale(handles.across, -1.0)
                    };
                    let down = if dr == 0 {
                        handles.down
                    } else {
                        scale(handles.down, -1.0)
                    };
                    let (i, j) = (3 * dc, 3 * dr);
                    let (inner_i, inner_j) =
                        (if dc == 0 { 1 } else { 2 }, if dr == 0 { 1 } else { 2 });
                    control[j][i] = point;
                    control[j][inner_i] = add(point, across);
                    control[inner_j][i] = add(point, down);
                    control[inner_j][inner_i] = add(add(point, across), down);
                }
                let rows = control.map(|line| bezier(line, s));
                bezier(rows, t)
            }
            WarpCurve::Linear => {
                let (top, bottom) = (self.row(row), self.row(row + 1));
                lerp(
                    lerp(top[column], top[column + 1], s),
                    lerp(bottom[column], bottom[column + 1], s),
                    t,
                )
            }
            WarpCurve::Smooth => {
                // Along the four rows around the cell (rows past the edges
                // extended in a straight line), then down through them.
                let along = |index: usize| {
                    let line = self.row(index);
                    let at = |offset: isize| extended(line, column as isize + offset);
                    catmull_rom(at(-1), at(0), at(1), at(2), s)
                };
                let last = usize::from(self.rows);
                let (p1, p2) = (along(row), along(row + 1));
                let p0 = if row == 0 {
                    lerp(p1, p2, -1.0)
                } else {
                    along(row - 1)
                };
                let p3 = if row + 1 == last {
                    lerp(p2, p1, -1.0)
                } else {
                    along(row + 2)
                };
                catmull_rom(p0, p1, p2, p3, t)
            }
        }
    }

    /// The same warp with `columns` × `rows` cells, its points taken on the
    /// current shape.
    pub fn resized(&self, columns: u8, rows: u8) -> Self {
        let (columns, rows) = (
            columns.clamp(1, MAX_WARP_CELLS),
            rows.clamp(1, MAX_WARP_CELLS),
        );
        let points = (0..=rows)
            .flat_map(|row| {
                (0..=columns).map(move |column| {
                    (
                        f32::from(column) / f32::from(columns),
                        f32::from(row) / f32::from(rows),
                    )
                })
            })
            .map(|(u, v)| self.evaluate(u, v))
            .collect();
        let mut resized = Self {
            columns,
            rows,
            curve: self.curve,
            points,
            handles: Vec::new(),
        };
        if !self.handles.is_empty() {
            // Handles taken on the shape before, at the new points.
            resized.handles = self.handles_from_shape(columns, rows);
        }
        resized
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: Point, b: Point) -> bool {
        (a.x - b.x).abs() < 1e-4 && (a.y - b.y).abs() < 1e-4
    }

    #[test]
    fn a_grid_on_a_rectangle_is_regular() {
        let warp = Warp::from_quad(&Quad::FULL, 2, 2);
        assert!(warp.is_valid());
        assert_eq!(warp.points().len(), 9);
        assert!(near(warp.points()[4], Point::new(0.5, 0.5)));
        for curve in [WarpCurve::Linear, WarpCurve::Smooth] {
            let warp = Warp {
                curve,
                ..warp.clone()
            };
            assert!(
                near(warp.evaluate(0.3, 0.8), Point::new(0.3, 0.8)),
                "{curve:?}"
            );
        }
    }

    #[test]
    fn a_grid_on_a_trapezoid_follows_its_perspective() {
        // Same trapezoid as the renderer's test: the middle of the image is
        // a third of the way down.
        let trapezoid = Quad([
            Point::new(0.25, 0.0),
            Point::new(0.75, 0.0),
            Point::new(1.0, 1.0),
            Point::new(0.0, 1.0),
        ]);
        let warp = Warp::from_quad(&trapezoid, 2, 2);
        assert!(near(warp.points()[0], Point::new(0.25, 0.0)));
        assert!(near(warp.points()[8], Point::new(1.0, 1.0)));
        assert!(near(warp.points()[4], Point::new(0.5, 1.0 / 3.0)));
    }

    #[test]
    fn smooth_curves_pass_through_the_points() {
        let mut warp = Warp::from_quad(&Quad::FULL, 2, 2);
        warp.points_mut()[4] = Point::new(0.6, 0.4);
        warp.curve = WarpCurve::Smooth;
        assert!(near(warp.evaluate(0.5, 0.5), Point::new(0.6, 0.4)));
        assert!(near(warp.evaluate(0.0, 0.0), Point::new(0.0, 0.0)));
        // Beside the moved point, the image bends towards it.
        let beside = warp.evaluate(0.25, 0.5);
        assert!(beside.x > 0.25 && beside.y < 0.5, "{beside:?}");
        warp.curve = WarpCurve::Linear;
        assert!(near(warp.evaluate(0.25, 0.5), Point::new(0.3, 0.45)));
    }

    #[test]
    fn resizing_keeps_the_shape() {
        let mut warp = Warp::from_quad(&Quad::FULL, 1, 1);
        warp.points_mut()[3] = Point::new(0.8, 0.9);
        let finer = warp.resized(4, 2);
        assert_eq!(finer.cells(), (4, 2));
        assert!(near(finer.evaluate(1.0, 1.0), Point::new(0.8, 0.9)));
        assert!(near(finer.evaluate(0.5, 0.5), warp.evaluate(0.5, 0.5)));
    }

    #[test]
    fn bezier_handles_keep_the_shape_then_bend_it() {
        let mut warp = Warp::from_quad(&Quad::FULL, 2, 2);
        warp.points_mut()[4] = Point::new(0.6, 0.4);
        warp.curve = WarpCurve::Smooth;
        let smooth = warp.evaluate(0.25, 0.5);
        warp.curve = WarpCurve::Bezier;
        warp.ensure_handles();
        assert_eq!(warp.handles().len(), 9);
        // Switched to Bézier, the curves through the points stay.
        assert!(near(warp.evaluate(0.5, 0.5), Point::new(0.6, 0.4)));
        let bezier = warp.evaluate(0.25, 0.5);
        assert!((bezier.x - smooth.x).abs() < 0.02 && (bezier.y - smooth.y).abs() < 0.02);
        // A longer handle across pulls the curve beside the point.
        warp.handles_mut()[4].across = Point::new(0.0, 0.3);
        let bent = warp.evaluate(0.75, 0.5);
        assert!(bent.y > 0.5, "{bent:?}");
        assert!(near(warp.evaluate(0.5, 0.5), Point::new(0.6, 0.4)));
        // Resized, the handles follow.
        let finer = warp.resized(4, 4);
        assert_eq!(finer.handles().len(), 25);
        let (a, b) = (finer.evaluate(0.6, 0.55), warp.evaluate(0.6, 0.55));
        assert!(
            (a.x - b.x).abs() < 0.02 && (a.y - b.y).abs() < 0.02,
            "{a:?} {b:?}"
        );
    }

    #[test]
    fn bezier_without_handles_draws_smooth_curves() {
        let mut warp = Warp::from_quad(&Quad::FULL, 2, 2);
        warp.points_mut()[4] = Point::new(0.6, 0.4);
        warp.curve = WarpCurve::Smooth;
        let smooth = warp.evaluate(0.3, 0.6);
        warp.curve = WarpCurve::Bezier;
        assert!(near(warp.evaluate(0.3, 0.6), smooth));
    }
}
