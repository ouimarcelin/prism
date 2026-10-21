// Plasma: a sum of sines, straight and circular, each with its own
// direction, frequency and pace; complexity folds the sum through one more
// sine.

fn field(p: vec2<f32>, t: f32) -> f32 {
    let terms = u32(clamp(round(param(0u)), 1.0, 8.0));
    let frequency = param(1u);
    var sum = 0.0;
    for (var index = 0u; index < terms; index += 1u) {
        let i = f32(index);
        let f = frequency * (1.0 + 0.37 * i);
        let pace = (0.6 + 0.29 * i) * select(1.0, -1.0, index % 2u == 1u);
        if index % 3u == 2u {
            // Circular term around a wandering centre.
            let centre = 0.4 * vec2<f32>(sin(t * 0.31 + i), cos(t * 0.27 + 2.0 * i));
            sum += sin(length(p - centre) * f * 1.5 + t * pace);
        } else {
            let a = i * 2.399;
            sum += sin(dot(p, vec2<f32>(cos(a), sin(a))) * f + t * pace);
        }
    }
    let v = sum / f32(terms);
    let folded = sin(PI * v * (1.0 + param(2u)));
    return 0.5 + 0.5 * mix(v, folded, min(param(2u), 1.0));
}
