// Julia set: the orbit z ← z² + c from each point, for a constant c that
// turns slowly around the origin; a smooth escape count outside.

fn field(p: vec2<f32>, t: f32) -> f32 {
    let angle = radians(param(1u)) + t * param(4u) * 2.0 * PI * 0.1;
    let c = param(0u) * vec2<f32>(cos(angle), sin(angle));
    let limit = u32(clamp(round(param(2u)), 16.0, 500.0));
    var z = p * 2.5;
    var count = 0u;
    for (; count < limit; count += 1u) {
        z = vec2<f32>(z.x * z.x - z.y * z.y, 2.0 * z.x * z.y) + c;
        if dot(z, z) > 256.0 {
            break;
        }
    }
    if count >= limit {
        return 0.0;
    }
    let smooth_count = f32(count) - log2(log2(dot(z, z)) * 0.5);
    return fract(smooth_count * 0.02 * param(3u));
}
