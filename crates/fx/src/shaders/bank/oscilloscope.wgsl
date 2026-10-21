// Oscilloscope: the trace of a signal (sine, triangle, square, sawtooth or
// noise) running across a graticule, with a glow.

fn scope_signal(x: f32, t: f32) -> f32 {
    let shape = u32(param(0u));
    let phase = x * param(1u) - t;
    var value: f32;
    if shape == 4u {
        value = fractal(vec3<f32>(phase, 0.0, t * 0.3), 2u, 0u, 3.0, 2.0, 0.5, 1.0) * 2.0 - 1.0;
    } else {
        value = wave_shape(phase, shape, 0.5, 0.15) * 2.0 - 1.0;
    }
    let noise = param(2u) * 0.15 * value_noise(vec3<f32>(x * 80.0, t * 20.0, 0.0));
    return param(3u) * value + noise;
}

fn field(p: vec2<f32>, t: f32) -> f32 {
    let e = 0.002;
    let y = scope_signal(p.x, t);
    let slope = (scope_signal(p.x + e, t) - scope_signal(p.x - e, t)) / (2.0 * e);
    let distance = abs(p.y - y) / sqrt(1.0 + slope * slope);
    let thickness = param(5u);
    let line = 1.0 - smoothstep(thickness * 0.5, thickness * 0.5 + bank_pixel() * 1.5, distance);
    let glow = param(4u) * exp(-distance / max(param(4u) * 0.04, 1e-4));
    // Graticule: ten divisions on the height.
    let grid = abs(fract(p * 10.0 + vec2<f32>(0.5)) - vec2<f32>(0.5)) / 10.0;
    let mark = 1.0 - smoothstep(0.0, bank_pixel() * 1.5, min(grid.x, grid.y));
    return clamp(max(max(line, glow), mark * param(6u) * 0.4), 0.0, 1.0);
}
