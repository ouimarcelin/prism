//! CPU implementation of the compositing formulas.
//!
//! It mirrors `composite.wgsl` and serves as the expected result in tests of
//! the GPU path. Colors are premultiplied RGBA in display space.

use prism_core::BlendMode;

/// Premultiplied RGBA color.
pub type Rgba = [f32; 4];

fn unpremultiply(color: Rgba) -> [f32; 3] {
    if color[3] <= 0.0 {
        [0.0; 3]
    } else {
        [
            color[0] / color[3],
            color[1] / color[3],
            color[2] / color[3],
        ]
    }
}

/// Blend function B(cb, cs) on straight (non-premultiplied) channels.
fn blend(cb: f32, cs: f32, mode: BlendMode) -> f32 {
    match mode {
        BlendMode::Normal => cs,
        BlendMode::Add => cb + cs,
        BlendMode::Multiply => cb * cs,
        BlendMode::Screen => cb + cs - cb * cs,
    }
}

/// Layer color during a transition: `previous` mixed with `current`.
pub fn transition(previous: Rgba, current: Rgba, progress: f32) -> Rgba {
    let mut out = [0.0; 4];
    for i in 0..4 {
        out[i] = previous[i] * (1.0 - progress) + current[i] * progress;
    }
    out
}

/// Composites `layer`, scaled by `opacity`, over `below`.
pub fn composite(below: Rgba, layer: Rgba, opacity: f32, mode: BlendMode) -> Rgba {
    let alpha_s = layer[3] * opacity;
    let alpha_b = below[3];
    let cs = unpremultiply(layer);
    let cb = unpremultiply(below);
    let mut out = [0.0; 4];
    for i in 0..3 {
        out[i] = alpha_s * (1.0 - alpha_b) * cs[i]
            + alpha_s * alpha_b * blend(cb[i], cs[i], mode)
            + (1.0 - alpha_s) * below[i];
    }
    out[3] = alpha_s + alpha_b * (1.0 - alpha_s);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: Rgba = [0.5, 0.25, 0.75, 1.0];
    const GRAY: Rgba = [0.5, 0.5, 0.5, 1.0];

    fn assert_close(actual: Rgba, expected: Rgba) {
        for i in 0..4 {
            assert!(
                (actual[i] - expected[i]).abs() < 1e-6,
                "expected {expected:?}, got {actual:?}"
            );
        }
    }

    #[test]
    fn blend_modes_on_opaque_colors() {
        assert_close(composite(BASE, GRAY, 1.0, BlendMode::Normal), GRAY);
        assert_close(
            composite(BASE, GRAY, 1.0, BlendMode::Add),
            [1.0, 0.75, 1.25, 1.0],
        );
        assert_close(
            composite(BASE, GRAY, 1.0, BlendMode::Multiply),
            [0.25, 0.125, 0.375, 1.0],
        );
        assert_close(
            composite(BASE, GRAY, 1.0, BlendMode::Screen),
            [0.75, 0.625, 0.875, 1.0],
        );
    }

    #[test]
    fn opacity_interpolates_toward_the_blend_result() {
        assert_close(
            composite(BASE, GRAY, 0.5, BlendMode::Normal),
            [0.5, 0.375, 0.625, 1.0],
        );
        assert_close(composite(BASE, GRAY, 0.0, BlendMode::Multiply), BASE);
    }

    #[test]
    fn transparent_layer_leaves_below_untouched() {
        for mode in BlendMode::ALL {
            assert_close(composite(BASE, [0.0; 4], 1.0, mode), BASE);
        }
    }

    #[test]
    fn layer_alpha_acts_like_opacity() {
        // Premultiplied white at half alpha.
        let half_white = [0.5, 0.5, 0.5, 0.5];
        let black = [0.0, 0.0, 0.0, 1.0];
        assert_close(
            composite(black, half_white, 1.0, BlendMode::Normal),
            [0.5, 0.5, 0.5, 1.0],
        );
    }

    #[test]
    fn layer_over_transparent_keeps_its_own_color() {
        let red = [1.0, 0.0, 0.0, 1.0];
        for mode in BlendMode::ALL {
            assert_close(composite([0.0; 4], red, 1.0, mode), red);
        }
    }
}
