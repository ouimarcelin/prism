// The center of the image turned more than its edge, like water draining.

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let point = from_uv(in.uv);
    let radius = max(param(1u) * 0.5 * length(params.size), 1.0);
    let closeness = clamp(1.0 - length(point) / radius, 0.0, 1.0);
    let turned = rotate(point, radians(param(0u)) * closeness * closeness);
    return sample(to_uv(turned));
}
