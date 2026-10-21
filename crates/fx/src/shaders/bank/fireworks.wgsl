// Fireworks: a burst at each beat (or more, or fewer), its sparks flying
// out, slowing and falling; the kept image fades and drifts down behind
// them into trails. The kept image holds the brightness in red.

fn fs_burst(p: vec2<f32>, t: f32, burst: f32) -> f32 {
    let rate = param(2u);
    let age = t - burst / rate;
    if age < 0.0 || age > 2.5 {
        return 0.0;
    }
    let random = hash33(vec3<f32>(burst, 9.0, 4.0));
    let centre = vec2<f32>(random.x * 1.2 - 0.6, random.y * 0.4 - 0.05);
    let sparks = u32(clamp(round(param(6u)), 8.0, 64.0));
    let reach = param(3u) * (0.6 + 0.4 * random.z) * (1.0 - exp(-age * 3.0));
    let fall = vec2<f32>(0.0, -param(4u) * 0.15 * age * age);
    let width = max(bank_pixel() * 2.5, bank_texel().y * 1.5 / max(bank_setting(C_ZOOM), 1e-3));
    let fade = exp(-age * 1.5);
    var light = 0.0;
    for (var index = 0u; index < sparks; index += 1u) {
        let jitter = hash31(vec3<f32>(burst, f32(index), 5.0));
        let angle = 2.0 * PI * (f32(index) + jitter * 0.5) / f32(sparks);
        let speed = 0.7 + 0.3 * hash31(vec3<f32>(burst, f32(index), 6.0));
        let spark = centre + vec2<f32>(cos(angle), sin(angle)) * reach * speed + fall;
        let away = p - spark;
        light += exp(-dot(away, away) / (width * width));
    }
    return light * fade;
}

@fragment
fn fs_step(in: VertexOutput) -> @location(0) vec4<f32> {
    let p = bank_point(in.uv);
    let t = bank_time();
    let e = bank_texel();
    // The trail drifts down a little: read from above.
    let above = textureSampleLevel(source, source_sampler, in.uv - vec2<f32>(0.0, e.y * param(4u) * 0.5), 0.0);
    var light = above.r * param(5u);
    let latest = floor(t * param(2u));
    light += fs_burst(p, t, latest) + fs_burst(p, t, latest - 1.0) + fs_burst(p, t, latest - 2.0);
    return vec4<f32>(clamp(light, 0.0, 1.0), 0.0, 0.0, 1.0);
}

fn field(p: vec2<f32>, t: f32) -> f32 {
    return sample(bank_uv).r;
}
