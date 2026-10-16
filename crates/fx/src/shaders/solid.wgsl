// One color everywhere.

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let rgb = hsv_to_rgb(vec3<f32>(param(0u) / 360.0, param(1u), param(2u)));
    return premultiply(rgb, param(3u));
}
