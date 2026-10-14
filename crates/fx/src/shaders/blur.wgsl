// Gaussian blur in two passes, across then down. The radius is in pixels;
// the edge of the image is repeated outside of it.

const TAPS: i32 = 6;

fn blur(uv: vec2<f32>, direction: vec2<f32>) -> vec4<f32> {
    let radius = param(0u);
    if radius <= 0.0 {
        return sample_clamped(uv);
    }
    // Taps spread over the radius, weighted by a bell curve.
    let step = direction * (radius / f32(TAPS)) / params.size;
    var total = vec4<f32>(0.0);
    var weights = 0.0;
    for (var tap = -TAPS; tap <= TAPS; tap += 1) {
        let x = f32(tap) / f32(TAPS);
        let weight = exp(-4.5 * x * x);
        total += sample_clamped(uv + step * f32(tap)) * weight;
        weights += weight;
    }
    return total / weights;
}

@fragment
fn fs_horizontal(in: VertexOutput) -> @location(0) vec4<f32> {
    return blur(in.uv, vec2<f32>(1.0, 0.0));
}

@fragment
fn fs_vertical(in: VertexOutput) -> @location(0) vec4<f32> {
    return blur(in.uv, vec2<f32>(0.0, 1.0));
}
