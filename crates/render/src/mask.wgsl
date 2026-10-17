// The coverage of a mask over its screen, drawn when the mask changes: a
// polygon in pixels of the screen, inside which the slice shows (outside
// when inverted), with a soft edge centered on the outline.

struct Mask {
    count: u32,
    inverted: u32,
    feather: f32,
    unused: u32,
    points: array<vec2<f32>>,
}

@group(0) @binding(0) var<storage, read> mask: Mask;

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let corner = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    return vec4<f32>(corner * 2.0 - 1.0, 0.0, 1.0);
}

@fragment
fn fs_main(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let pixel = position.xy;
    var inside = false;
    var nearest = 1e18;
    // Each edge from the previous point: one read per edge.
    var a = mask.points[mask.count - 1u];
    for (var i = 0u; i < mask.count; i += 1u) {
        let b = mask.points[i];
        if (a.y > pixel.y) != (b.y > pixel.y) {
            let x = a.x + (pixel.y - a.y) / (b.y - a.y) * (b.x - a.x);
            if pixel.x < x {
                inside = !inside;
            }
        }
        let edge = b - a;
        let t = clamp(dot(pixel - a, edge) / max(dot(edge, edge), 1e-6), 0.0, 1.0);
        let away = pixel - (a + edge * t);
        nearest = min(nearest, dot(away, away));
        a = b;
    }
    var signed = select(-sqrt(nearest), sqrt(nearest), inside);
    if mask.inverted != 0u {
        signed = -signed;
    }
    // At least a pixel of soft edge, so the outline is not jagged.
    let feather = max(mask.feather, 1.0);
    return vec4<f32>(clamp(signed / feather + 0.5, 0.0, 1.0), 0.0, 0.0, 1.0);
}
