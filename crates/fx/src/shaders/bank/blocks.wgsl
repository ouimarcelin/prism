// Digital blocks: rectangles of corrupted picture flashing in and out,
// with smaller blocks inside some of them.

fn field(p: vec2<f32>, t: f32) -> f32 {
    let size = param(0u);
    let stretch = param(3u);
    let frame = floor(t * param(2u));
    let q = p * vec2<f32>(size / stretch, size);
    let cell = floor(q);
    let random = hash33(vec3<f32>(cell, frame));
    if random.x > param(1u) {
        return 0.0;
    }
    // Inside a lit block: a smaller block, or stripes, or a flat value.
    let inner = floor(q * 4.0);
    let small = hash33(vec3<f32>(inner, frame + 0.5));
    if random.y < 0.3 {
        return small.x;
    }
    if random.y < 0.5 {
        return step(0.5, fract(q.y * 8.0 + random.z));
    }
    return random.z;
}
