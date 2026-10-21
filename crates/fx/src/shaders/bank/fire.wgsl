// Fire: a turbulent noise stretched upwards, eaten away more and more
// with the height; the wind bends the flames.

fn field(p: vec2<f32>, t: f32) -> f32 {
    let height = p.y + 0.5;
    let x = p.x - param(2u) * height * height * 0.5;
    let scale = param(3u);
    let q = vec2<f32>(x * scale, height * scale * 0.6 - t * 1.5);
    let n = fractal(vec3<f32>(q, t * param(5u) * 0.5), 1u, 1u, 5.0, 2.0, 0.5, 1.0);
    let top = max(param(0u), 0.05);
    var value = (1.0 - height / top) * 1.4 - n * 0.9;
    value *= param(1u);
    let width = param(4u);
    if width < 2.0 {
        let half = width * 0.5;
        value *= 1.0 - smoothstep(half * 0.6, half, abs(x));
    }
    return clamp(value, 0.0, 1.0);
}
