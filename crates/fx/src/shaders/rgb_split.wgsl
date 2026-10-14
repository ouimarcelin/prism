// Red and blue shifted apart from green, in a direction; the edge of the
// image is repeated outside of it.

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let angle = radians(param(1u));
    let offset = vec2<f32>(cos(angle), -sin(angle)) * param(0u) / params.size;
    let red = sample_clamped(in.uv + offset);
    let green = sample_clamped(in.uv);
    let blue = sample_clamped(in.uv - offset);
    return vec4<f32>(red.r, green.g, blue.b, max(max(red.a, green.a), blue.a));
}
