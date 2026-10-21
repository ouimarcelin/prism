// Caustics: light focused by a moving water surface, drawn as the bright
// edges of drifting cells, bent by noise, in one to three layers.

fn field(p: vec2<f32>, t: f32) -> f32 {
    let scale = param(0u);
    let bend = param(1u);
    let sharp = max(param(2u), 1e-3);
    let layers = u32(clamp(round(param(3u)), 1.0, 3.0));
    let push = vec2<f32>(
        base_noise(vec3<f32>(p * 2.0, t * 0.3), 2u),
        base_noise(vec3<f32>(p * 2.0 + vec2<f32>(5.2, 1.3), t * 0.3), 2u),
    );
    var light = 0.0;
    for (var layer = 0u; layer < layers; layer += 1u) {
        let l = f32(layer);
        let q = (p + bend * 0.15 * push) * scale * (1.0 + 0.6 * l) + vec2<f32>(l * 7.1, l * 3.3);
        let cells = worley(q, t * (1.0 + 0.3 * l), 1.0, 1.2, 0u);
        let edge = 1.0 - smoothstep(0.0, sharp, cells.y - cells.x);
        light += edge * edge / (1.0 + l);
    }
    return clamp(light, 0.0, 1.0);
}
