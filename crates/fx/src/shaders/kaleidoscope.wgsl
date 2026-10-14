// A wedge of the image repeated and reflected around the center.

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let point = from_uv(in.uv);
    let radius = length(point) / param(2u);
    let wedge = 2.0 * PI / max(round(param(0u)), 2.0);
    var angle = atan2(point.y, point.x) - radians(param(1u));
    angle = angle - wedge * floor(angle / wedge);
    // Every other wedge is reflected, so neighbours meet without a seam.
    angle = min(angle, wedge - angle);
    let folded = vec2<f32>(cos(angle), sin(angle)) * radius;
    return sample_clamped(to_uv(folded));
}
