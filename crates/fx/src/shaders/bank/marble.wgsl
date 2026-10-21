// Marble and wood: sines bent by a fractal noise; marble along one
// direction, wood in rings around the centre.

fn field(p: vec2<f32>, t: f32) -> f32 {
    let n = fractal(vec3<f32>(p * param(4u), t * 0.1), 1u, 1u, 5.0, 2.0, 0.5, 1.0);
    let bend = param(2u) * n * 4.0;
    let sharp = param(3u);
    if u32(param(0u)) == 1u {
        let rings = fract(length(p) * param(1u) + bend * 0.25);
        return pow(smoothstep(0.0, 1.0, rings), 1.0 + sharp * 4.0);
    }
    let vein = 0.5 + 0.5 * sin(p.x * param(1u) * PI + bend);
    return 1.0 - pow(1.0 - vein, 1.0 + sharp * 8.0);
}
