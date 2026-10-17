//! Pixel mapping: the colours taken for each pixel of the fixtures, laid
//! out in DMX universes.

use std::collections::BTreeMap;

use prism_core::Fixture;

use crate::{UNIVERSE_SIZE, Universe};

/// Pixels whose colour must be taken, in order: every pixel of every
/// enabled fixture, first fixture first.
pub fn pixel_count(fixtures: &[Fixture]) -> usize {
    fixtures
        .iter()
        .filter(|fixture| fixture.enabled)
        .map(|fixture| usize::from(fixture.pixels))
        .sum()
}

/// The universes holding `colors` (one per pixel, in the order of
/// [`pixel_count`]) for `fixtures`. A pixel whose channels do not fit in
/// what is left of a universe starts the next one, from channel 1.
pub fn universes(fixtures: &[Fixture], colors: &[[u8; 3]]) -> BTreeMap<u16, Universe> {
    let mut universes: BTreeMap<u16, Universe> = BTreeMap::new();
    let mut colors = colors.iter();
    for fixture in fixtures.iter().filter(|fixture| fixture.enabled) {
        let width = fixture.order.channels();
        let mut universe = fixture.universe;
        let mut offset = usize::from(fixture.channel.clamp(1, UNIVERSE_SIZE as u16)) - 1;
        let brightness = fixture.brightness.clamp(0.0, 1.0);
        let gamma = fixture.gamma.clamp(0.1, 5.0);
        // One table per fixture: 256 powers instead of one per channel.
        let table: [u8; 256] = std::array::from_fn(|value| {
            ((value as f32 / 255.0).powf(gamma) * brightness * 255.0).round() as u8
        });
        for _ in 0..fixture.pixels {
            let Some(color) = colors.next() else {
                return universes;
            };
            if offset + width > UNIVERSE_SIZE {
                let Some(next) = universe.checked_add(1) else {
                    break;
                };
                universe = next;
                offset = 0;
            }
            let color = color.map(|value| table[usize::from(value)]);
            let channels = universes.entry(universe).or_insert([0; UNIVERSE_SIZE]);
            for (index, value) in fixture.order.values(color).into_iter().enumerate() {
                channels[offset + index] = value;
            }
            offset += width;
        }
    }
    universes
}

#[cfg(test)]
mod tests {
    use super::*;
    use prism_core::{ColorOrder, Lighting};

    #[test]
    fn pixels_fill_their_universes_and_spill_over() {
        let mut lighting = Lighting::default();
        lighting.add_strip("Bande", 171);
        let lamp = lighting.add_strip("Lampe", 1);
        {
            let lamp = lighting.fixture_mut(lamp).unwrap();
            lamp.universe = 5;
            lamp.channel = 10;
            lamp.order = ColorOrder::Grb;
            lamp.brightness = 0.5;
        }
        let fixtures = lighting.fixtures();
        assert_eq!(pixel_count(fixtures), 172);
        let mut colors = vec![[1, 2, 3]; 171];
        colors.push([200, 100, 50]);
        let universes = universes(fixtures, &colors);
        // 170 RGB pixels in universe 0 (510 channels), the 171st in 1.
        assert_eq!(universes[&0][..3], [1, 2, 3]);
        assert_eq!(universes[&0][507..512], [1, 2, 3, 0, 0]);
        assert_eq!(universes[&1][..4], [1, 2, 3, 0]);
        // The lamp: channel 10 of universe 5, green first, half bright.
        assert_eq!(universes[&5][9..12], [50, 100, 25]);
    }

    #[test]
    fn gamma_darkens_mid_tones_only() {
        let mut lighting = Lighting::default();
        let strip = lighting.add_strip("Bande", 1);
        lighting.fixture_mut(strip).unwrap().gamma = 2.0;
        let universes = universes(lighting.fixtures(), &[[255, 128, 0]]);
        // 128/255 squared: about a quarter.
        assert_eq!(universes[&0][..3], [255, 64, 0]);
    }
}
