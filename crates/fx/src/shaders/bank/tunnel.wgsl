// Tunnel: the plane seen down a pipe, depth from the inverse of the
// distance to the centre; round or square, twisted, with a pattern on
// its walls, dark in the distance.

fn field(p: vec2<f32>, t: f32) -> f32 {
    var r = length(p);
    if u32(param(0u)) == 1u {
        r = max(abs(p.x), abs(p.y));
    }
    r = max(r, 1e-3);
    let depth = 0.25 / r + t;
    let around = atan2(p.y, p.x) / (2.0 * PI) + param(1u) * depth * 0.1;
    let u = around * round(param(3u));
    var value: f32;
    switch u32(param(2u)) {
        case 1u: {
            value = wave_shape(depth * 2.0, 2u, 0.5, 0.2);
        }
        case 2u: {
            value = wave_shape(u, 2u, 0.5, 0.2);
        }
        case 3u: {
            value = fractal(vec3<f32>(fract(around) * 8.0, depth * 2.0, 0.0), 2u, 0u, 4.0, 2.0, 0.5, 1.0);
        }
        default: {
            let checker = (floor(u) + floor(depth * 2.0)) % 2.0;
            value = abs(checker);
        }
    }
    let fog = param(4u);
    return value * mix(1.0, clamp(r * 2.5, 0.0, 1.0), fog);
}
