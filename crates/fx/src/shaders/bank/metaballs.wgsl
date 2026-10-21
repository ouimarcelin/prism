// Metaballs: soft balls on looping paths whose fields add up; the shape
// is where the sum passes the threshold, with a glow around it.

fn field(p: vec2<f32>, t: f32) -> f32 {
    let balls = u32(clamp(round(param(0u)), 2.0, 16.0));
    let size = param(1u);
    var sum = 0.0;
    for (var index = 0u; index < balls; index += 1u) {
        let random = hash33(vec3<f32>(f32(index), 3.0, 11.0));
        let pace = t * param(4u) * (0.3 + 0.5 * random.z);
        let centre = vec2<f32>(
            0.6 * sin(pace * (1.0 + random.x) + random.y * 6.28),
            0.35 * cos(pace * (0.7 + random.y) + random.x * 6.28),
        );
        let d = p - centre;
        sum += size * size / max(dot(d, d), 1e-5);
    }
    let threshold = param(2u);
    let soft = param(3u);
    let inside = smoothstep(threshold * (1.0 - soft * 0.5), threshold * (1.0 + soft * 0.1) + 1e-3, sum);
    let glow = clamp(sum / threshold, 0.0, 1.0) * 0.4 * soft;
    return max(inside, glow);
}
