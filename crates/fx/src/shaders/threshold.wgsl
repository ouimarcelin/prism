// Black below a brightness, white above, with a soft transition.

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let pixel = sample(in.uv);
    let brightness = dot(unpremultiply(pixel), LUMA);
    let softness = max(param(1u), 0.0001);
    let level = smoothstep(param(0u) - softness, param(0u) + softness, brightness);
    return premultiply(vec3<f32>(level), pixel.a);
}
