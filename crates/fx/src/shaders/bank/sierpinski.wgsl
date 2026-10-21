// Sierpinski triangle (odd numbers of Pascal's triangle, by Lucas's
// theorem) or carpet (holes in the middle thirds), to a number of
// levels.

fn field(p: vec2<f32>, t: f32) -> f32 {
    let levels = u32(clamp(round(param(0u)), 1.0, 10.0));
    if u32(param(1u)) == 1u {
        var q = p / 0.9 + vec2<f32>(0.5);
        if any(q < vec2<f32>(0.0)) || any(q >= vec2<f32>(1.0)) {
            return 0.0;
        }
        for (var level = 0u; level < levels; level += 1u) {
            q = q * 3.0;
            let digit = floor(q);
            if digit.x == 1.0 && digit.y == 1.0 {
                return 0.0;
            }
            q = fract(q);
        }
        return 1.0;
    }
    let side = 0.9;
    let y = (0.45 - p.y) / side;
    let x = p.x / (side * 2.0 / sqrt(3.0)) + y * 0.5;
    if y < 0.0 || y >= 1.0 || x < 0.0 || x > y {
        return 0.0;
    }
    let count = f32(1u << levels);
    let row = u32(y * count);
    let column = min(u32(x * count), row);
    return select(0.0, 1.0, (row & column) == column);
}
