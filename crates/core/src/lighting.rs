//! Lighting: LED strips and fixtures placed over the composition, whose
//! colours are taken from the image and sent as DMX over the network.

use serde::{Deserialize, Serialize};

use crate::Point;

/// Most pixels of one fixture.
pub const MAX_FIXTURE_PIXELS: u16 = 4096;

/// How DMX travels on the network.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DmxProtocol {
    /// Art-Net (port 6454).
    #[default]
    ArtNet,
    /// sACN, ANSI E1.31 (port 5568).
    Sacn,
}

/// Order and number of the channels of one pixel.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColorOrder {
    /// Red, green, blue.
    #[default]
    Rgb,
    /// Green, red, blue, as many LED strips take them.
    Grb,
    /// Red, green, blue, then white (the smallest of the three).
    Rgbw,
    /// One channel: the brightness.
    Dimmer,
}

impl ColorOrder {
    /// Channels per pixel.
    pub fn channels(self) -> usize {
        match self {
            Self::Rgb | Self::Grb => 3,
            Self::Rgbw => 4,
            Self::Dimmer => 1,
        }
    }

    /// The channel values of a pixel of colour `rgb`.
    pub fn values(self, [red, green, blue]: [u8; 3]) -> Vec<u8> {
        match self {
            Self::Rgb => vec![red, green, blue],
            Self::Grb => vec![green, red, blue],
            Self::Rgbw => {
                let white = red.min(green).min(blue);
                vec![red - white, green - white, blue - white, white]
            }
            Self::Dimmer => vec![red.max(green).max(blue)],
        }
    }
}

/// Identifier of a fixture, unique in its composition.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct FixtureId(pub u32);

/// A line of pixels over the composition: a LED strip, a row of lights,
/// or one light when it has one pixel.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Fixture {
    id: FixtureId,
    /// Name shown in the interface.
    pub name: String,
    /// Whether its colours are sent.
    #[serde(default = "enabled")]
    pub enabled: bool,
    /// Where its first pixel takes its colour, as fractions of the
    /// composition.
    pub from: Point,
    /// Where its last pixel takes its colour.
    pub to: Point,
    /// Number of pixels, spread evenly from `from` to `to`.
    pub pixels: u16,
    /// Channels of each pixel.
    #[serde(default)]
    pub order: ColorOrder,
    /// Universe of its first pixel.
    pub universe: u16,
    /// Channel of its first pixel, from 1 to 512. Pixels that do not fit
    /// in the universe continue on the next one, from channel 1.
    pub channel: u16,
    /// Brightness, from 0 to 1.
    #[serde(default = "full")]
    pub brightness: f32,
}

fn enabled() -> bool {
    true
}

fn full() -> f32 {
    1.0
}

impl Fixture {
    /// Identifier of the fixture.
    pub fn id(&self) -> FixtureId {
        self.id
    }

    /// Where pixel `index` takes its colour.
    pub fn pixel_point(&self, index: u16) -> Point {
        // One pixel: the middle of the line.
        let t = if self.pixels > 1 {
            f32::from(index) / f32::from(self.pixels - 1)
        } else {
            0.5
        };
        let (from, to) = (self.from, self.to);
        Point::new(from.x + (to.x - from.x) * t, from.y + (to.y - from.y) * t)
    }
}

/// The fixtures of a composition and where their DMX goes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Lighting {
    /// Whether DMX is sent.
    pub enabled: bool,
    /// Protocol of the packets.
    pub protocol: DmxProtocol,
    /// Address the packets go to (IPv4); empty, broadcast (Art-Net) or
    /// multicast (sACN).
    pub target: String,
    /// Universes sent per second at most.
    pub rate: f32,
    fixtures: Vec<Fixture>,
    next_id: u32,
}

impl Default for Lighting {
    fn default() -> Self {
        Self {
            enabled: false,
            protocol: DmxProtocol::ArtNet,
            target: String::new(),
            rate: 40.0,
            fixtures: Vec::new(),
            next_id: 0,
        }
    }
}

