//! The stage: the screens the composition is projected on, and the slices
//! of the composition each screen shows.
//!
//! A screen is an image of its own size, usually sent to one projector or
//! display. A slice takes a rectangle of the composition and places it on
//! a screen through four corners, so it can be moved, scaled, rotated or
//! put in perspective. Positions are fractions of the composition or of
//! the screen, from the top left corner: they survive a change of size.

use serde::{Deserialize, Serialize};

use crate::{ModelError, Warp};

/// Largest width or height of a screen, in pixels.
pub const MAX_SCREEN_SIZE: u32 = 16_384;

/// A point, as fractions of an image's width and height from its top left
/// corner. Values outside 0 to 1 lie outside the image.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Point {
    /// Fraction of the width, from the left.
    pub x: f32,
    /// Fraction of the height, from the top.
    pub y: f32,
}

impl Point {
    /// A point at `x`, `y`.
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// A rectangle of the composition, as fractions of its size.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Area {
    /// Left edge.
    pub x: f32,
    /// Top edge.
    pub y: f32,
    /// Width.
    pub width: f32,
    /// Height.
    pub height: f32,
}

impl Area {
    /// The whole image.
    pub const FULL: Self = Self {
        x: 0.0,
        y: 0.0,
        width: 1.0,
        height: 1.0,
    };

    /// Corners in drawing order: top left, top right, bottom right, bottom
    /// left.
    pub fn corners(&self) -> [Point; 4] {
        let (left, top) = (self.x, self.y);
        let (right, bottom) = (self.x + self.width, self.y + self.height);
        [
            Point::new(left, top),
            Point::new(right, top),
            Point::new(right, bottom),
            Point::new(left, bottom),
        ]
    }
}

impl Default for Area {
    fn default() -> Self {
        Self::FULL
    }
}

/// Where a slice lands on its screen: its four corners, top left, top
/// right, bottom right, bottom left of the slice's image.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Quad(pub [Point; 4]);

impl Quad {
    /// The whole screen.
    pub const FULL: Self = Self([
        Point::new(0.0, 0.0),
        Point::new(1.0, 0.0),
        Point::new(1.0, 1.0),
        Point::new(0.0, 1.0),
    ]);

    /// The quad covering `area`, without perspective.
    pub fn from_area(area: Area) -> Self {
        Self(area.corners())
    }
}

impl Default for Quad {
    fn default() -> Self {
        Self::FULL
    }
}

/// Identifier of a screen, unique in its stage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ScreenId(pub u32);

/// Identifier of a slice, unique in its stage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SliceId(pub u32);

/// A rectangle of the composition placed on a screen.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Slice {
    id: SliceId,
    /// Name shown in the interface.
    pub name: String,
    /// Whether the slice is drawn.
    #[serde(default = "enabled")]
    pub enabled: bool,
    /// Part of the composition shown.
    #[serde(default)]
    pub input: Area,
    /// Turn of the part shown about its center, in degrees, clockwise.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub rotation: f32,
    /// Mirroring of the part shown.
    #[serde(default, skip_serializing_if = "Flip::is_none")]
    pub flip: Flip,
    /// Where it lands on the screen.
    #[serde(default)]
    pub output: Quad,
    /// Grid warping the slice; it then replaces [`Slice::output`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub warp: Option<Warp>,
    /// Shape limiting where the slice shows on its screen.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mask: Option<Mask>,
    /// Darkening of the slice's edges where it overlaps the image of
    /// another projector.
    #[serde(default, skip_serializing_if = "EdgeBlend::is_off")]
    pub blend: EdgeBlend,
}

/// Soft edges of a slice, so two projectors overlapping on a strip add up
/// to an even image: each side fades out over its width, along a curve
/// that sums to one with the other projector's, corrected for the display
/// gamma.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EdgeBlend {
    /// Width of the fade on the left side, as a fraction of the slice.
    pub left: f32,
    /// Width of the fade on the right side.
    pub right: f32,
    /// Height of the fade on the top side.
    pub top: f32,
    /// Height of the fade on the bottom side.
    pub bottom: f32,
    /// Steepness of the fade: 1 is a straight ramp, higher values keep more
    /// light near the middle of the overlap.
    pub power: f32,
    /// Gamma of the projectors, to fade in light rather than in pixel
    /// values.
    pub gamma: f32,
    /// Pixel value added outside the fades, so the black of the image
    /// matches the overlap, where two projectors' black adds up.
    pub black: f32,
}

