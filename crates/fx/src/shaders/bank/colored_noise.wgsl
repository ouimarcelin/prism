// Pink and brown noise: grain summed over octaves, coarser octaves
// stronger as the spectral slope rises (0 white, 1 pink, 2 brown).

fn field(p: vec2<f32>, t: f32) -> f32 {
    let size = max(param(1u), 1e-4);
    let frame = floor(t * param(2u));
    let slope = param(0u);
    var sum = 0.0;
    var norm = 0.0;
    for (var octave = 0u; octave < 8u; octave += 1u) {
        let scale = exp2(f32(octave));
        let amplitude = pow(scale, slope * 0.5);
        let n = value_noise(vec3<f32>(p / (size * scale), frame + f32(octave) * 13.0));
        sum += n * amplitude;
        norm += amplitude;
    }
    return clamp(0.5 + sum / norm, 0.0, 1.0);
}
