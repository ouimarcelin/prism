// Truchet tiles: square tiles with arcs or diagonals, each turned at
// random; they flip again at the chosen rate.

fn field(p: vec2<f32>, t: f32) -> f32 {
    let q = p * param(1u);
    let cell = floor(q);
    var f = fract(q) - vec2<f32>(0.5);
    let flip = hash31(vec3<f32>(cell, floor(t * param(3u)) + 0.5));
    if flip > 0.5 {
        f.x = -f.x;
    }
    let soft = bank_pixel() * param(1u) * 1.5;
    let half = param(2u) * 0.5;
    switch u32(param(0u)) {
        case 1u: {
            let d = abs(f.x - f.y) / sqrt(2.0);
            return 1.0 - smoothstep(half - soft, half + soft, d);
        }
        case 2u: {
            return smoothstep(-soft, soft, f.y - f.x);
        }
        default: {
            let a = abs(length(f - vec2<f32>(0.5)) - 0.5);
            let b = abs(length(f + vec2<f32>(0.5)) - 0.5);
            return 1.0 - smoothstep(half - soft, half + soft, min(a, b));
        }
    }
}