impl Default for EdgeBlend {
    fn default() -> Self {
        Self {
            left: 0.0,
            right: 0.0,
            top: 0.0,
            bottom: 0.0,
            power: 2.0,
            gamma: 2.2,
            black: 0.0,
        }
    }
}

impl EdgeBlend {
    /// Whether no side fades.
    pub fn is_off(&self) -> bool {
        self.left <= 0.0 && self.right <= 0.0 && self.top <= 0.0 && self.bottom <= 0.0
    }

    /// Light kept at `t`, from 0 at the outer edge of a fade to 1 at its
    /// inner edge, as a pixel value: two overlapping projectors, one at
    /// `t` and the other at `1 - t`, add up to full light.
    pub fn factor(&self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        let power = self.power.max(0.1);
        let light = if t < 0.5 {
            0.5 * (2.0 * t).powf(power)
        } else {
            1.0 - 0.5 * (2.0 * (1.0 - t)).powf(power)
        };
        light.powf(1.0 / self.gamma.max(0.1))
    }
}

/// Most points of a mask.
pub const MAX_MASK_POINTS: usize = 64;

/// A polygon on the screen: the slice shows inside it only, or outside it
/// only when inverted, with a soft edge.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Mask {
    /// Corners of the polygon, in fractions of the screen, in order.
    pub points: Vec<Point>,
    /// Whether the slice shows outside the polygon instead of inside.
    #[serde(default)]
    pub inverted: bool,
    /// Width of the soft edge, in pixels of the screen, centered on the
    /// outline.
    #[serde(default)]
    pub feather: f32,
}

impl Mask {
    /// A mask following `quad`, its corners pulled a little inwards.
    pub fn inside(quad: &Quad) -> Self {
        let center = quad.0.iter().fold(Point::default(), |sum, point| {
            Point::new(sum.x + point.x / 4.0, sum.y + point.y / 4.0)
        });
        Self {
            points: quad
                .0
                .iter()
                .map(|point| {
                    Point::new(
                        center.x + (point.x - center.x) * 0.9,
                        center.y + (point.y - center.y) * 0.9,
                    )
                })
                .collect(),
            inverted: false,
            feather: 0.0,
        }
    }

    /// Whether the polygon has between 3 and [`MAX_MASK_POINTS`] points.
    pub fn is_valid(&self) -> bool {
        (3..=MAX_MASK_POINTS).contains(&self.points.len())
    }

    /// Adds a point in the middle of the longest edge; returns its index.
    pub fn split_longest_edge(&mut self) -> Option<usize> {
        if self.points.len() >= MAX_MASK_POINTS || self.points.len() < 2 {
            return None;
        }
        let count = self.points.len();
        let length = |index: usize| {
            let (a, b) = (self.points[index], self.points[(index + 1) % count]);
            (b.x - a.x).hypot(b.y - a.y)
        };
        let longest = (0..count).max_by(|a, b| length(*a).total_cmp(&length(*b)))?;
        let (a, b) = (self.points[longest], self.points[(longest + 1) % count]);
        self.points.insert(
            longest + 1,
            Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0),
        );
        Some(longest + 1)
    }
}

fn enabled() -> bool {
    true
}

fn is_zero(value: &f32) -> bool {
    *value == 0.0
}

/// Mirroring of an image.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Flip {
    /// Left and right swapped.
    pub horizontal: bool,
    /// Top and bottom swapped.
    pub vertical: bool,
}

impl Flip {
    /// Whether nothing is mirrored.
    pub fn is_none(&self) -> bool {
        !self.horizontal && !self.vertical
    }

    /// `point` (fractions of an image) mirrored.
    pub fn apply(&self, point: Point) -> Point {
        Point::new(
            if self.horizontal {
                1.0 - point.x
            } else {
                point.x
            },
            if self.vertical {
                1.0 - point.y
            } else {
                point.y
            },
        )
    }
}

impl Slice {
    /// Identifier of the slice.
    pub fn id(&self) -> SliceId {
        self.id
    }

