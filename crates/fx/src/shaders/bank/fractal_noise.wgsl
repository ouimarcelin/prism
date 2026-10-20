// Fractal noise: value, Perlin or simplex noise summed over octaves, as
// fBm, turbulence, ridges or billows; it evolves in time (third axis) and
// drifts across the plane.

fn field(p: vec2<f32>, t: f32) -> f32 {
    let drift = vec2<f32>(cos(radians(param(8u))), sin(radians(param(8u)))) * param(7u) * t;
    let point = vec3<f32>((p + drift) * param(2u), t * param(6u));
    return fractal(point, u32(param(0u)), u32(param(1u)), param(3u), param(4u), param(5u), param(9u));
}
