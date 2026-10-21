// Ocean: a sum of directional waves around the wind, sharpened crests,
// shaded by their slope, foam on the highest crests; seen from above or
// in perspective towards a horizon.

fn ocean_height(q: vec2<f32>, t: f32) -> f32 {
    let wind = radians(param(1u));
    let sharp = 1.0 + param(2u) * 3.0;
    var height = 0.0;
    var amplitude = 1.0;
    var frequency = 1.2;
    for (var index = 0u; index < 6u; index += 1u) {
        let angle = wind + (f32(index) - 2.5) * 0.35;
        let phase = dot(q, vec2<f32>(cos(angle), sin(angle))) * frequency - t * sqrt(frequency) * 1.5 + f32(index) * 1.7;
        height += amplitude * pow(0.5 + 0.5 * sin(phase), sharp);
        amplitude *= 0.62;
        frequency *= 1.45;
    }
    return height / 2.4;
}

fn field(p: vec2<f32>, t: f32) -> f32 {
    let perspective = param(4u);
    let below = param(5u) - p.y;
    if perspective > 0.0 && below <= 0.0 {
        // Sky, lighter towards the horizon.
        return mix(0.6, 0.85, clamp(-below * 2.0, 0.0, 1.0));
    }
    let depth = 0.35 / max(below, 1e-3);
    let q = mix(p * 6.0, vec2<f32>(p.x * depth, depth) * 2.0, perspective);
    let h = ocean_height(q, t);
    let e = 0.02;
    let hx = ocean_height(q + vec2<f32>(e, 0.0), t) - h;
    let hy = ocean_height(q + vec2<f32>(0.0, e), t) - h;
    let normal = normalize(vec3<f32>(-hx / e * param(0u), -hy / e * param(0u), 1.0));
    let light = clamp(dot(normal, normalize(vec3<f32>(0.3, 0.5, 0.8))), 0.0, 1.0);
    var value = 0.15 + 0.55 * light * light;
    let foam = param(3u);
    value = mix(value, 1.0, smoothstep(1.0 - foam * 0.5, 1.0 - foam * 0.3, h) * step(0.01, foam));
    // Haze towards the horizon.
    let haze = mix(0.0, 1.0 - exp(-depth * 0.04), perspective);
    return mix(value, 0.6, haze);
}
