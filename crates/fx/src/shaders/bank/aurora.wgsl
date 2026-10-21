// Aurora: curtains of light, each a waving lower edge with rays fading
// upwards, streaked and shimmering.

fn field(p: vec2<f32>, t: f32) -> f32 {
    let curtains = u32(clamp(round(param(0u)), 1.0, 5.0));
    var light = 0.0;
    for (var index = 0u; index < curtains; index += 1u) {
        let i = f32(index);
        let wave = fractal(vec3<f32>(p.x * 1.5 + i * 3.1, i * 2.3, t * 0.15), 2u, 0u, 3.0, 2.0, 0.5, 1.0) - 0.5;
        let edge = param(4u) - 0.1 * i + param(1u) * 0.6 * wave;
        let d = p.y - edge;
        let rays = exp(-max(d, 0.0) / max(param(2u), 0.01)) * smoothstep(-0.03, 0.0, d);
        let streaks = 0.6 + 0.4 * value_noise(vec3<f32>(p.x * 60.0 + i * 11.0, 0.0, t * param(3u) * 3.0));
        light += rays * mix(1.0, streaks, param(3u)) * (1.0 - 0.15 * i);
    }
    return clamp(light / (0.6 + 0.4 * f32(curtains)), 0.0, 1.0);
}
