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
    /// Where it lands on the screen.
    #[serde(default)]
    pub output: Quad,
    /// Grid warping the slice; it then replaces [`Slice::output`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub warp: Option<Warp>,
}

fn enabled() -> bool {
    true
}

impl Slice {
    /// Identifier of the slice.
    pub fn id(&self) -> SliceId {
        self.id
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
}

#[derive(Deserialize)]
struct StageRepr {
    #[serde(default)]
    screens: Vec<Screen>,
    #[serde(default)]
    next_id: u32,
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
            output: Quad::FULL,
            warp: None,
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
