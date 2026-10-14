// Brightness, contrast, saturation and hue.

// Rotation of a color around the gray axis.
fn rotate_hue(color: vec3<f32>, degrees: f32) -> vec3<f32> {
    let angle = radians(degrees);
    let axis = vec3<f32>(0.57735027);
    let c = cos(angle);
    return color * c + cross(axis, color) * sin(angle) + axis * dot(axis, color) * (1.0 - c);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let pixel = sample(in.uv);
    var rgb = unpremultiply(pixel);
    rgb = (rgb - vec3<f32>(0.5)) * param(1u) + vec3<f32>(0.5) + vec3<f32>(param(0u));
    rgb = mix(vec3<f32>(dot(rgb, LUMA)), rgb, param(2u));
    rgb = rotate_hue(rgb, param(3u));
    return premultiply(clamp(rgb, vec3<f32>(0.0), vec3<f32>(1.0)), pixel.a);
}
