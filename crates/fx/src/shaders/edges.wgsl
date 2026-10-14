// Outlines: brightness changes found with a Sobel filter, light on dark,
// mixed with the image.

fn brightness(uv: vec2<f32>) -> f32 {
    return dot(sample_clamped(uv).rgb, LUMA);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let texel = 1.0 / params.size;
    let left = in.uv - vec2<f32>(texel.x, 0.0);
    let right = in.uv + vec2<f32>(texel.x, 0.0);
    let up = in.uv - vec2<f32>(0.0, texel.y);
    let down = in.uv + vec2<f32>(0.0, texel.y);
    let across = brightness(right - vec2<f32>(0.0, texel.y)) + 2.0 * brightness(right)
        + brightness(right + vec2<f32>(0.0, texel.y))
        - brightness(left - vec2<f32>(0.0, texel.y)) - 2.0 * brightness(left)
        - brightness(left + vec2<f32>(0.0, texel.y));
    let along = brightness(down - vec2<f32>(texel.x, 0.0)) + 2.0 * brightness(down)
        + brightness(down + vec2<f32>(texel.x, 0.0))
        - brightness(up - vec2<f32>(texel.x, 0.0)) - 2.0 * brightness(up)
        - brightness(up + vec2<f32>(texel.x, 0.0));
    let edge = clamp(length(vec2<f32>(across, along)) * param(0u), 0.0, 1.0);
    let pixel = sample(in.uv);
    return mix(pixel, premultiply(vec3<f32>(edge), pixel.a), param(1u));
}
