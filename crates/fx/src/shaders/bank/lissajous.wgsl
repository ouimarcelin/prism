// Lissajous figure: the curve of two sines of frequencies a and b, out of
// phase; the phase turns with time. Drawn as a line with a glow.

fn field(p: vec2<f32>, t: f32) -> f32 {
    let a = round(param(0u));
    let b = round(param(1u));
    let delta = 2.0 * PI * (param(2u) + t * 0.125);
    let size = param(5u);
    var nearest = 1e9;
    for (var index = 0u; index < 600u; index += 1u) {
        let s = f32(index) / 600.0 * 2.0 * PI;
        let point = size * vec2<f32>(sin(a * s + delta), sin(b * s));
        nearest = min(nearest, length(p - point));
    }
    let thickness = param(3u);
    let line = 1.0 - smoothstep(thickness * 0.5, thickness * 0.5 + bank_pixel() * 1.5, nearest);
    let glow = param(4u) * exp(-nearest / max(param(4u) * 0.05, 1e-4));
    return clamp(max(line, glow), 0.0, 1.0);
}
