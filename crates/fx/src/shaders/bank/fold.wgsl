// Fractal fold: the plane folded on its axes, turned, scaled and shifted
// again and again; the closest pass to an axis draws thin, branching
// lines.

fn field(p: vec2<f32>, t: f32) -> f32 {
    let folds = u32(clamp(round(param(0u)), 1.0, 16.0));
    let angle = radians(param(1u)) + t * 0.1;
    let scale = param(2u);
    let shift = vec2<f32>(param(3u));
    var q = p * 2.0;
    var size = 1.0;
    var trap = 1e9;
    for (var index = 0u; index < folds; index += 1u) {
        q = abs(q) - shift;
        q = rotate(q, angle);
        q = q * scale;
        size *= scale;
        trap = min(trap, abs(q.y) / size);
    }
    let width = max(param(4u) * 0.02, 1e-4);
    return 1.0 - smoothstep(0.0, width, trap);
}
