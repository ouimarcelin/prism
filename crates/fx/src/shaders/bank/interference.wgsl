// Interference: two to eight sources of circular waves, laid on a circle,
// a line or at random, swinging around their place; the waves add up.
// Shown as the sum, or as its strength (where the waves cancel: dark).

fn source_at(index: u32, count: u32, t: f32) -> vec2<f32> {
    let spread = param(2u);
    let i = f32(index);
    var at: vec2<f32>;
    switch u32(param(1u)) {
        case 1u: {
            let x = select(i / f32(count - 1u) * 2.0 - 1.0, 0.0, count < 2u);
            at = vec2<f32>(x * spread, 0.0);
        }
        case 2u: {
            at = (hash33(vec3<f32>(i, 17.0, bank_setting(C_SEED))).xy * 2.0 - 1.0) * spread;
        }
        default: {
            let a = 2.0 * PI * i / f32(count);
            at = vec2<f32>(cos(a), sin(a)) * spread;
        }
    }
    let swing = param(3u) * 0.15;
    return at + swing * vec2<f32>(sin(t * 0.7 + i * 2.1), cos(t * 0.53 + i * 1.3));
}

fn field(p: vec2<f32>, t: f32) -> f32 {
    let count = u32(clamp(round(param(0u)), 2.0, 8.0));
    let wavelength = max(param(4u), 1e-3);
    var sum = 0.0;
    for (var index = 0u; index < count; index += 1u) {
        let r = length(p - source_at(index, count, t));
        sum += cos(2.0 * PI * (r / wavelength - t + param(5u) * f32(index)));
    }
    let v = sum / f32(count);
    if u32(param(6u)) == 1u {
        return clamp(abs(v) * 1.5, 0.0, 1.0);
    }
    return 0.5 + 0.5 * v;
}
