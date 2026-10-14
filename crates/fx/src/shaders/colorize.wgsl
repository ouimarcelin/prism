// The brightness of the image painted in one color.

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let pixel = sample(in.uv);
    let rgb = unpremultiply(pixel);
    let brightness = dot(rgb, LUMA);
    let painted = hsv_to_rgb(vec3<f32>(param(0u) / 360.0, param(1u), brightness));
    return premultiply(mix(rgb, painted, param(2u)), pixel.a);
}
