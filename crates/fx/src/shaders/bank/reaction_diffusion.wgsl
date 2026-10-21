// Reaction-diffusion (Gray-Scott): two substances spread at different
// rates; B feeds on A and A is fed back from outside. Depending on the
// feed and kill rates the image grows spots, stripes or mazes. The kept
// image holds A in red, B in green; alpha 0 means not seeded yet.

fn rd_read(uv: vec2<f32>) -> vec4<f32> {
    return textureSampleLevel(source, source_sampler, fract(uv), 0.0);
}

@fragment
fn fs_step(in: VertexOutput) -> @location(0) vec4<f32> {
    let p = bank_point(in.uv);
    let t = bank_time();
    let here = rd_read(in.uv);
    if here.a < 0.5 {
        // Seeds: small discs of B scattered on the plane.
        let cell = floor(p * 12.0);
        let local = fract(p * 12.0) - vec2<f32>(0.5);
        let seeded = hash31(vec3<f32>(cell, 3.0)) < param(5u) * 0.5 && length(local) < 0.3;
        return vec4<f32>(1.0, select(0.0, 1.0, seeded), 0.0, 1.0);
    }
    let e = bank_texel();
    var laplacian = -here.rg;
    laplacian += 0.2 * (rd_read(in.uv + vec2<f32>(e.x, 0.0)).rg + rd_read(in.uv - vec2<f32>(e.x, 0.0)).rg
        + rd_read(in.uv + vec2<f32>(0.0, e.y)).rg + rd_read(in.uv - vec2<f32>(0.0, e.y)).rg);
    laplacian += 0.05 * (rd_read(in.uv + e).rg + rd_read(in.uv - e).rg
        + rd_read(in.uv + vec2<f32>(e.x, -e.y)).rg + rd_read(in.uv + vec2<f32>(-e.x, e.y)).rg);
    let a = here.r;
    let b = here.g;
    let feed = param(2u);
    let kill = param(3u);
    let spread = param(4u);
    let reaction = a * b * b;
    var next_a = a + spread * 1.0 * laplacian.x - reaction + feed * (1.0 - a);
    var next_b = b + spread * 0.5 * laplacian.y + reaction - (kill + feed) * b;
    // Injection: B dropped where a slow noise peaks.
    let spot = smoothstep(0.72, 0.8, fractal(vec3<f32>(p * 3.0, t * 0.3), 2u, 0u, 2.0, 2.0, 0.5, 1.0));
    next_b += param(6u) * 0.05 * spot;
    return vec4<f32>(clamp(next_a, 0.0, 1.0), clamp(next_b, 0.0, 1.0), 0.0, 1.0);
}

fn field(p: vec2<f32>, t: f32) -> f32 {
    let kept = sample(bank_uv);
    return clamp(kept.g * 3.0, 0.0, 1.0);
}
