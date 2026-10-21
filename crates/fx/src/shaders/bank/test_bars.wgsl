// Test patterns: bars stepping down in brightness, a ramp, a convergence
// grid with a circle, or a zone plate (rings ever finer); they can drift.

fn field(p: vec2<f32>, t: f32) -> f32 {
    let aspect = params.size.x / max(params.size.y, 1.0);
    let x = fract(p.x / aspect + 0.5 + t * param(2u) * 0.1);
    switch u32(param(0u)) {
        case 1u: {
            return x;
        }
        case 2u: {
            let lines = round(param(1u)) * 2.0;
            let g = abs(fract(p * lines + vec2<f32>(0.5)) - vec2<f32>(0.5)) / lines;
            let soft = bank_pixel() * 1.5;
            let grid = 1.0 - smoothstep(0.0, soft, min(g.x, g.y));
            let circle = 1.0 - smoothstep(0.0, soft, abs(length(p) - 0.4));
            return max(grid, circle);
        }
        case 3u: {
            let r = length(p);
            return 0.5 + 0.5 * cos(PI * r * r * 200.0 * param(1u) / 8.0 - t);
        }
        default: {
            let bars = round(param(1u));
            return 1.0 - floor(x * bars) / max(bars - 1.0, 1.0);
        }
    }
}
