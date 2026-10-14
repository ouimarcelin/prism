// The image flashes on the beat: shown for part of each flash, replaced by
// black, white or nothing for the rest.

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let pixel = sample(in.uv);
    let phase = fract(params.beat * param(0u));
    if phase < param(1u) {
        return pixel;
    }
    switch u32(param(2u)) {
        case 0u: {
            return vec4<f32>(0.0, 0.0, 0.0, pixel.a);
        }
        case 1u: {
            return vec4<f32>(pixel.a, pixel.a, pixel.a, pixel.a);
        }
        default: {
            return vec4<f32>(0.0);
        }
    }
}
