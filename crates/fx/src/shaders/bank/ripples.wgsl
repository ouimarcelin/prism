// Ripples: circles leaving a centre, one wavelength per unit of time,
// fading with the distance (damping) towards mid grey.

fn field(p: vec2<f32>, t: f32) -> f32 {
    let r = length(p - vec2<f32>(param(1u), param(2u)));
    let phase = r / max(param(3u), 1e-3) - t;
    let w = wave_shape(phase, u32(param(0u)), param(5u), 0.3);
    return 0.5 + (w - 0.5) * exp(-param(4u) * r);
}
