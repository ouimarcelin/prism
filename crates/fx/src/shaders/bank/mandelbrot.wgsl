// Mandelbrot set: points c whose orbit z ← z² + c stays bounded; outside,
// a smooth escape count, cycled through the palette. Depth zooms in by
// powers of two around the centre.

fn field(p: vec2<f32>, t: f32) -> f32 {
    let c = vec2<f32>(param(0u), param(1u)) + p * 3.0 / exp2(param(2u));
    let limit = u32(clamp(round(param(3u)), 16.0, 1000.0));
    var z = vec2<f32>(0.0);
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
    return fract(smooth_count * 0.02 * param(4u) - t * 0.1);
}
