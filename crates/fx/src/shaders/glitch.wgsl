// Horizontal bands of the image pushed sideways, their colors split; the
// bands change a set number of times per beat.

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let amount = param(0u);
    let bands = max(round(param(1u)), 1.0);
    let band = floor(in.uv.y * bands);
    let moment = floor(params.beat * param(2u));
    let moved = select(0.0, 1.0, hash(band * 7.13 + moment * 13.7) > 1.0 - amount);
    let shift = (hash(band * 3.1 + moment * 5.3) - 0.5) * 0.3 * amount * moved;
    let split = 0.01 * amount * moved;
    let x = in.uv.x + shift;
    let pixel = sample_clamped(vec2<f32>(fract(x), in.uv.y));
    let red = sample_clamped(vec2<f32>(fract(x + split), in.uv.y)).r;
    let blue = sample_clamped(vec2<f32>(fract(x - split), in.uv.y)).b;
    return vec4<f32>(red, pixel.g, blue, pixel.a);
}
