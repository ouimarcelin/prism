// Curl noise: points carried a few steps along the curl of a fractal
// noise (a flow without sources or sinks), then the noise read there:
// swirling, streaked shapes.

fn curl_potential(q: vec2<f32>, z: f32) -> f32 {
    return fractal(vec3<f32>(q, z), 2u, 0u, param(2u), 2.0, 0.5, 1.0);
}

fn field(p: vec2<f32>, t: f32) -> f32 {
    let z = t * param(4u);
    let e = 0.01;
    var q = p * param(0u);
    let steps = u32(clamp(round(param(3u)), 1.0, 8.0));
    for (var step = 0u; step < steps; step += 1u) {
        let dx = curl_potential(q + vec2<f32>(e, 0.0), z) - curl_potential(q - vec2<f32>(e, 0.0), z);
        let dy = curl_potential(q + vec2<f32>(0.0, e), z) - curl_potential(q - vec2<f32>(0.0, e), z);
        let flow = vec2<f32>(dy, -dx) / (2.0 * e);
        q += flow * param(1u) * 0.03;
    }
    return curl_potential(q * 1.7 + vec2<f32>(3.7, 1.9), z);
}
