// Vortex: arms turned more and more towards the centre, the twist fading
// with the distance.

fn field(p: vec2<f32>, t: f32) -> f32 {
    let r = length(p);
    let a = atan2(p.y, p.x) / (2.0 * PI);
    let turn = param(0u) * exp(-param(2u) * r * 4.0);
    let phase = round(param(1u)) * (a + turn / (2.0 * PI)) - t * 0.25;
    return wave_shape(phase, u32(param(3u)), param(4u), 0.3);
}