impl Lighting {
    /// Whether there is nothing to send nor keep.
    pub fn is_unused(&self) -> bool {
        !self.enabled && self.fixtures.is_empty()
    }

    /// The fixtures, in order.
    pub fn fixtures(&self) -> &[Fixture] {
        &self.fixtures
    }

    /// The fixture `id`, to change.
    pub fn fixture_mut(&mut self, id: FixtureId) -> Option<&mut Fixture> {
        self.fixtures.iter_mut().find(|fixture| fixture.id == id)
    }

    /// Adds a strip of `pixels` RGB pixels across the middle of the
    /// composition, after the channels used so far.
    pub fn add_strip(&mut self, name: impl Into<String>, pixels: u16) -> FixtureId {
        let next = self.next_id.max(
            self.fixtures
                .iter()
                .map(|fixture| fixture.id.0 + 1)
                .max()
                .unwrap_or(0),
        );
        self.next_id = next + 1;
        let (universe, channel) = self.next_free_channel();
        let id = FixtureId(next);
        self.fixtures.push(Fixture {
            id,
            name: name.into(),
            enabled: true,
            from: Point::new(0.05, 0.5),
            to: Point::new(0.95, 0.5),
            pixels: pixels.clamp(1, MAX_FIXTURE_PIXELS),
            order: ColorOrder::Rgb,
            universe,
            channel,
            brightness: 1.0,
        });
        id
    }

    /// Removes the fixture `id`.
    pub fn remove(&mut self, id: FixtureId) -> Option<Fixture> {
        let index = self.fixtures.iter().position(|fixture| fixture.id == id)?;
        Some(self.fixtures.remove(index))
    }

    /// The universe and channel after the last channel of the last fixture.
    fn next_free_channel(&self) -> (u16, u16) {
        let Some(last) = self.fixtures.last() else {
            return (0, 1);
        };
        let used = usize::from(last.channel - 1) + usize::from(last.pixels) * last.order.channels();
        let universe = last.universe as usize + used / 510;
        let channel = used % 510 + 1;
        (universe.min(usize::from(u16::MAX)) as u16, channel as u16)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_become_channels_in_order() {
        assert_eq!(ColorOrder::Rgb.values([10, 20, 30]), [10, 20, 30]);
        assert_eq!(ColorOrder::Grb.values([10, 20, 30]), [20, 10, 30]);
        assert_eq!(ColorOrder::Rgbw.values([10, 20, 30]), [0, 10, 20, 10]);
        assert_eq!(ColorOrder::Dimmer.values([10, 20, 30]), [30]);
    }

    #[test]
    fn pixels_spread_along_the_line() {
        let mut lighting = Lighting::default();
        let id = lighting.add_strip("Bande", 5);
        let strip = lighting.fixture_mut(id).unwrap();
        strip.from = Point::new(0.0, 0.0);
        strip.to = Point::new(1.0, 0.5);
        assert_eq!(strip.pixel_point(0), Point::new(0.0, 0.0));
        assert_eq!(strip.pixel_point(2), Point::new(0.5, 0.25));
        assert_eq!(strip.pixel_point(4), Point::new(1.0, 0.5));
        strip.pixels = 1;
        assert_eq!(strip.pixel_point(0), Point::new(0.5, 0.25));
    }

    #[test]
    fn new_strips_follow_the_channels_used() {
        let mut lighting = Lighting::default();
        lighting.add_strip("A", 100);
        let b = lighting.add_strip("B", 100);
        let b = lighting
            .fixtures()
            .iter()
            .find(|fixture| fixture.id() == b)
            .unwrap();
        assert_eq!((b.universe, b.channel), (0, 301));
        // 300 + 300 channels: past 510, the next universe.
        let c = lighting.add_strip("C", 10);
        let c = lighting
            .fixtures()
            .iter()
            .find(|fixture| fixture.id() == c)
            .unwrap();
        assert_eq!((c.universe, c.channel), (1, 91));
    }
}
