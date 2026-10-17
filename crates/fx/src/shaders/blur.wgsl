// Gaussian blur in two passes, across then down. The radius is in pixels;
// the edge of the image is repeated outside of it.
//
// A bell curve over 13 taps spread across the radius, read in 7 fetches:
// each pair of taps on a side is one fetch between them, weighted by both,
// which the sampler's linear filtering blends.

const CENTER: f32 = 0.19967563;
// Offset in taps from the center, and weight, of each merged pair.
const OFFSETS: array<f32, 3> = array<f32, 3>(1.4073334, 3.2942150, 5.2018132);
const WEIGHTS: array<f32, 3> = array<f32, 3>(0.29732251, 0.09184834, 0.01099133);
const TAPS: f32 = 6.0;

fn blur(uv: vec2<f32>, direction: vec2<f32>) -> vec4<f32> {
    let radius = param(0u);
    if radius <= 0.0 {
        return sample_clamped(uv);
    }
    let step = direction * (radius / TAPS) / params.size;
    var total = sample_clamped(uv) * CENTER;
    for (var pair = 0; pair < 3; pair += 1) {
        let offset = step * OFFSETS[pair];
        total += (sample_clamped(uv + offset) + sample_clamped(uv - offset)) * WEIGHTS[pair];
    }
    return total;
}

@fragment
fn fs_horizontal(in: VertexOutput) -> @location(0) vec4<f32> {
    return blur(in.uv, vec2<f32>(1.0, 0.0));
}

@fragment
fn fs_vertical(in: VertexOutput) -> @location(0) vec4<f32> {
    return blur(in.uv, vec2<f32>(0.0, 1.0));
}
