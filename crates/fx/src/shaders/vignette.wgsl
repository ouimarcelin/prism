// Edges and corners darkened.

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let pixel = sample(in.uv);
    // 0 at the center, 1 in the corners.
    let spread = length(in.uv - vec2<f32>(0.5)) * 1.41421356;
    let start = 1.0 - param(1u);
    let shade = smoothstep(start, start + 0.5, spread);
    return vec4<f32>(pixel.rgb * (1.0 - param(0u) * shade), pixel.a);
}
