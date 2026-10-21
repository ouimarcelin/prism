// Tilings: hexagons, triangles or squares, filled or outlined, each tile
// pulsing at its own pace.

fn tile_mod(x: vec2<f32>, y: vec2<f32>) -> vec2<f32> {
    return x - y * floor(x / y);
}

fn field(p: vec2<f32>, t: f32) -> f32 {
    let q = p * param(1u);
    var edge: f32;
    var id: vec2<f32>;
    switch u32(param(0u)) {
        case 1u: {
            // Triangles, in skewed coordinates.
            let s = vec2<f32>(q.x - q.y / sqrt(3.0), q.y * 2.0 / sqrt(3.0));
            let cell = floor(s);
            let f = fract(s);
            let upper = f.x + f.y > 1.0;
            let g = select(f, vec2<f32>(1.0) - f, upper);
            edge = min(min(g.x, g.y), 1.0 - g.x - g.y) * sqrt(3.0) * 0.5;
            id = cell * 2.0 + select(vec2<f32>(0.0), vec2<f32>(1.0, 0.0), upper);
        }
        case 2u: {
            let f = fract(q) - vec2<f32>(0.5);
            edge = 0.5 - max(abs(f.x), abs(f.y));
            id = floor(q);
        }
        default: {
            let r = vec2<f32>(1.0, sqrt(3.0));
            let h = r * 0.5;
            let a = tile_mod(q, r) - h;
            let b = tile_mod(q - h, r) - h;
            let g = select(b, a, dot(a, a) < dot(b, b));
            let hex = max(dot(abs(g), normalize(vec2<f32>(1.0, sqrt(3.0)))), abs(g.x));
            edge = 0.5 - hex;
            id = q - g;
        }
    }
    let soft = bank_pixel() * param(1u) * 1.5;
    let gap = param(2u) * 0.5;
    let random = hash31(vec3<f32>(round(id * 2.0), 5.0));
    let pulse = 0.5 + 0.5 * sin(t * PI + random * 2.0 * PI);
    let shade = mix(1.0, pulse, param(3u));
    if u32(param(4u)) == 1u {
        return 1.0 - smoothstep(gap - soft, gap + soft, edge);
    }
    return smoothstep(gap - soft, gap + soft, edge) * shade;
}
