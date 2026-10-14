// Big square pixels, each the color of its center.

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let cell = max(param(0u), 1.0);
    let center = (floor(in.uv * params.size / cell) + vec2<f32>(0.5)) * cell;
    return sample_clamped(center / params.size);
}
