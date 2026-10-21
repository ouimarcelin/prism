// 2D fluid: a velocity field carried by itself, stirred by a
// divergence-free curl noise, smoothed by viscosity; colour poured at
// moving points is carried along and slowly fades. The kept image holds
// the velocity (in texels a step) in red and green, the colour in blue.
// There is no pressure solve: the stirring keeps the flow swirling.

fn fluid_read(uv: vec2<f32>) -> vec4<f32> {
    return textureSampleLevel(source, source_sampler, fract(uv), 0.0);
}

fn fluid_potential(q: vec2<f32>, z: f32) -> f32 {
    return fractal(vec3<f32>(q, z), 2u, 0u, 3.0, 2.0, 0.5, 1.0);
}

@fragment
fn fs_step(in: VertexOutput) -> @location(0) vec4<f32> {
    let p = bank_point(in.uv);
    let t = bank_time();
    let e = bank_texel();
    let here = fluid_read(in.uv);
    if here.a < 0.5 {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }
    // Carried back along the velocity (semi-Lagrangian).
    let carried = fluid_read(in.uv - here.xy * e);
    let around = 0.25 * (fluid_read(in.uv + vec2<f32>(e.x, 0.0)).xy + fluid_read(in.uv - vec2<f32>(e.x, 0.0)).xy
        + fluid_read(in.uv + vec2<f32>(0.0, e.y)).xy + fluid_read(in.uv - vec2<f32>(0.0, e.y)).xy);
    var velocity = mix(carried.xy, around, param(2u) * 0.5) * (1.0 - param(2u) * 0.02);
    // Stirring: the curl of a noise, without sources or sinks.
    let q = p * param(6u);
    let d = 0.01;
    let z = t * 0.2;
    let dx = fluid_potential(q + vec2<f32>(d, 0.0), z) - fluid_potential(q - vec2<f32>(d, 0.0), z);
    let dy = fluid_potential(q + vec2<f32>(0.0, d), z) - fluid_potential(q - vec2<f32>(0.0, d), z);
    // Plane y runs up, texture y down.
    velocity += param(3u) * 0.05 * vec2<f32>(dy, dx) / (2.0 * d);
    let speed = length(velocity);
    if speed > 4.0 {
        velocity *= 4.0 / speed;
    }
    // Colour poured at three moving points.
    var colour = carried.z * (1.0 - param(5u) * 0.02);
    for (var index = 0u; index < 3u; index += 1u) {
        let i = f32(index);
        let source_point = 0.35 * vec2<f32>(sin(t * 0.5 + i * 2.1), cos(t * 0.37 + i * 1.3));
        let away = p - source_point;
        colour += param(4u) * 0.15 * exp(-dot(away, away) / 0.002);
    }
    return vec4<f32>(velocity, clamp(colour, 0.0, 1.0), 1.0);
}

fn field(p: vec2<f32>, t: f32) -> f32 {
    return sample(bank_uv).z;
}
