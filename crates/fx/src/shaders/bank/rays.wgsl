// Rays: sectors around the centre, turning, fading towards the centre.

fn field(p: vec2<f32>, t: f32) -> f32 {
    let a = atan2(p.y, p.x) / (2.0 * PI) - param(1u) * t;
    let value = wave_shape(round(param(0u)) * a, u32(param(2u)), param(3u), param(4u));
    let fade = mix(1.0, smoothstep(0.0, 0.3, length(p)), param(5u));
    return value * fade;
}
