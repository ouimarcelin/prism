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
    /// When set, the line is a cubic Bézier curve from `from` to `to`, bent
    /// towards these two control points (the first next to `from`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bend: Option<[Point; 2]>,
    /// Number of pixels, spread evenly along the line from `from` to `to`.
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
    /// Gamma applied to the colours sent: 1 sends the image's values as
    /// they are; about 2.2 suits LEDs whose controller does not correct
    /// them, which otherwise show mid tones too bright.
    #[serde(default = "full")]
    pub gamma: f32,
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

    /// The point of the line at `t`, from 0 (`from`) to 1 (`to`): on the
    /// straight segment, or on the Bézier curve when it is bent.
    pub fn line_point(&self, t: f32) -> Point {
        let (from, to) = (self.from, self.to);
        match self.bend {
            None => Point::new(from.x + (to.x - from.x) * t, from.y + (to.y - from.y) * t),
            Some([a, b]) => {
                let u = 1.0 - t;
                let w = [u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t];
                let p = [from, a, b, to];
                Point::new(
                    p.iter().zip(w).map(|(p, w)| p.x * w).sum(),
                    p.iter().zip(w).map(|(p, w)| p.y * w).sum(),
                )
            }
        }
    }

    /// The line as `segments` straight pieces, for drawing and picking.
    pub fn path(&self, segments: usize) -> Vec<Point> {
        let segments = if self.bend.is_some() {
            segments.max(1)
        } else {
            1
        };
        (0..=segments)
            .map(|index| self.line_point(index as f32 / segments as f32))
            .collect()
    }

    /// Where each pixel takes its colour: evenly spaced along the line,
    /// curved or not; one pixel sits in the middle.
    pub fn pixel_points(&self) -> Vec<Point> {
        let count = usize::from(self.pixels);
        if count <= 1 {
            return vec![self.line_point(0.5); count];
        }
        if self.bend.is_none() {
            return (0..count)
                .map(|index| self.line_point(index as f32 / (count - 1) as f32))
                .collect();
        }
        // Length along the curve, to space the pixels evenly on it.
        let path = self.path(256);
        let mut lengths = Vec::with_capacity(path.len());
        let mut total = 0.0;
        lengths.push(0.0);
        for pair in path.windows(2) {
            total += ((pair[1].x - pair[0].x).powi(2) + (pair[1].y - pair[0].y).powi(2)).sqrt();
            lengths.push(total);
        }
        let mut segment = 0;
        (0..count)
            .map(|index| {
                let wanted = total * index as f32 / (count - 1) as f32;
                while segment + 2 < lengths.len() && lengths[segment + 1] < wanted {
                    segment += 1;
                }
                let span = (lengths[segment + 1] - lengths[segment]).max(1e-9);
                let t = ((wanted - lengths[segment]) / span).clamp(0.0, 1.0);
                let (a, b) = (path[segment], path[segment + 1]);
                Point::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)
            })
            .collect()
    }

    /// Bends the straight line into a curve that still looks straight:
    /// control points at a third and two thirds of it.
    pub fn bend_straight(&mut self) {
        self.bend = Some([self.line_point(1.0 / 3.0), self.line_point(2.0 / 3.0)]);
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
            bend: None,
            pixels: pixels.clamp(1, MAX_FIXTURE_PIXELS),
            order: ColorOrder::Rgb,
            universe,
            channel,
            brightness: 1.0,
            gamma: 1.0,
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
        let points = strip.pixel_points();
        assert_eq!(points[0], Point::new(0.0, 0.0));
        assert_eq!(points[2], Point::new(0.5, 0.25));
        assert_eq!(points[4], Point::new(1.0, 0.5));
        strip.pixels = 1;
        assert_eq!(strip.pixel_points(), [Point::new(0.5, 0.25)]);
    }

    #[test]
    fn pixels_spread_evenly_along_a_curve() {
        let mut lighting = Lighting::default();
        let id = lighting.add_strip("Bande", 9);
        let strip = lighting.fixture_mut(id).unwrap();
        strip.from = Point::new(0.0, 0.5);
        strip.to = Point::new(1.0, 0.5);
        // Bent without moving: the pixels stay where they were.
        let straight = strip.pixel_points();
        strip.bend_straight();
        for (a, b) in straight.iter().zip(strip.pixel_points()) {
            assert!((a.x - b.x).abs() < 1e-3 && (a.y - b.y).abs() < 1e-3);
        }
        // An arch: ends in place, the middle pixel at its top, and equal
        // gaps between neighbours.
        strip.bend = Some([Point::new(0.0, 0.0), Point::new(1.0, 0.0)]);
        let points = strip.pixel_points();
        assert_eq!(points[0], Point::new(0.0, 0.5));
        assert!((points[8].x - 1.0).abs() < 1e-4);
        assert!((points[4].x - 0.5).abs() < 1e-3 && (points[4].y - 0.125).abs() < 1e-3);
        let gap = |i: usize| {
            ((points[i + 1].x - points[i].x).powi(2) + (points[i + 1].y - points[i].y).powi(2))
                .sqrt()
        };
        for i in 1..8 {
            assert!((gap(i) - gap(0)).abs() < 2e-3, "{i}");
        }
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
