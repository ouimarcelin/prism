// Moves, scales and turns the image around its center. Position is in
// image widths and heights; what falls outside of the image is transparent.

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let offset = vec2<f32>(param(0u), -param(1u)) * params.size;
    let point = rotate(from_uv(in.uv) - offset, -radians(param(3u))) / param(2u);
    return sample(to_uv(point));
}
