// Particles in a flow: sparks dropped at random each step, carried along
// the curl of a noise, the kept image fading behind them into trails.
// The kept image holds the brightness in red.

fn flow_potential(q: vec2<f32>, z: f32) -> f32 {
    return fractal(vec3<f32>(q, z), 2u, 0u, 3.0, 2.0, 0.5, 1.0);
}

@fragment
fn fs_step(in: VertexOutput) -> @location(0) vec4<f32> {
    let p = bank_point(in.uv);
    let t = bank_time();
    let e = bank_texel();
    let q = p * param(5u);
    let d = 0.01;
    let z = t * 0.1;
    let dx = flow_potential(q + vec2<f32>(d, 0.0), z) - flow_potential(q - vec2<f32>(d, 0.0), z);
    let dy = flow_potential(q + vec2<f32>(0.0, d), z) - flow_potential(q - vec2<f32>(0.0, d), z);
    var flow = vec2<f32>(dy, dx) / (2.0 * d);
    flow = flow / max(length(flow), 1e-3) * param(4u);
    let behind = textureSampleLevel(source, source_sampler, fract(in.uv - flow * e), 0.0);
    var light = behind.r * param(3u);
    // New sparks, a different scatter each step.
    let pixel = floor(in.uv / e);
    let step_index = floor(params.time * 240.0);
    if hash31(vec3<f32>(pixel, step_index)) < param(2u) * 0.003 {
        light = 1.0;
    }
    return vec4<f32>(clamp(light, 0.0, 1.0), 0.0, 0.0, 1.0);
}

fn field(p: vec2<f32>, t: f32) -> f32 {
    return sample(bank_uv).r;
}
