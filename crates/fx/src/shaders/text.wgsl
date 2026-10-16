// A text: its coverage, rasterized on the CPU, is the source image. Placed
// by its center, lines `param(0)` of the image height high, in one color.

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let mask_size = vec2<f32>(textureDimensions(source));
    // Pixels of the mask per pixel of the image.
    let scale = params.mask_line / max(param(0u) * params.size.y, 0.001);
    let center = vec2<f32>(0.5 + param(1u) * 0.5, 0.5 - param(2u) * 0.5) * params.size;
    let uv = (in.uv * params.size - center) * scale / mask_size + vec2<f32>(0.5);
    let coverage = sample(uv).a;
    let rgb = hsv_to_rgb(vec3<f32>(param(3u) / 360.0, param(4u), param(5u)));
    return premultiply(rgb, coverage);
}
