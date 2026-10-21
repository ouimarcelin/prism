// Starfield: layers of stars coming towards the viewer from the centre,
// stretched into streaks as they pass.

fn field(p: vec2<f32>, t: f32) -> f32 {
    let layers = u32(clamp(round(param(1u)), 1.0, 8.0));
    let density = param(0u);
    let trail = param(2u);
    let size = param(3u) * 0.003;
    let outward = normalize(p + vec2<f32>(1e-5));
    var light = 0.0;
    for (var layer = 0u; layer < layers; layer += 1u) {
        let z = 1.0 - fract(f32(layer) / f32(layers) + t * 0.2);
        let depth = max(z, 0.02);
        let q = p * depth;
        let cells = 6.0;
        let cell = floor(q * cells);
        for (var y = -1; y <= 1; y += 1) {
            for (var x = -1; x <= 1; x += 1) {
                let neighbour = cell + vec2<f32>(f32(x), f32(y));
                let random = hash33(vec3<f32>(neighbour, f32(layer) * 7.0 + floor(f32(layer) / f32(layers) + t * 0.2) * 31.0));
                if random.z > density {
                    continue;
                }
                let star = (neighbour + random.xy) / cells;
                let d = (q - star) / depth;
                let along = dot(d, outward);
                let side = d.x * outward.y - d.y * outward.x;
                let length_ = size / depth * (1.0 + trail * 30.0 * (1.0 - z));
                let width = size / depth;
                let shape = exp(-(side * side) / (width * width) - (along * along) / (length_ * length_));
                light += shape * smoothstep(1.0, 0.8, z);
            }
        }
    }
    return clamp(light, 0.0, 1.0);
}
