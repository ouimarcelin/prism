// Smoke: a fractal noise rising and stirred sideways, thinning with the
// height; from a narrow source or across the whole frame.

fn field(p: vec2<f32>, t: f32) -> f32 {
    let height = clamp(p.y + 0.5, 0.0, 1.0);
    var q = p * param(0u);
    q.y -= t * param(1u);
    let stir = base_noise(vec3<f32>(q * 0.7, t * 0.2), 2u);
    q.x += param(2u) * stir;
    let n = fractal(vec3<f32>(q, t * 0.15), 2u, 0u, 5.0, 2.0, 0.5, 1.0);
    var value = clamp((n - 0.5 + 0.5 * param(3u) - 0.25) * 2.0, 0.0, 1.0);
    value *= 1.0 - 0.7 * height;
    let spread = param(4u);
    if spread < 1.0 {
        let width = (spread * 0.4 + 0.02) * (0.3 + height * 1.4);
        value *= exp(-(p.x * p.x) / (width * width));
    }
    return value;
}
