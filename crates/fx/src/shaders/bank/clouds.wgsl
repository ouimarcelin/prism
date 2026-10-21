// Clouds: a fractal noise kept above a coverage threshold, lit from the
// side of the sun, drifting with the wind.

fn field(p: vec2<f32>, t: f32) -> f32 {
    let wind = radians(param(4u));
    let drift = vec2<f32>(cos(wind), sin(wind)) * param(3u) * t * 0.1;
    let q = (p - drift) * param(2u);
    let sun = radians(param(5u));
    let towards = vec2<f32>(cos(sun), sin(sun)) * 0.05;
    let n = fractal(vec3<f32>(q, t * 0.03), 2u, 0u, 6.0, 2.0, 0.5, 1.0);
    let lit = fractal(vec3<f32>(q + towards, t * 0.03), 2u, 0u, 6.0, 2.0, 0.5, 1.0);
    let threshold = 1.0 - param(0u);
    let density = smoothstep(threshold, threshold + param(1u), n);
    let light = clamp(0.6 + (lit - n) * 6.0, 0.0, 1.0);
    return mix(0.2, 0.35 + 0.65 * light, density);
}