    /// The point of the composition shown at `u`, `v` of the slice (fractions
    /// across and down), after its mirroring and rotation; `aspect` is the
    /// composition's width over its height.
    pub fn source(&self, u: f32, v: f32, aspect: f32) -> Point {
        let local = self.flip.apply(Point::new(u, v));
        let area = self.input;
        if self.rotation == 0.0 {
            return Point::new(
                area.x + local.x * area.width,
                area.y + local.y * area.height,
            );
        }
        // Turned in pixels (heights as the unit), so angles stay true.
        let (dx, dy) = (
            (local.x - 0.5) * area.width * aspect,
            (local.y - 0.5) * area.height,
        );
        let (sin, cos) = self.rotation.to_radians().sin_cos();
        // Clockwise on the screen: y grows downwards.
        let (rx, ry) = (dx * cos + dy * sin, -dx * sin + dy * cos);
        Point::new(
            area.x + area.width / 2.0 + rx / aspect.max(1e-6),
            area.y + area.height / 2.0 + ry,
        )
    }
}

/// An image sent to a projector or display, made of slices of the
/// composition over black.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Screen {
    id: ScreenId,
    /// Name shown in the interface.
    pub name: String,
    width: u32,
    height: u32,
    /// Name of the display (as the system names it) that shows this screen
    /// when its output is open.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display: Option<String>,
    /// Slices, drawn first to last: a later slice covers an earlier one.
    #[serde(default)]
    pub slices: Vec<Slice>,
    /// Mirroring of the whole screen, for a projector hung upside down or
    /// projecting from behind.
    #[serde(default, skip_serializing_if = "Flip::is_none")]
    pub flip: Flip,
    /// Whether the slices show a test pattern instead of the composition,
    /// to line projectors up.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub test_pattern: bool,
    /// Whether the screen's image is published over Syphon (macOS), under
    /// « Prism – » and its name.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub syphon: bool,
}

impl Screen {
    /// Identifier of the screen.
    pub fn id(&self) -> ScreenId {
        self.id
    }

    /// Size in pixels.
    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Changes the size in pixels; slices keep their place in fractions.
    pub fn set_size(&mut self, width: u32, height: u32) -> Result<(), ModelError> {
        check_size(width, height)?;
        (self.width, self.height) = (width, height);
        Ok(())
    }

    /// The slice `id`.
    pub fn slice(&self, id: SliceId) -> Option<&Slice> {
        self.slices.iter().find(|slice| slice.id == id)
    }

    /// The slice `id`, to change.
    pub fn slice_mut(&mut self, id: SliceId) -> Option<&mut Slice> {
        self.slices.iter_mut().find(|slice| slice.id == id)
    }
}

fn check_size(width: u32, height: u32) -> Result<(), ModelError> {
    if width == 0 || height == 0 || width > MAX_SCREEN_SIZE || height > MAX_SCREEN_SIZE {
        return Err(ModelError::Inconsistent(format!(
            "screen size {width} × {height} out of range"
        )));
    }
    Ok(())
}

/// The screens of a composition. Empty, each output shows the whole
/// composition.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Stage {
    screens: Vec<Screen>,
    next_id: u32,
    /// Rehearsal of the projection on the computer's own display.
    #[serde(skip_serializing_if = "Simulation::is_off")]
    pub simulation: Simulation,
}

/// The screens shown side by side, in order, each overlapping the previous
/// one, their light added as overlapping projectors would: to check edge
/// blending without projectors.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Simulation {
    /// Whether the engine draws the simulation.
    pub enabled: bool,
    /// Width shared by two neighbouring screens, in pixels.
    pub overlap: u32,
    /// Gamma of the simulated projectors.
    pub gamma: f32,
}

impl Default for Simulation {
    fn default() -> Self {
        Self {
            enabled: false,
            overlap: 192,
            gamma: 2.2,
        }
    }
}

impl Simulation {
    /// Whether the simulation is off.
    pub fn is_off(&self) -> bool {
        !self.enabled
    }
}

#[derive(Deserialize)]
struct StageRepr {
    #[serde(default)]
    screens: Vec<Screen>,
    #[serde(default)]
    next_id: u32,
    #[serde(default)]
    simulation: Simulation,
}

