// White rings on black, growing from the center with the beat.

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let radius = length(from_uv(in.uv)) / (0.5 * length(params.size));
    let position = fract(radius * param(0u) - params.beat * param(2u));
    let soft = param(0u) / (0.5 * length(params.size));
    let width = param(1u);
    let level = smoothstep(0.0, soft, position) * (1.0 - smoothstep(width - soft, width, position));
    return vec4<f32>(vec3<f32>(level), 1.0);
}
