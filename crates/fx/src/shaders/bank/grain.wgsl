// Grain: a new random value per grain and per frame of the grain's own
// rate, like the snow of a television; below full density, only the
// brightest grains show.

fn field(p: vec2<f32>, t: f32) -> f32 {
    let size = max(param(0u), 1e-4);
    let cell = floor(p / size);
    let frame = floor(t * param(1u));
    let value = hash31(vec3<f32>(cell, frame));
    let density = param(2u);
    if density >= 1.0 {
        return value;
    }
    return select(0.0, value, value > 1.0 - density);
}