impl<'de> Deserialize<'de> for Stage {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let repr = StageRepr::deserialize(deserializer)?;
        let mut ids = std::collections::HashSet::new();
        let mut highest = None;
        for screen in &repr.screens {
            check_size(screen.width, screen.height).map_err(serde::de::Error::custom)?;
            if screen
                .slices
                .iter()
                .any(|slice| slice.warp.as_ref().is_some_and(|warp| !warp.is_valid()))
            {
                return Err(serde::de::Error::custom("warp grid of the wrong size"));
            }
            if screen
                .slices
                .iter()
                .any(|slice| slice.mask.as_ref().is_some_and(|mask| !mask.is_valid()))
            {
                return Err(serde::de::Error::custom(
                    "mask with too few or too many points",
                ));
            }
            let slice_ids = screen.slices.iter().map(|slice| slice.id.0);
            for id in std::iter::once(screen.id.0).chain(slice_ids) {
                if !ids.insert(id) {
                    return Err(serde::de::Error::custom(format!(
                        "duplicate stage identifier {id}"
                    )));
                }
                highest = highest.max(Some(id));
            }
        }
        let next_free = highest.map_or(0, |id: u32| id.saturating_add(1));
        Ok(Self {
            screens: repr.screens,
            next_id: repr.next_id.max(next_free),
            simulation: repr.simulation,
        })
    }
}

impl Stage {
    /// Whether there is no screen.
    pub fn is_empty(&self) -> bool {
        self.screens.is_empty()
    }

    /// The screens, in order.
    pub fn screens(&self) -> &[Screen] {
        &self.screens
    }

    /// The screen `id`.
    pub fn screen(&self, id: ScreenId) -> Option<&Screen> {
        self.screens.iter().find(|screen| screen.id == id)
    }

    /// The screen `id`, to change.
    pub fn screen_mut(&mut self, id: ScreenId) -> Option<&mut Screen> {
        self.screens.iter_mut().find(|screen| screen.id == id)
    }

