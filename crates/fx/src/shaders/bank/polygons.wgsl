// Repeated polygons: nested regular polygons moving outwards, each ring
// turned a little more than the one inside it.

fn polygon_distance(p: vec2<f32>, sides: f32, turn: f32) -> f32 {
    let sector = 2.0 * PI / sides;
    let a = atan2(p.y, p.x) + turn;
    let local = a - sector * floor(a / sector) - sector * 0.5;
    return length(p) * cos(local) / cos(PI / sides);
}

fn field(p: vec2<f32>, t: f32) -> f32 {
    let sides = round(param(0u));
    let repeats = param(1u);
    let turn_step = radians(param(2u));
    var d = polygon_distance(p, sides, 0.0);
    // Twice: the ring index sets the turn, the turn moves the ring.
    for (var attempt = 0u; attempt < 2u; attempt += 1u) {
        let ring = floor(d * repeats - t);
        d = polygon_distance(p, sides, ring * turn_step);
    }
    return wave_shape(d * repeats - t, 2u, param(3u), 0.2);
}
