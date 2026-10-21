// Moiré: two gratings (lines, circles or grids) laid one on the other,
// the second slightly turned, finer or moved; the product shows the
// large fringes. Motion turns the second grating with time.

fn grating(p: vec2<f32>, pattern: u32, pitch: f32, sharp: f32) -> f32 {
    var g: f32;
    switch pattern {
        case 1u: {
            g = 0.5 + 0.5 * cos(2.0 * PI * length(p) / pitch);
        }
        case 2u: {
            let c = 0.5 + 0.5 * cos(2.0 * PI * p / pitch);
            g = c.x * c.y;
        }
        default: {
            g = 0.5 + 0.5 * cos(2.0 * PI * p.x / pitch);
        }
    }
    let width = 0.5 * (1.0 - sharp) + 1e-3;
    return smoothstep(0.5 - width, 0.5 + width, g);
}

fn field(p: vec2<f32>, t: f32) -> f32 {
    let pattern = u32(param(0u));
    let pitch = max(param(1u), 1e-3);
    let sharp = param(6u);
    let turn = radians(param(2u) + param(5u) * t);
    let shift = vec2<f32>(param(4u), 0.0);
    let first = grating(p + shift * 0.5, pattern, pitch, sharp);
    let second = grating(rotate(p - shift * 0.5, turn), pattern, pitch * (1.0 + param(3u)), sharp);
    return first * second;
}
