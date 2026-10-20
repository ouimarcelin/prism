// Domain warping: a fractal noise read at a point pushed by other fractal
// noises, one to three times, for marbled, flowing shapes.

fn warp_noise(p: vec2<f32>, t: f32) -> f32 {
    return fractal(vec3<f32>(p, t), u32(param(0u)), 0u, param(2u), 2.0, 0.5, 1.0);
}

fn field(p: vec2<f32>, t: f32) -> f32 {
    let point = p * param(1u);
    let z = t * param(6u);
    let strength = param(4u);
    let scale = param(5u);
    let levels = u32(clamp(round(param(3u)), 1.0, 3.0));
    var push = vec2<f32>(0.0);
    for (var level = 0u; level < levels; level += 1u) {
        let shift = f32(level) * 4.7;
        let at = point * scale + strength * push;
        push = vec2<f32>(
            warp_noise(at + vec2<f32>(shift, 1.3), z) * 2.0 - 1.0,
            warp_noise(at + vec2<f32>(5.2 + shift, 9.2), z) * 2.0 - 1.0,
        );
    }
    return warp_noise(point + strength * push, z);
}
