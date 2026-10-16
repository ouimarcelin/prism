// From one color to another along a direction, sliding with the beat.

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let angle = radians(param(2u));
    let direction = vec2<f32>(cos(angle), -sin(angle));
    // 0 to 1 across the image along the direction, then moved by the beat
    // and folded back so the colors flow without a jump.
    let across = dot(in.uv - vec2<f32>(0.5), direction) + 0.5 + params.beat * param(4u);
    let fold = 1.0 - abs(1.0 - 2.0 * fract(across * 0.5));
    let start = hsv_to_rgb(vec3<f32>(param(0u) / 360.0, param(3u), 1.0));
    let end = hsv_to_rgb(vec3<f32>(param(1u) / 360.0, param(3u), 1.0));
    return vec4<f32>(mix(start, end, fold), 1.0);
}
