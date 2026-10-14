// Each channel reduced to a few levels.

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let pixel = sample(in.uv);
    let steps = max(round(param(0u)), 2.0) - 1.0;
    let rgb = floor(unpremultiply(pixel) * steps + vec3<f32>(0.5)) / steps;
    return premultiply(rgb, pixel.a);
}
