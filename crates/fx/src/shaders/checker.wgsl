// Black and white squares, swapping on the beat.

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let size = max(params.size.x, params.size.y) / max(param(0u), 1.0);
    let cell = floor(in.uv * params.size / size);
    let swap = floor(params.beat * param(1u));
    let white = fract((cell.x + cell.y + swap) * 0.5) > 0.25;
    return vec4<f32>(vec3<f32>(select(0.0, 1.0, white)), 1.0);
}
