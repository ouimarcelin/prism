// Line waves: horizontal lines pushed up and down by a sine or by noise,
// optionally seen in perspective (crowding towards a horizon at the top).

fn field(p: vec2<f32>, t: f32) -> f32 {
    let lines = round(param(0u));
    let depth = param(4u);
    // 0 at the bottom of the frame, 1 at the top.
    let height = clamp(p.y + 0.5, 0.0, 1.0);
    let row = mix(height, 1.0 - sqrt(1.0 - height), depth);
    let x = p.x / mix(1.0, 1.0 - 0.8 * height, depth);
    let sine = sin(2.0 * PI * (x * param(2u) - t)) * 0.5;
    let noise = fractal(vec3<f32>(x * param(2u), row * 3.0, t * 0.5), 1u, 0u, 3.0, 2.0, 0.5, 1.0) - 0.5;
    let push = param(1u) * mix(sine, noise, param(5u)) * (1.0 - 0.7 * depth * height);
    let along = (row + push) * lines;
    // 0 on a line, 1 halfway between two.
    let from_line = abs(fract(along) - 0.5) * 2.0;
    let thickness = param(3u);
    return 1.0 - smoothstep(thickness - 0.05, thickness + 0.05, from_line);
}
