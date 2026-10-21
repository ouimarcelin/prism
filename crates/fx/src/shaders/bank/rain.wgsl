// Rain and snow: layers falling at their own speed, slanted by the
// angle; streaks for rain, drifting flakes for snow, nearer layers
// brighter.

fn field(p: vec2<f32>, t: f32) -> f32 {
    let snow = u32(param(0u)) == 1u;
    let layers = u32(clamp(round(param(3u)), 1.0, 6.0));
    let q0 = rotate(p, radians(param(2u)));
    var light = 0.0;
    for (var layer = 0u; layer < layers; layer += 1u) {
        let l = f32(layer);
        let scale = 6.0 + l * 5.0;
        let fall = select(2.0, 0.25, snow) * (1.0 + 0.5 * (f32(layers) - l));
        var q = q0 * scale;
        q.y += t * fall * scale * 0.25;
        let cell = floor(q);
        let random = hash33(vec3<f32>(cell, l * 3.0 + 1.0));
        if random.z > param(1u) {
            continue;
        }
        var f = fract(q) - vec2<f32>(0.2) - random.xy * 0.6;
        var shape: f32;
        if snow {
            f.x += 0.15 * sin(t * 2.0 + random.z * 6.28);
            shape = 1.0 - smoothstep(0.03 * param(5u), 0.06 * param(5u), length(f));
        } else {
            let half = 0.05 + 0.4 * param(4u);
            shape = (1.0 - smoothstep(0.01 * param(5u), 0.025 * param(5u), abs(f.x)))
                * (1.0 - smoothstep(half * 0.6, half, abs(f.y)));
        }
        light += shape * (1.0 - 0.6 * l / f32(layers));
    }
    return clamp(light, 0.0, 1.0);
}
