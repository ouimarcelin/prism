// Gabor noise (Lagae et al., 2009): random impulses, each a small
// oriented wave under a Gaussian, added up; fibrous, directional
// textures.

fn field(p: vec2<f32>, t: f32) -> f32 {
    let scale = param(5u);
    let q = p * scale;
    let cell = floor(q);
    let impulses = u32(clamp(round(param(4u)), 1.0, 8.0));
    let frequency = param(2u) / scale;
    let width = param(3u);
    let base = radians(param(0u));
    let spread = param(1u);
    var sum = 0.0;
    for (var y = -1; y <= 1; y += 1) {
        for (var x = -1; x <= 1; x += 1) {
            let neighbour = cell + vec2<f32>(f32(x), f32(y));
            for (var index = 0u; index < impulses; index += 1u) {
                let random = hash33(vec3<f32>(neighbour, f32(index) + 0.5));
                let d = q - (neighbour + random.xy);
                let angle = base + (random.z - 0.5) * spread * PI;
                let along = dot(d, vec2<f32>(cos(angle), sin(angle)));
                let polarity = select(-1.0, 1.0, hash31(vec3<f32>(neighbour, f32(index) + 40.5)) > 0.5);
                let envelope = exp(-PI * dot(d, d) * width * width);
                sum += polarity * envelope * cos(2.0 * PI * (frequency * along) + t * 2.0);
            }
        }
    }
    return clamp(0.5 + 0.35 * sum / sqrt(f32(impulses)), 0.0, 1.0);
}
