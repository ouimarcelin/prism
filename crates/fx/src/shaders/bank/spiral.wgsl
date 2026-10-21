// Spiral: arms turning around the centre, Archimedean (evenly spaced) or
// logarithmic (widening), drawn with a wave shape.

fn field(p: vec2<f32>, t: f32) -> f32 {
    let arms = round(param(0u));
    let r = max(length(p), 1e-4);
    let a = atan2(p.y, p.x) / (2.0 * PI);
    var radial = r;
    if u32(param(2u)) == 1u {
        radial = log(r) * 0.25;
    }
    let phase = arms * a + param(1u) * radial - t;
    return wave_shape(phase, u32(param(3u)), param(4u), 0.3);
}
