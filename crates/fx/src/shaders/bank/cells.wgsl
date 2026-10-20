// Cells (Worley, Voronoi): distances to scattered points that swing in
// time. Modes: 0 nearest (F1), 1 edges (F2 − F1), 2 flat cells, 3 second
// nearest (F2), 4 product (F1 × F2).

fn field(p: vec2<f32>, t: f32) -> f32 {
    let cells = worley(p * param(2u), t, param(3u), param(4u), u32(param(1u)));
    let f1 = cells.x;
    let f2 = cells.y;
    let id = cells.z;
    let edge = max(param(5u), 1e-3);
    switch u32(param(0u)) {
        case 1u: {
            return smoothstep(0.0, edge, f2 - f1);
        }
        case 2u: {
            // Each cell its own value, darkened along its border.
            return id * smoothstep(0.0, edge, f2 - f1);
        }
        case 3u: {
            return clamp(f2 * 0.8, 0.0, 1.0);
        }
        case 4u: {
            return clamp(f1 * f2 * 1.5, 0.0, 1.0);
        }
        default: {
            return clamp(f1, 0.0, 1.0);
        }
    }
}
