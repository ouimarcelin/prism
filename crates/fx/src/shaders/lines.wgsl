// Parallel white lines on black, moving with the beat.

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let angle = radians(param(2u));
    let across = rotate(from_uv(in.uv), -angle).x / max(params.size.x, params.size.y);
    let position = fract(across * param(0u) - params.beat * param(3u));
    // Lines softened over one pixel at either edge.
    let soft = param(0u) / max(params.size.x, params.size.y);
    let width = param(1u);
    let level = smoothstep(0.0, soft, position) * (1.0 - smoothstep(width - soft, width, position));
    return vec4<f32>(vec3<f32>(level), 1.0);
}