    fn next(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Adds a screen of `width` × `height` pixels with one slice showing the
    /// whole composition on the whole screen.
    pub fn add_screen(
        &mut self,
        name: impl Into<String>,
        width: u32,
        height: u32,
    ) -> Result<ScreenId, ModelError> {
        check_size(width, height)?;
        let id = ScreenId(self.next());
        self.screens.push(Screen {
            id,
            name: name.into(),
            width,
            height,
            display: None,
            slices: Vec::new(),
            flip: Flip::default(),
            test_pattern: false,
            syphon: false,
        });
        self.add_slice(id, "Découpe 1");
        Ok(id)
    }

    /// Removes the screen `id` and its slices.
    pub fn remove_screen(&mut self, id: ScreenId) -> Option<Screen> {
        let index = self.screens.iter().position(|screen| screen.id == id)?;
        Some(self.screens.remove(index))
    }

    /// Adds a slice showing the whole composition on the whole screen `screen`,
    /// above its other slices.
    pub fn add_slice(&mut self, screen: ScreenId, name: impl Into<String>) -> Option<SliceId> {
        self.screen(screen)?;
        let id = SliceId(self.next());
        let slice = Slice {
            id,
            name: name.into(),
            enabled: true,
            input: Area::FULL,
            rotation: 0.0,
            flip: Flip::default(),
            output: Quad::FULL,
            warp: None,
            mask: None,
            blend: EdgeBlend::default(),
        };
        self.screen_mut(screen)?.slices.push(slice);
        Some(id)
    }

    /// Removes the slice `id` from the screen holding it.
    pub fn remove_slice(&mut self, id: SliceId) -> Option<Slice> {
        self.screens.iter_mut().find_map(|screen| {
            let index = screen.slices.iter().position(|slice| slice.id == id)?;
            Some(screen.slices.remove(index))
        })
    }

    /// Where each screen lies in the simulation, as left edge in pixels,
    /// and the simulation's size.
    pub fn simulation_layout(&self) -> (Vec<(ScreenId, u32)>, (u32, u32)) {
        let mut left = 0u32;
        let mut placed = Vec::with_capacity(self.screens.len());
        let mut size = (0, 0);
        for screen in &self.screens {
            let start = if placed.is_empty() {
                0
            } else {
                left.saturating_sub(self.simulation.overlap)
            };
            placed.push((screen.id, start));
            left = start + screen.width;
            size = (left, size.1.max(screen.height));
        }
        (placed, size)
    }

    /// Spreads the composition over the screens, left to right, each
    /// overlapping the previous one by the simulation's overlap: each
    /// screen keeps one slice, covering it, showing its share of the
    /// composition, with its edges fading where it overlaps a neighbour.
    /// The composition's shape is not kept: give screens the size of the
    /// projectors.
    pub fn spread(&mut self) {
        let (placed, (total, _)) = self.simulation_layout();
        if total == 0 {
            return;
        }
        let total = total as f32;
        let count = self.screens.len();
        for (index, ((_, left), screen)) in placed.iter().zip(&mut self.screens).enumerate() {
            let width = screen.width as f32;
            let overlap = (self.simulation.overlap as f32).min(width) / width;
            screen.slices.truncate(1);
            let Some(slice) = screen.slices.first_mut() else {
                continue;
            };
            slice.input = Area {
                x: *left as f32 / total,
                y: 0.0,
                width: width / total,
                height: 1.0,
            };
            slice.output = Quad::FULL;
            slice.warp = None;
            slice.rotation = 0.0;
            slice.flip = Flip::default();
            slice.blend.left = if index > 0 { overlap } else { 0.0 };
            slice.blend.right = if index + 1 < count { overlap } else { 0.0 };
            slice.blend.top = 0.0;
            slice.blend.bottom = 0.0;
        }
    }

    /// The screen shown on the display named `display`.
    pub fn screen_on(&self, display: &str) -> Option<&Screen> {
        self.screens
            .iter()
            .find(|screen| screen.display.as_deref() == Some(display))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_screen_shows_the_whole_composition() {
        let mut stage = Stage::default();
        assert!(stage.is_empty());
        let screen = stage.add_screen("Projecteur 1", 1920, 1080).unwrap();
        let screen = stage.screen(screen).unwrap();
        assert_eq!(screen.size(), (1920, 1080));
        assert_eq!(screen.slices.len(), 1);
        assert_eq!(screen.slices[0].input, Area::FULL);
        assert_eq!(screen.slices[0].output, Quad::FULL);
        assert!(stage.add_screen("Vide", 0, 1080).is_err());
    }

    #[test]
    fn identifiers_are_unique_and_survive_saving() {
        let mut stage = Stage::default();
        let first = stage.add_screen("A", 1280, 720).unwrap();
        let slice = stage.add_slice(first, "Gauche").unwrap();
        stage.screen_mut(first).unwrap().display = Some("Projecteur".into());
        stage
            .screen_mut(first)
            .unwrap()
            .slice_mut(slice)
            .unwrap()
            .input = Area {
            x: 0.0,
            y: 0.0,
            width: 0.5,
            height: 1.0,
        };
        let json = serde_json::to_string(&stage).unwrap();
        let mut loaded: Stage = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded, stage);
        // Identifiers continue after the highest one, even if `next_id` was lost.
        let mut value: serde_json::Value = serde_json::from_str(&json).unwrap();
        value["next_id"] = 0.into();
        let mut forgetful: Stage = serde_json::from_value(value).unwrap();
        let next = forgetful.add_screen("B", 640, 480).unwrap();
        assert_eq!(next, loaded.add_screen("B", 640, 480).unwrap());
        assert!(next.0 > slice.0);
        assert_eq!(
            forgetful.screen_on("Projecteur").map(Screen::id),
            Some(first)
        );
    }

    #[test]
    fn duplicate_identifiers_are_rejected() {
        let mut stage = Stage::default();
        let screen = stage.add_screen("A", 1280, 720).unwrap();
        stage.add_slice(screen, "B").unwrap();
        let mut value = serde_json::to_value(&stage).unwrap();
        value["screens"][0]["slices"][1]["id"] = value["screens"][0]["slices"][0]["id"].clone();
        assert!(serde_json::from_value::<Stage>(value).is_err());
    }

    #[test]
    fn masks_start_inside_their_slice_and_grow_on_their_longest_edge() {
        let mut mask = Mask::inside(&Quad::FULL);
        assert!(mask.is_valid());
        assert!((mask.points[0].x - 0.05).abs() < 1e-6);
        assert!((mask.points[2].y - 0.95).abs() < 1e-6);
        // A wide polygon: its top edge is the longest.
        mask.points = vec![
            Point::new(0.0, 0.0),
            Point::new(1.0, 0.0),
            Point::new(1.0, 0.1),
            Point::new(0.2, 0.1),
        ];
        assert_eq!(mask.split_longest_edge(), Some(1));
        assert_eq!(mask.points[1], Point::new(0.5, 0.0));
        mask.points.truncate(2);
        assert!(!mask.is_valid());
    }

    #[test]
    fn overlapping_fades_add_up_to_full_light() {
        let blend = EdgeBlend::default();
        assert!(blend.is_off());
        assert_eq!(blend.factor(0.0), 0.0);
        assert!((blend.factor(1.0) - 1.0).abs() < 1e-6);
        for t in [0.1, 0.3, 0.5, 0.8] {
            // Back to light, through the gamma, then summed.
            let light = |value: f32| value.powf(blend.gamma);
            let sum = light(blend.factor(t)) + light(blend.factor(1.0 - t));
            assert!((sum - 1.0).abs() < 1e-5, "{t}: {sum}");
        }
        // In pixel values, the middle of the overlap is brighter than half.
        assert!(blend.factor(0.5) > 0.7);
    }

    #[test]
    fn simulated_screens_overlap_side_by_side() {
        let mut stage = Stage::default();
        let a = stage.add_screen("A", 1920, 1080).unwrap();
        let b = stage.add_screen("B", 1280, 720).unwrap();
        stage.simulation.overlap = 200;
        let (placed, size) = stage.simulation_layout();
        assert_eq!(placed, [(a, 0), (b, 1720)]);
        assert_eq!(size, (3000, 1080));
    }

    #[test]
    fn sources_are_mirrored_and_turned_about_the_center() {
        let mut stage = Stage::default();
        let screen = stage.add_screen("A", 100, 100).unwrap();
        let slice = &mut stage.screen_mut(screen).unwrap().slices[0];
        slice.input = Area {
            x: 0.25,
            y: 0.0,
            width: 0.5,
            height: 1.0,
        };
        let near = |a: Point, b: Point| (a.x - b.x).abs() < 1e-5 && (a.y - b.y).abs() < 1e-5;
        assert!(near(slice.source(0.0, 0.0, 2.0), Point::new(0.25, 0.0)));
        slice.flip.horizontal = true;
        assert!(near(slice.source(0.0, 0.0, 2.0), Point::new(0.75, 0.0)));
        slice.flip = Flip::default();
        // A half turn: the top left of the slice shows the bottom right.
        slice.rotation = 180.0;
        assert!(near(slice.source(0.0, 0.0, 2.0), Point::new(0.75, 1.0)));
        // A quarter turn clockwise in a square area: the top left of the
        // slice shows the bottom left of the area.
        slice.input = Area {
            x: 0.25,
            y: 0.0,
            width: 0.5,
            height: 1.0,
        };
        slice.rotation = 90.0;
        // 0.5 × 2.0 wide = 1.0 high: a square in pixels.
        assert!(near(slice.source(0.0, 0.0, 2.0), Point::new(0.25, 1.0)));
        assert!(near(slice.source(1.0, 0.0, 2.0), Point::new(0.25, 0.0)));
    }

    #[test]
    fn spreading_shares_the_composition_with_fades_on_the_overlaps() {
        let mut stage = Stage::default();
        let a = stage.add_screen("A", 1000, 500).unwrap();
        let b = stage.add_screen("B", 1000, 500).unwrap();
        stage.add_slice(b, "En trop").unwrap();
        stage.simulation.overlap = 200;
        stage.spread();
        let (left, right) = (
            &stage.screen(a).unwrap().slices[0],
            &stage.screen(b).unwrap().slices,
        );
        assert_eq!(right.len(), 1);
        let right = &right[0];
        // 1800 pixels in all: A shows 0 to 1000, B 800 to 1800.
        assert!((left.input.width - 1000.0 / 1800.0).abs() < 1e-6);
        assert!((right.input.x - 800.0 / 1800.0).abs() < 1e-6);
        assert!((left.blend.right - 0.2).abs() < 1e-6 && left.blend.left == 0.0);
        assert!((right.blend.left - 0.2).abs() < 1e-6 && right.blend.right == 0.0);
        // The overlap shows the same part of the composition on both.
        let shared_a = left.input.x + left.input.width * (1.0 - left.blend.right);
        assert!((shared_a - right.input.x).abs() < 1e-6);
    }

    #[test]
    fn slices_are_removed_from_their_screen() {
        let mut stage = Stage::default();
        let screen = stage.add_screen("A", 1280, 720).unwrap();
        let slice = stage.add_slice(screen, "B").unwrap();
        assert_eq!(
            stage.remove_slice(slice).map(|slice| slice.name),
            Some("B".into())
        );
        assert_eq!(stage.screen(screen).unwrap().slices.len(), 1);
        assert!(stage.remove_screen(screen).is_some());
        assert!(stage.is_empty());
    }
}
