// Inverted colors, mixed with the original by an amount.

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let pixel = sample(in.uv);
    let rgb = unpremultiply(pixel);
    return premultiply(mix(rgb, vec3<f32>(1.0) - rgb, param(0u)), pixel.a);
}
