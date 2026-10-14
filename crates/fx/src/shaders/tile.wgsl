// The image repeated in a grid, every other tile reflected on request.

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let counts = max(round(vec2<f32>(param(0u), param(1u))), vec2<f32>(1.0));
    let cell = floor(in.uv * counts);
    var uv = fract(in.uv * counts);
    if u32(param(2u)) == 1u {
        let odd = fract(cell * 0.5) > vec2<f32>(0.25);
        uv = select(uv, vec2<f32>(1.0) - uv, odd);
    }
    return sample_clamped(uv);
}
