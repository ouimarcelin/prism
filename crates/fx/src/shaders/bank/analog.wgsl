// Analog signal: a picture (bars, ramp or noise) as a worn video signal
// shows it: scan lines, lines pushed sideways, a dark bar rolling up,
// dropouts and snow.

fn field(p: vec2<f32>, t: f32) -> f32 {
    let lines = param(3u);
    let line = floor((p.y + 0.5) * lines);
    let frame = floor(t * 15.0);
    let jitter = param(1u);
    let shift = jitter * 0.05 * (hash31(vec3<f32>(line, frame, 1.0)) - 0.5)
        + jitter * 0.02 * sin(p.y * 20.0 + t * 5.0);
    let x = p.x + shift;
    var value: f32;
    switch u32(param(5u)) {
        case 1u: {
            value = clamp(x + 0.5, 0.0, 1.0);
        }
        case 2u: {
            value = fractal(vec3<f32>(x * 3.0, p.y * 3.0, t * 0.2), 2u, 0u, 4.0, 2.0, 0.5, 1.0);
        }
        default: {
            value = 1.0 - floor(fract(x + 0.5) * 7.0) / 6.0;
        }
    }
    // The bar of the vertical blanking, rolling.
    let roll = fract(p.y + 0.5 + t * param(0u) * 0.5);
    value *= 0.55 + 0.45 * smoothstep(0.0, 0.08, roll);
    // Scan lines.
    let across = fract((p.y + 0.5) * lines);
    value *= 0.75 + 0.25 * sin(PI * across);
    // Dropouts: whole lines torn white.
    if hash31(vec3<f32>(line, floor(t * 12.0), 2.0)) < param(2u) * 0.08 {
        value = mix(value, hash31(vec3<f32>(floor(x * 200.0), line, frame)), 0.8);
    }
    let snow = hash31(vec3<f32>(floor(p * params.size.y), frame));
    return clamp(mix(value, snow, param(4u) * 0.4), 0.0, 1.0);
}
