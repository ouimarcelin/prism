// Warped grid: grid lines pushed around by a noise that evolves.

fn field(p: vec2<f32>, t: f32) -> f32 {
    let z = t * param(4u);
    let s = param(3u);
    let push = vec2<f32>(
        base_noise(vec3<f32>(p * s, z), 2u),
        base_noise(vec3<f32>(p * s + vec2<f32>(5.2, 1.3), z), 2u),
    );
    let q = (p + param(2u) * 0.1 * push) * param(0u);
    let f = abs(fract(q + vec2<f32>(0.5)) - vec2<f32>(0.5));
    let soft = bank_pixel() * param(0u) * 1.5;
    let half = param(1u) * 0.5;
    return 1.0 - smoothstep(half - soft, half + soft, min(f.x, f.y));
}
