// The image rippled by a sine wave that travels with the beat.

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var uv = in.uv;
    let amplitude = param(0u);
    let waves = param(1u);
    let travel = params.beat * param(2u);
    if u32(param(3u)) == 0u {
        uv.x += amplitude * sin(2.0 * PI * (waves * uv.y + travel));
    } else {
        uv.y += amplitude * sin(2.0 * PI * (waves * uv.x + travel));
    }
    return sample_clamped(uv);
}
