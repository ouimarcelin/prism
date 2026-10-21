// Dot grid: round or square dots on a grid, their size, brightness or
// both following a fractal noise.

fn field(p: vec2<f32>, t: f32) -> f32 {
    let pitch = param(0u);
    let q = p * pitch;
    let cell = floor(q) + vec2<f32>(0.5);
    let n = fractal(vec3<f32>(cell / pitch * param(2u), t * param(3u)), 2u, 0u, 4.0, 2.0, 0.5, 1.0);
    let mode = u32(param(5u));
    let radius = param(1u) * 0.5 * select(1.0, n, mode != 1u);
    let brightness = select(1.0, n, mode != 0u);
    let f = q - cell;
    var d = length(f);
    if u32(param(4u)) == 1u {
        d = max(abs(f.x), abs(f.y));
    }
    let soft = bank_pixel() * pitch * 1.2;
    return brightness * (1.0 - smoothstep(radius - soft, radius + soft, d));
}
