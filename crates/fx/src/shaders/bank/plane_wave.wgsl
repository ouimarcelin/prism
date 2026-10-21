// Plane wave: one wave shape repeated along a direction, travelling one
// wavelength per unit of time.

fn field(p: vec2<f32>, t: f32) -> f32 {
    let angle = radians(param(1u));
    let along = dot(p, vec2<f32>(cos(angle), sin(angle)));
    let phase = along / max(param(2u), 1e-3) - t;
    return wave_shape(phase, u32(param(0u)), param(3u), param(4u));
}
